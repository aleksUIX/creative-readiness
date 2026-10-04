//! Local identity lookup. Not a shared SSP graph.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{CreativeIdentity, DimensionState, Finding, Lifecycle, ReadinessEvidence};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCreative {
    pub fingerprint: String,
    pub identity: CreativeIdentity,
    pub evidence: ReadinessEvidence,
}

#[derive(Debug, Clone, Default)]
pub struct IdentityStore {
    by_fingerprint: HashMap<String, StoredCreative>,
    by_crid: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum LookupHit {
    Unknown,
    Known { evidence: ReadinessEvidence },
    MediaChanged { previous_fingerprint: String },
}

pub fn media_fingerprint(id: &CreativeIdentity) -> String {
    let uad = id
        .universal_ad_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let base = if let Some(h) = id
        .content_hash
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        format!("hash:{h}")
    } else if let Some(url) = id
        .media_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        format!("url:{:x}", hasher.finalize())
    } else {
        String::new()
    };
    if base.is_empty() {
        return String::new();
    }
    match uad {
        Some(u) => format!("{base}|uad:{u}"),
        None => base,
    }
}

pub fn apply_reuse(ev: &mut ReadinessEvidence, prior: &ReadinessEvidence) {
    ev.dimensions.technical = prior.dimensions.technical;
    ev.dimensions.operational = prior.dimensions.operational;
    if ev.prepared_media.is_none() {
        ev.prepared_media = prior.prepared_media.clone();
    }
    if ev.operational_claims.is_none() {
        ev.operational_claims = prior.operational_claims.clone();
    }
    ev.lifecycle = if ev.dimensions.operational == DimensionState::Ready
        && ev.dimensions.technical == DimensionState::Ready
        && ev.dimensions.policy != DimensionState::NotReady
    {
        Lifecycle::OperationallyReady
    } else if ev.dimensions.technical == DimensionState::Ready
        && ev.dimensions.policy != DimensionState::NotReady
    {
        Lifecycle::TechnicallyReady
    } else {
        Lifecycle::Unknown
    };
    ev.findings.push(Finding {
        requirement_id: "identity.reused".into(),
        severity: "info".into(),
        message: "same media fingerprint; inherited technical and operational dimensions.".into(),
        remediable: Some(false),
        ..Default::default()
    });
}

impl IdentityStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lookup(&self, identity: &CreativeIdentity) -> LookupHit {
        let fp = media_fingerprint(identity);
        if fp.is_empty() {
            return LookupHit::Unknown;
        }
        if let Some(stored) = self.by_fingerprint.get(&fp) {
            return LookupHit::Known {
                evidence: stored.evidence.clone(),
            };
        }
        if let Some(crid) = identity.crid.as_deref() {
            if let Some(prev) = self.by_crid.get(crid) {
                if prev != &fp {
                    return LookupHit::MediaChanged {
                        previous_fingerprint: prev.clone(),
                    };
                }
            }
        }
        LookupHit::Unknown
    }

    pub fn lookup_crid(&self, crid: &str) -> LookupHit {
        let Some(fp) = self.by_crid.get(crid) else {
            return LookupHit::Unknown;
        };
        match self.by_fingerprint.get(fp) {
            Some(stored) => LookupHit::Known {
                evidence: stored.evidence.clone(),
            },
            None => LookupHit::Unknown,
        }
    }

    /// Seller-supplied fallback_crid. Returns prior evidence if known. Does not choose fallback.
    pub fn lookup_fallback(&self, ctx: &crate::AuctionContext) -> LookupHit {
        match ctx.fallback_crid.as_deref() {
            Some(crid) if !crid.is_empty() => self.lookup_crid(crid),
            _ => LookupHit::Unknown,
        }
    }

    pub fn review_queue(&self) -> Vec<ReadinessEvidence> {
        let mut out: Vec<ReadinessEvidence> = self
            .by_fingerprint
            .values()
            .filter(|row| {
                matches!(
                    row.evidence.lifecycle,
                    Lifecycle::Observed | Lifecycle::PendingReview
                )
            })
            .map(|row| row.evidence.clone())
            .collect();
        out.sort_by(|a, b| a.issued_at.cmp(&b.issued_at));
        out
    }

    pub fn insert(&mut self, mut identity: CreativeIdentity, evidence: ReadinessEvidence) {
        let fp = media_fingerprint(&identity);
        if fp.is_empty() {
            return;
        }
        identity.fingerprint = Some(fp.clone());
        if let Some(crid) = identity.crid.clone() {
            self.by_crid.insert(crid, fp.clone());
        }
        if let Some(row) = self.by_fingerprint.get_mut(&fp) {
            let mut evidence = evidence;
            evidence.identity = row.identity.clone();
            row.evidence = evidence;
            return;
        }
        self.by_fingerprint.insert(
            fp.clone(),
            StoredCreative {
                fingerprint: fp,
                identity,
                evidence,
            },
        );
    }

    pub fn from_json(raw: &str) -> Result<Self, String> {
        if raw.trim().is_empty() {
            return Ok(Self::new());
        }
        let rows: Vec<StoredCreative> = serde_json::from_str(raw).map_err(|e| e.to_string())?;
        let mut store = Self::new();
        for row in rows {
            store.insert(row.identity, row.evidence);
        }
        Ok(store)
    }

    pub fn to_json(&self) -> Result<String, String> {
        let rows: Vec<&StoredCreative> = self.by_fingerprint.values().collect();
        serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Self::from_json(&raw)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = self.to_json()?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        std::fs::write(path, json).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        evaluate, evaluate_xml, CreativeIdentity, EvaluateRequest, OperationalClaims, Pack,
    };

    fn claims_ok() -> OperationalClaims {
        serde_json::from_str(include_str!("../../../fixtures/claims-ok.json")).unwrap()
    }

    #[test]
    fn same_media_two_crids_inherits() {
        let a = include_str!("../../../fixtures/ok-mezzanine.xml");
        let b = include_str!("../../../fixtures/ok-mezzanine-b.xml");
        let mut store = IdentityStore::new();
        let first = evaluate(EvaluateRequest {
            vast_xml: a,
            pack: Pack::LiveSsai,
            authority_id: "local",
            claims: Some(claims_ok()),
            ..Default::default()
        });
        assert_eq!(first.identity.crid.as_deref(), Some("cr-mezz"));
        assert_eq!(first.dimensions.operational, DimensionState::Ready);
        store.insert(first.identity.clone(), first.clone());

        let second = crate::evaluate_with_store(
            EvaluateRequest {
                vast_xml: b,
                pack: Pack::LiveSsai,
                authority_id: "local",
                claims: None,
                ..Default::default()
            },
            &store,
        );
        assert_eq!(second.identity.crid.as_deref(), Some("cr-mezz-b"));
        assert_eq!(second.dimensions.operational, DimensionState::Ready);
        assert_eq!(second.lifecycle, Lifecycle::OperationallyReady);
        assert!(second
            .findings
            .iter()
            .any(|f| f.requirement_id == "identity.reused"));
        assert_eq!(
            media_fingerprint(&first.identity),
            media_fingerprint(&second.identity)
        );
    }

    #[test]
    fn new_media_same_crid_does_not_inherit() {
        let a = include_str!("../../../fixtures/ok-mezzanine.xml");
        let b = include_str!("../../../fixtures/ok-mezzanine-new-media.xml");
        let mut store = IdentityStore::new();
        let first = evaluate(EvaluateRequest {
            vast_xml: a,
            pack: Pack::LiveSsai,
            authority_id: "local",
            claims: Some(claims_ok()),
            ..Default::default()
        });
        store.insert(first.identity.clone(), first.clone());

        let second = crate::evaluate_with_store(
            EvaluateRequest {
                vast_xml: b,
                pack: Pack::LiveSsai,
                authority_id: "local",
                claims: None,
                ..Default::default()
            },
            &store,
        );
        assert_eq!(second.identity.crid.as_deref(), Some("cr-mezz"));
        assert_eq!(second.dimensions.operational, DimensionState::NotReady);
        assert!(second
            .findings
            .iter()
            .any(|f| f.requirement_id == "identity.media-changed"));
        assert!(!second
            .findings
            .iter()
            .any(|f| f.requirement_id == "identity.reused"));
        match store.lookup(&second.identity) {
            LookupHit::MediaChanged { .. } => {}
            other => panic!("expected media change, got {other:?}"),
        }
    }

    #[test]
    fn content_hash_match_beats_url() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut store = IdentityStore::new();
        let first = evaluate(EvaluateRequest {
            vast_xml: xml,
            pack: Pack::None,
            authority_id: "local",
            claims: None,
            prepared_media: None,
            content_hash: Some("abc123".into()),
            ..Default::default()
        });
        store.insert(first.identity.clone(), first.clone());
        let mut other = evaluate_xml(xml, Pack::None);
        other.identity.crid = Some("other".into());
        other.identity.content_hash = Some("abc123".into());
        other.identity.media_url = Some("https://cdn.example.com/other.mp4".into());
        match store.lookup(&other.identity) {
            LookupHit::Known { .. } => {}
            other => panic!("expected known, got {other:?}"),
        }
    }

    #[test]
    fn lookup_crid_and_fallback_and_review_queue() {
        let xml = include_str!("../../../fixtures/ok-mezzanine.xml");
        let mut store = IdentityStore::new();
        let ready = evaluate(EvaluateRequest {
            vast_xml: xml,
            pack: Pack::LiveSsai,
            claims: Some(claims_ok()),
            ..Default::default()
        });
        store.insert(ready.identity.clone(), ready.clone());
        match store.lookup_crid("cr-mezz") {
            LookupHit::Known { evidence } => {
                assert_eq!(evidence.identity.crid.as_deref(), Some("cr-mezz"));
            }
            other => panic!("expected known, got {other:?}"),
        }
        let ctx = crate::AuctionContext {
            fallback_crid: Some("cr-mezz".into()),
            ..Default::default()
        };
        match store.lookup_fallback(&ctx) {
            LookupHit::Known { .. } => {}
            other => panic!("expected known fallback, got {other:?}"),
        }
        assert!(store.review_queue().is_empty());
        let pending = crate::observe(crate::Observation {
            vast_xml: Some(include_str!("../../../fixtures/ok-https-mp4.xml").into()),
            identity: None,
            pack: Pack::None,
            treatment: crate::SellerTreatment::Defer,
            pattern: Some("A".into()),
            won: Some(false),
            authority_id: None,
            auction_context: None,
            openrtb_json: None,
            overlays: Vec::new(),
        });
        store.insert(pending.identity.clone(), pending.clone());
        let q = store.review_queue();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].lifecycle, Lifecycle::PendingReview);
    }

    #[test]
    fn same_fingerprint_keeps_first_identity() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut store = IdentityStore::new();
        let mut first = evaluate_xml(xml, Pack::None);
        first.identity.crid = Some("cr-a".into());
        store.insert(first.identity.clone(), first.clone());
        let mut second = evaluate_xml(xml, Pack::None);
        second.identity.crid = Some("cr-b".into());
        store.insert(second.identity.clone(), second);
        match store.lookup_crid("cr-a") {
            LookupHit::Known { evidence } => {
                assert_eq!(evidence.identity.crid.as_deref(), Some("cr-a"));
            }
            other => panic!("expected known, got {other:?}"),
        }
        match store.lookup_crid("cr-b") {
            LookupHit::Known { evidence } => {
                assert_eq!(evidence.identity.crid.as_deref(), Some("cr-a"));
            }
            other => panic!("expected known, got {other:?}"),
        }
    }

    #[test]
    fn fingerprint_includes_universal_ad_id() {
        let mut a = CreativeIdentity {
            media_url: Some("https://cdn.example.com/ad.mp4".into()),
            universal_ad_id: Some("UAD-1".into()),
            ..Default::default()
        };
        let mut b = a.clone();
        b.universal_ad_id = Some("UAD-2".into());
        assert_ne!(media_fingerprint(&a), media_fingerprint(&b));
        assert!(media_fingerprint(&a).contains("|uad:UAD-1"));
        a.universal_ad_id = None;
        assert!(!media_fingerprint(&a).contains("|uad:"));
        a.universal_ad_id = Some("   ".into());
        assert!(!media_fingerprint(&a).contains("|uad:"));
    }
}
