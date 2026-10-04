//! Live-event creative readiness evaluate.
//!
//! Input: VAST XML plus a policy pack. Output: [`ReadinessEvidence`].
//! Technical findings come from `vastlint-core`. Packs `live-ssai`,
//! `live-sgai`, `live-csai`, and `live-hybrid` add environment requirements.

use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use vastlint_core::{Severity as VastSeverity, ValidationResult};

mod diff;
mod eligibility;
mod expiry;
mod gate;
mod media;
mod observe;
mod operational;
mod path;
mod policy;
mod store;
mod summary;
mod surface;
mod transform;

#[cfg(test)]
mod schema_catalog;

pub use diff::{apply_diff, diff_vast, diff_vast_with_policy, DiffResult, Invalidation};
pub use eligibility::{check_eligibility, eligibility, Eligibility, Opportunity, ReadyRef};
pub use expiry::{apply_clock, apply_pull, fail_closed, revalidate};
pub use gate::{gate, prebid_gate, BidGate};
pub use media::{media_facts, media_url, MediaFacts};
pub use observe::{observe, parse_openrtb_lite, AuctionContext, Observation, SellerTreatment};
pub use operational::{
    apply_claims, apply_prepared_media, OperationalClaims, PreparedMedia, PreparedRendition,
};
pub use path::{
    evaluate_authorities, evaluate_paths, evaluate_paths_ctx, AuthoritySpec, EnvironmentResult,
    EnvironmentSpec, PathContext, SgaiSplit,
};
pub use policy::{
    apply_overlay, apply_overlay_for, apply_policy, apply_policy_for, builtin_policy_ids,
    default_invalidation_rules, discover_policy, invalidation_rules, pack_expired, parse_overlay,
    parse_overlays, parse_policy, resolve_overlays, select_overlays, AuctionTriggered,
    InvalidationRule, PolicyDocument, PolicyEnvironment, PolicyOverlay, PolicyRequirement,
    TranscodeLadder, TransformationRights,
};
pub use store::{apply_reuse, media_fingerprint, IdentityStore, LookupHit, StoredCreative};
pub use summary::{summarize, ReadinessSummary};
pub use surface::{module_surface, CapabilitySurface};
pub use transform::{transform, TransformationKind};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Pack {
    #[default]
    None,
    LiveSsai,
    LiveSgai,
    LiveCsai,
    LiveHybrid,
}

impl Pack {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" => Ok(Pack::None),
            "live-ssai" => Ok(Pack::LiveSsai),
            "live-sgai" => Ok(Pack::LiveSgai),
            "live-csai" => Ok(Pack::LiveCsai),
            "live-hybrid" => Ok(Pack::LiveHybrid),
            other => Err(format!(
                "unknown pack {other:?}; expected none, live-ssai, live-sgai, live-csai, or live-hybrid"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Pack::None => "none",
            Pack::LiveSsai => "live-ssai",
            Pack::LiveSgai => "live-sgai",
            Pack::LiveCsai => "live-csai",
            Pack::LiveHybrid => "live-hybrid",
        }
    }

    pub(crate) fn policy(self) -> Option<PolicyDocument> {
        match self {
            Pack::None => None,
            Pack::LiveSsai => Some(discover_policy("live-ssai").expect("builtin live-ssai")),
            Pack::LiveSgai => Some(discover_policy("live-sgai").expect("builtin live-sgai")),
            Pack::LiveCsai => Some(discover_policy("live-csai").expect("builtin live-csai")),
            Pack::LiveHybrid => Some(discover_policy("live-hybrid").expect("builtin live-hybrid")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessEvidence {
    pub authority: Authority,
    pub scope: Scope,
    pub version: String,
    #[serde(with = "time::serde::rfc3339")]
    pub issued_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    pub identity: CreativeIdentity,
    pub dimensions: Dimensions,
    pub lifecycle: Lifecycle,
    pub findings: Vec<Finding>,
    pub entry_path: EntryPath,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepared_media: Option<PreparedMedia>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operational_claims: Option<OperationalClaims>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seller_treatment: Option<SellerTreatment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unknown_creative_treatment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environments: Vec<EnvironmentResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facts: Option<MediaFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sgai: Option<SgaiSplit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auction_context: Option<AuctionContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Authority {
    pub id: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acts_on_behalf_of: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scope {
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreativeIdentity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub universal_ad_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dsp_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssp_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mezzanine_url: Option<String>,
}

impl CreativeIdentity {
    /// Blank and whitespace-only fields become None.
    pub fn normalize(&mut self) {
        fn blank(s: &mut Option<String>) {
            if let Some(v) = s {
                let t = v.trim();
                if t.is_empty() {
                    *s = None;
                } else if t != v.as_str() {
                    *s = Some(t.to_string());
                }
            }
        }
        blank(&mut self.crid);
        blank(&mut self.universal_ad_id);
        blank(&mut self.media_url);
        blank(&mut self.content_hash);
        blank(&mut self.fingerprint);
        blank(&mut self.version);
        blank(&mut self.dsp_id);
        blank(&mut self.ssp_id);
        blank(&mut self.mid);
        blank(&mut self.mezzanine_url);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimensions {
    pub policy: DimensionState,
    pub technical: DimensionState,
    pub operational: DimensionState,
    pub contextual: DimensionState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionState {
    Unknown,
    Ready,
    NotReady,
    NotEvaluated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Unknown,
    Observed,
    PendingReview,
    Approved,
    Rejected,
    TechnicallyReady,
    OperationallyReady,
    Expired,
    RevalidationRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EntryPath {
    #[default]
    Proactive,
    AuctionTriggered,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Finding {
    pub requirement_id: String,
    pub severity: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "time::serde::rfc3339::option"
    )]
    pub issued_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct EvaluateRequest<'a> {
    pub vast_xml: &'a str,
    pub pack: Pack,
    pub authority_id: &'a str,
    pub claims: Option<OperationalClaims>,
    pub prepared_media: Option<PreparedMedia>,
    pub content_hash: Option<String>,
    pub overlays: Vec<PolicyOverlay>,
    pub entry_path: EntryPath,
    pub acts_on_behalf_of: Vec<String>,
    pub authority_role: Option<String>,
    pub transaction: Option<String>,
    pub device_family: Option<String>,
    pub event_id: Option<String>,
    pub deal_id: Option<String>,
    pub publisher_id: Option<String>,
    /// W9: a PolicyDocument loaded from disk or URI. Wins over builtin pack requirements.
    pub policy: Option<PolicyDocument>,
}

impl Default for EvaluateRequest<'_> {
    fn default() -> Self {
        Self {
            vast_xml: "",
            pack: Pack::None,
            authority_id: "local",
            claims: None,
            prepared_media: None,
            content_hash: None,
            overlays: Vec::new(),
            entry_path: EntryPath::Proactive,
            acts_on_behalf_of: Vec::new(),
            authority_role: None,
            transaction: None,
            device_family: None,
            event_id: None,
            deal_id: None,
            publisher_id: None,
            policy: None,
        }
    }
}

pub fn evaluate(req: EvaluateRequest<'_>) -> ReadinessEvidence {
    let issued = OffsetDateTime::now_utc();
    let vast: ValidationResult = vastlint_core::validate(req.vast_xml);
    let mut findings: Vec<Finding> = vast
        .issues
        .iter()
        .map(|issue| Finding {
            requirement_id: issue.id.to_string(),
            severity: issue.severity.as_str().to_string(),
            message: issue.message.to_string(),
            remediable: Some(issue.severity != VastSeverity::Error),
            path: issue.path.clone(),
            dimension: Some("technical".into()),
            ..Default::default()
        })
        .collect();

    let policy = req.policy.clone().or_else(|| req.pack.policy());
    let device = eligibility::nonempty_str(req.device_family.as_deref());
    if let Some(ref policy) = policy {
        if pack_expired(policy, issued) {
            findings.push(Finding {
                requirement_id: "policy.pack-expired".into(),
                severity: "error".into(),
                message: format!("policy pack {} is past expires_at; fail closed.", policy.id),
                remediable: Some(true),
                dimension: Some("policy".into()),
                ..Default::default()
            });
        }
        apply_policy_for(policy, req.vast_xml, device, &mut findings);
    }
    let opp = Opportunity {
        deal_id: eligibility::nonempty_owned(req.deal_id.clone()),
        publisher_id: eligibility::nonempty_owned(req.publisher_id.clone()),
        event_id: eligibility::nonempty_owned(req.event_id.clone()),
        device_family: eligibility::nonempty_owned(req.device_family.clone()),
        ..Default::default()
    };
    let mut overlays = policy
        .as_ref()
        .map(|p| select_overlays(&p.overlays, &opp))
        .unwrap_or_default();
    overlays.extend(req.overlays.iter().cloned());
    let mut seen_ids = Vec::new();
    overlays.retain(|o| {
        if seen_ids.iter().any(|id| id == &o.id) {
            false
        } else {
            seen_ids.push(o.id.clone());
            true
        }
    });
    for overlay in &overlays {
        apply_overlay_for(overlay, req.vast_xml, device, &mut findings);
        if overlay.auction_triggered == Some(AuctionTriggered::Forbidden)
            && req.entry_path == EntryPath::AuctionTriggered
        {
            findings.push(Finding {
                requirement_id: format!("{}.auction-triggered-forbidden", overlay.id),
                severity: "error".into(),
                message: format!(
                    "deal overlay {} forbids auction-triggered entry.",
                    overlay.id
                ),
                remediable: Some(true),
                path: None,
                environment: None,
                publisher: eligibility::nonempty_owned(overlay.publisher_id.clone()),
                deal: eligibility::nonempty_owned(overlay.deal_id.clone()),
                dimension: Some("policy".into()),
                ..Default::default()
            });
        }
    }

    let mut operational = DimensionState::Unknown;
    if let Some(ref media) = req.prepared_media {
        operational = apply_prepared_media(
            policy.as_ref().and_then(|p| p.transcode_ladder.as_ref()),
            media,
            &mut findings,
        );
    } else if let Some(ref claims) = req.claims {
        operational = apply_claims(
            policy.as_ref().and_then(|p| p.transcode_ladder.as_ref()),
            claims,
            &mut findings,
        );
    } else if policy.as_ref().is_some_and(|p| p.proactive_required) {
        findings.push(Finding {
            requirement_id: "operational.prep-skipped".into(),
            severity: "error".into(),
            message: "policy requires proactive preparation; auction-triggered skip is incomplete."
                .into(),
            remediable: Some(true),
            dimension: Some("operational".into()),
            ..Default::default()
        });
        operational = DimensionState::NotReady;
    }

    let policy_state = if findings
        .iter()
        .any(|f| f.severity == "error" && f.dimension.as_deref() == Some("policy"))
    {
        DimensionState::NotReady
    } else if policy.is_some() {
        DimensionState::Ready
    } else {
        DimensionState::Unknown
    };

    let technical = if findings.iter().any(|f| {
        f.severity == "error"
            && f.dimension.as_deref() != Some("policy")
            && f.dimension.as_deref() != Some("operational")
            && !f.requirement_id.starts_with("operational.")
    }) {
        DimensionState::NotReady
    } else {
        DimensionState::Ready
    };

    let lifecycle = if operational == DimensionState::Ready
        && technical == DimensionState::Ready
        && policy_state != DimensionState::NotReady
    {
        Lifecycle::OperationallyReady
    } else if technical == DimensionState::Ready && policy_state != DimensionState::NotReady {
        Lifecycle::TechnicallyReady
    } else {
        Lifecycle::Unknown
    };

    let facts = media_facts(req.vast_xml);
    let sgai = matches!(req.pack, Pack::LiveSgai | Pack::LiveHybrid).then(|| {
        crate::path::sgai_split(
            req.vast_xml,
            technical == DimensionState::Ready && policy_state != DimensionState::NotReady,
            req.claims.as_ref(),
        )
    });

    for f in &mut findings {
        if f.issued_at.is_none() {
            f.issued_at = Some(issued);
        }
    }

    ReadinessEvidence {
        authority: Authority {
            id: req.authority_id.to_string(),
            role: req.authority_role.clone().unwrap_or_else(|| "other".into()),
            acts_on_behalf_of: req.acts_on_behalf_of.clone(),
        },
        scope: evidence_scope(&req, policy.as_ref()),
        version: VERSION.into(),
        issued_at: issued,
        expires_at: issued + Duration::hours(24),
        identity: extract_identity(req.vast_xml, req.content_hash.clone()),
        dimensions: Dimensions {
            policy: policy_state,
            technical,
            operational,
            contextual: DimensionState::NotEvaluated,
        },
        lifecycle,
        findings,
        entry_path: req.entry_path,
        prepared_media: req.prepared_media,
        operational_claims: req.claims.clone(),
        seller_treatment: None,
        unknown_creative_treatment: policy
            .as_ref()
            .and_then(|p| p.unknown_creative_treatment.clone()),
        pattern: None,
        environments: Vec::new(),
        facts: Some(facts),
        sgai,
        transaction: req.transaction.clone(),
        auction_context: None,
    }
}

fn evidence_scope(req: &EvaluateRequest<'_>, policy: Option<&PolicyDocument>) -> Scope {
    if let Some(id) = eligibility::nonempty_str(req.deal_id.as_deref()) {
        return Scope {
            kind: "deal".into(),
            id: id.into(),
        };
    }
    if let Some(id) = eligibility::nonempty_str(req.event_id.as_deref()) {
        return Scope {
            kind: "event".into(),
            id: id.into(),
        };
    }
    if let Some(id) = eligibility::nonempty_str(req.publisher_id.as_deref()) {
        return Scope {
            kind: "publisher".into(),
            id: id.into(),
        };
    }
    Scope {
        kind: "environment".into(),
        id: policy
            .map(|p| p.id.clone())
            .unwrap_or_else(|| req.pack.as_str().into()),
    }
}

pub fn evaluate_xml(xml: &str, pack: Pack) -> ReadinessEvidence {
    evaluate(EvaluateRequest {
        vast_xml: xml,
        pack,
        ..Default::default()
    })
}

pub fn evaluate_with_store(req: EvaluateRequest<'_>, store: &IdentityStore) -> ReadinessEvidence {
    let mut ev = evaluate(req);
    match store.lookup(&ev.identity) {
        LookupHit::Known { evidence } => apply_reuse(&mut ev, &evidence),
        LookupHit::MediaChanged {
            previous_fingerprint,
        } => {
            ev.findings.push(Finding {
                requirement_id: "identity.media-changed".into(),
                severity: "warning".into(),
                message: format!(
                    "same crid now maps to a different media fingerprint (was {previous_fingerprint})."
                ),
                remediable: Some(true),
                ..Default::default()
            });
        }
        LookupHit::Unknown => {}
    }
    ev
}

fn extract_identity(xml: &str, content_hash: Option<String>) -> CreativeIdentity {
    let mut identity = CreativeIdentity {
        crid: media::attr_value(xml, "Creative", "id"),
        universal_ad_id: media::universal_ad_id_text(xml),
        media_url: media_url(xml),
        content_hash,
        fingerprint: None,
        version: Some("1".into()),
        dsp_id: None,
        ssp_id: None,
        mid: media::ad_serving_id_text(xml),
        mezzanine_url: media::mezzanine_url(xml),
    };
    identity.normalize();
    let fp = media_fingerprint(&identity);
    if !fp.is_empty() {
        identity.fingerprint = Some(fp);
    }
    identity
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(name: &str) -> &'static str {
        match name {
            "ok-https-mp4.xml" => include_str!("../../../fixtures/ok-https-mp4.xml"),
            "ok-mezzanine.xml" => include_str!("../../../fixtures/ok-mezzanine.xml"),
            "vpaid.xml" => include_str!("../../../fixtures/vpaid.xml"),
            _ => panic!("unknown fixture {name}"),
        }
    }

    #[test]
    fn spec_valid_pack_none_is_technically_ready() {
        let xml = load("ok-https-mp4.xml");
        let ev = evaluate_xml(xml, Pack::None);
        assert_eq!(ev.dimensions.technical, DimensionState::Ready);
        assert_eq!(ev.lifecycle, Lifecycle::TechnicallyReady);
        assert!(ev.findings.iter().all(|f| f.severity != "error"));
        assert_eq!(ev.identity.universal_ad_id.as_deref(), Some("ABCD1234567"));
        assert_eq!(ev.identity.mid.as_deref(), Some("srv-001"));
        assert_eq!(
            ev.identity.media_url.as_deref(),
            Some("https://cdn.example.com/ad.mp4")
        );
        assert!(ev
            .identity
            .fingerprint
            .as_deref()
            .unwrap()
            .starts_with("url:"));
        let json = serde_json::to_value(&ev).unwrap();
        assert!(json.get("authority").is_some());
        assert!(json.get("findings").is_some());
        crate::schema_catalog::assert_schema("readiness-evidence.json", &json);
    }

    #[test]
    fn empty_adservingid_is_not_mid() {
        let xml = load("ok-https-mp4.xml").replace(">srv-001</AdServingId>", "></AdServingId>");
        let ev = evaluate_xml(&xml, Pack::None);
        assert_eq!(ev.identity.mid, None);
    }

    #[test]
    fn empty_creative_id_is_not_crid() {
        let xml = load("ok-mezzanine.xml").replace("id=\"cr-mezz\"", "id=\"   \"");
        let ev = evaluate_xml(&xml, Pack::None);
        assert_eq!(ev.identity.crid, None);
    }

    #[test]
    fn later_creative_id_fills_crid() {
        let xml = load("ok-https-mp4.xml").replace(
            "<Creative>",
            "<Creative id=\"\"></Creative><Creative id=\"cr-2\">",
        );
        let ev = evaluate_xml(&xml, Pack::None);
        assert_eq!(ev.identity.crid.as_deref(), Some("cr-2"));
    }

    #[test]
    fn later_adservingid_fills_mid() {
        let xml = load("ok-https-mp4.xml").replace(
            ">srv-001</AdServingId>",
            "></AdServingId><AdServingId>srv-2</AdServingId>",
        );
        let ev = evaluate_xml(&xml, Pack::None);
        assert_eq!(ev.identity.mid.as_deref(), Some("srv-2"));
    }

    #[test]
    fn id_value_fills_universal_ad_id() {
        let xml = load("ok-https-mp4.xml").replace(
            "<UniversalAdId idRegistry=\"ad-id.org\">ABCD1234567</UniversalAdId>",
            "<UniversalAdId idRegistry=\"ad-id.org\" idValue=\"FROM-ATTR\"/>",
        );
        let ev = evaluate_xml(&xml, Pack::None);
        assert_eq!(ev.identity.universal_ad_id.as_deref(), Some("FROM-ATTR"));
        let later = load("ok-https-mp4.xml").replace(
            "<UniversalAdId idRegistry=\"ad-id.org\">ABCD1234567</UniversalAdId>",
            "<UniversalAdId idRegistry=\"ad-id.org\"></UniversalAdId><UniversalAdId>UAD-2</UniversalAdId>",
        );
        let ev = evaluate_xml(&later, Pack::None);
        assert_eq!(ev.identity.universal_ad_id.as_deref(), Some("UAD-2"));
    }

    #[test]
    fn live_ssai_rejects_missing_mezzanine() {
        let xml = load("ok-https-mp4.xml");
        let ev = evaluate_xml(xml, Pack::LiveSsai);
        assert_eq!(ev.dimensions.policy, DimensionState::NotReady);
        assert_eq!(ev.dimensions.technical, DimensionState::Ready);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "live-ssai.mezzanine-required"));
        assert_eq!(ev.facts.as_ref().unwrap().duration_seconds, Some(30));
    }

    #[test]
    fn live_ssai_rejects_empty_mezzanine() {
        let xml = load("ok-mezzanine.xml").replace(
            "<![CDATA[https://cdn.example.com/mezz.mp4]]>",
            "<![CDATA[]]>",
        );
        let ev = evaluate_xml(&xml, Pack::LiveSsai);
        assert_eq!(ev.dimensions.policy, DimensionState::NotReady);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "live-ssai.mezzanine-required"));
    }

    #[test]
    fn live_ssai_rejects_http_mezzanine() {
        let xml = load("ok-mezzanine.xml").replace(
            "https://cdn.example.com/mezz.mp4",
            "http://cdn.example.com/mezz.mp4",
        );
        let ev = evaluate_xml(&xml, Pack::LiveSsai);
        assert_eq!(ev.dimensions.policy, DimensionState::NotReady);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "live-ssai.media-https"));
    }

    #[test]
    fn live_ssai_rejects_missing_universaladid() {
        let xml = load("ok-mezzanine.xml").replace(
            "<UniversalAdId idRegistry=\"ad-id.org\">ABCD1234567</UniversalAdId>",
            "",
        );
        let ev = evaluate_xml(&xml, Pack::LiveSsai);
        assert_eq!(ev.dimensions.policy, DimensionState::NotReady);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "live-ssai.universaladid-required"));
    }

    #[test]
    fn live_ssai_rejects_empty_universaladid() {
        let xml =
            load("ok-mezzanine.xml").replace(">ABCD1234567</UniversalAdId>", "></UniversalAdId>");
        let ev = evaluate_xml(&xml, Pack::LiveSsai);
        assert_eq!(ev.dimensions.policy, DimensionState::NotReady);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "live-ssai.universaladid-required"));
    }

    #[test]
    fn live_ssai_accepts_id_value_universaladid() {
        let xml = load("ok-mezzanine.xml").replace(
            "<UniversalAdId idRegistry=\"ad-id.org\">ABCD1234567</UniversalAdId>",
            "<UniversalAdId idRegistry=\"ad-id.org\" idValue=\"ABCD1234567\"/>",
        );
        let ev = evaluate_xml(&xml, Pack::LiveSsai);
        assert!(!ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "live-ssai.universaladid-required"));
        assert_eq!(ev.identity.universal_ad_id.as_deref(), Some("ABCD1234567"));
    }

    #[test]
    fn live_sgai_empty_universaladid_is_not_signaling_ready() {
        let xml = load("ok-https-mp4.xml")
            .replace(">ABCD1234567</UniversalAdId>", ">   </UniversalAdId>");
        let ev = evaluate_xml(&xml, Pack::LiveSgai);
        assert_eq!(ev.dimensions.policy, DimensionState::NotReady);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "live-sgai.universaladid-required"));
        let split = ev.sgai.unwrap();
        assert_eq!(split.signaling, DimensionState::NotReady);
    }

    #[test]
    fn live_sgai_empty_adservingid_is_not_signaling_ready() {
        let xml = load("ok-https-mp4.xml").replace(">srv-001</AdServingId>", "></AdServingId>");
        let ev = evaluate_xml(&xml, Pack::LiveSgai);
        assert_eq!(ev.dimensions.policy, DimensionState::NotReady);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "live-sgai.adservingid-required"));
        let split = ev.sgai.unwrap();
        assert_eq!(split.signaling, DimensionState::NotReady);
    }

    #[test]
    fn live_ssai_rejects_vpaid() {
        let xml = load("vpaid.xml");
        let ev = evaluate_xml(xml, Pack::LiveSsai);
        assert_eq!(ev.dimensions.policy, DimensionState::NotReady);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "live-ssai.vpaid-forbidden"));
    }

    #[test]
    fn live_ssai_evaluate_without_prep_is_operationally_incomplete() {
        let xml = load("ok-mezzanine.xml");
        let ev = evaluate_xml(xml, Pack::LiveSsai);
        assert_eq!(ev.dimensions.technical, DimensionState::Ready);
        assert_eq!(ev.dimensions.policy, DimensionState::Ready);
        assert_eq!(ev.dimensions.operational, DimensionState::NotReady);
        assert_eq!(
            ev.unknown_creative_treatment.as_deref(),
            Some("informational")
        );
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "operational.prep-skipped"));
    }

    #[test]
    fn live_sgai_allows_missing_mezzanine() {
        let xml = load("ok-https-mp4.xml");
        let ev = evaluate_xml(xml, Pack::LiveSgai);
        assert_eq!(ev.dimensions.technical, DimensionState::Ready);
        assert_eq!(ev.dimensions.policy, DimensionState::Ready);
        assert!(!ev.findings.iter().any(|f| f.severity == "error"));
        let split = ev.sgai.unwrap();
        assert_eq!(split.server, DimensionState::Ready);
        assert_eq!(split.signaling, DimensionState::Ready);
        assert_eq!(split.client_capability, DimensionState::NotEvaluated);
        assert_eq!(split.execution, DimensionState::NotEvaluated);
    }

    #[test]
    fn live_csai_allows_missing_mezzanine() {
        let xml = load("ok-https-mp4.xml");
        let ev = evaluate_xml(xml, Pack::LiveCsai);
        assert_eq!(ev.dimensions.policy, DimensionState::Ready);
        assert_eq!(ev.dimensions.technical, DimensionState::Ready);
        assert!(ev.sgai.is_none());
    }

    #[test]
    fn live_sgai_client_ready_is_end_to_end() {
        let xml = load("ok-https-mp4.xml");
        let ev = evaluate(EvaluateRequest {
            vast_xml: xml,
            pack: Pack::LiveSgai,
            claims: Some(OperationalClaims {
                client_ready: Some(true),
                transcoded: Some(true),
                rendition_count: Some(0),
                ..Default::default()
            }),
            ..Default::default()
        });
        let split = ev.sgai.unwrap();
        assert_eq!(split.execution, DimensionState::Ready);
    }

    #[test]
    fn live_sgai_rejects_vpaid() {
        let xml = load("vpaid.xml");
        let ev = evaluate_xml(xml, Pack::LiveSgai);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "live-sgai.vpaid-forbidden"));
    }

    #[test]
    fn claims_only_fixture_evaluates_operational_without_ffmpeg() {
        let xml = load("ok-mezzanine.xml");
        let claims: OperationalClaims =
            serde_json::from_str(include_str!("../../../fixtures/claims-ok.json")).unwrap();
        let ev = evaluate(EvaluateRequest {
            vast_xml: xml,
            pack: Pack::LiveSsai,
            claims: Some(claims),
            ..Default::default()
        });
        assert_eq!(ev.dimensions.technical, DimensionState::Ready);
        assert_eq!(ev.dimensions.operational, DimensionState::Ready);
        assert_eq!(ev.lifecycle, Lifecycle::OperationallyReady);
        crate::schema_catalog::assert_schema(
            "readiness-evidence.json",
            &serde_json::to_value(&ev).unwrap(),
        );
    }

    #[test]
    fn incomplete_claims_do_not_fail_technical() {
        let xml = load("ok-mezzanine.xml");
        let claims: OperationalClaims =
            serde_json::from_str(include_str!("../../../fixtures/claims-incomplete.json")).unwrap();
        let ev = evaluate(EvaluateRequest {
            vast_xml: xml,
            pack: Pack::LiveSsai,
            claims: Some(claims),
            ..Default::default()
        });
        assert_eq!(ev.dimensions.technical, DimensionState::Ready);
        assert_eq!(ev.dimensions.operational, DimensionState::NotReady);
        assert_eq!(ev.lifecycle, Lifecycle::TechnicallyReady);
    }

    #[test]
    fn live_hybrid_requires_mezzanine_and_records_url() {
        let xml = load("ok-mezzanine.xml");
        let ev = evaluate_xml(xml, Pack::LiveHybrid);
        assert_eq!(ev.dimensions.policy, DimensionState::Ready);
        assert_eq!(ev.dimensions.technical, DimensionState::Ready);
        assert_eq!(ev.dimensions.operational, DimensionState::NotReady);
        assert_eq!(
            ev.identity.mezzanine_url.as_deref(),
            Some("https://cdn.example.com/mezz.mp4")
        );
        assert!(ev.sgai.is_some());
        let xml = load("ok-https-mp4.xml");
        let ev = evaluate_xml(xml, Pack::LiveHybrid);
        assert_eq!(ev.dimensions.policy, DimensionState::NotReady);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "live-hybrid.mezzanine-required"));
    }

    #[test]
    fn prepared_media_fixture_marks_operational_ready() {
        let xml = load("ok-mezzanine.xml");
        let media: PreparedMedia =
            serde_json::from_str(include_str!("../../../fixtures/prepared-ok.json")).unwrap();
        let ev = evaluate(EvaluateRequest {
            vast_xml: xml,
            pack: Pack::LiveSsai,
            prepared_media: Some(media),
            ..Default::default()
        });
        assert_eq!(ev.dimensions.operational, DimensionState::Ready);
        assert_eq!(ev.lifecycle, Lifecycle::OperationallyReady);
        assert!(ev.prepared_media.is_some());
    }

    #[test]
    fn custom_expired_pack_fails_closed() {
        let policy = parse_policy(include_str!("../../../fixtures/expired-pack.json")).unwrap();
        let ev = evaluate(EvaluateRequest {
            vast_xml: load("ok-https-mp4.xml"),
            pack: Pack::None,
            policy: Some(policy),
            ..Default::default()
        });
        assert_eq!(ev.scope.kind, "environment");
        assert_eq!(ev.scope.id, "expired-pack");
        assert_eq!(ev.dimensions.policy, DimensionState::NotReady);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "policy.pack-expired"));
    }

    #[test]
    fn pack_overlays_select_event_and_deal_forbids_auction() {
        let mut policy = discover_policy("live-csai").unwrap();
        policy.overlays =
            serde_json::from_str(include_str!("../../../fixtures/overlays/catalog.json")).unwrap();
        let sports = evaluate(EvaluateRequest {
            vast_xml: load("ok-https-mp4.xml"),
            pack: Pack::LiveCsai,
            policy: Some(policy.clone()),
            event_id: Some("game-1".into()),
            ..Default::default()
        });
        assert_eq!(sports.scope.kind, "event");
        assert_eq!(sports.scope.id, "game-1");
        assert!(sports
            .findings
            .iter()
            .any(|f| f.requirement_id == "event-sports.max-duration"));
        let news = evaluate(EvaluateRequest {
            vast_xml: load("ok-https-mp4.xml"),
            pack: Pack::LiveCsai,
            policy: Some(policy.clone()),
            event_id: Some("news-1".into()),
            ..Default::default()
        });
        assert!(!news
            .findings
            .iter()
            .any(|f| f.requirement_id == "event-sports.max-duration"));
        let auction = evaluate(EvaluateRequest {
            vast_xml: load("ok-https-mp4.xml"),
            pack: Pack::LiveCsai,
            policy: Some(policy),
            deal_id: Some("deal-9".into()),
            entry_path: EntryPath::AuctionTriggered,
            ..Default::default()
        });
        assert_eq!(auction.scope.kind, "deal");
        assert!(auction
            .findings
            .iter()
            .any(|f| f.requirement_id == "deal-premium.auction-triggered-forbidden"));
    }

    #[test]
    fn leftover_empty_publisher_id_is_not_stamped_on_evaluate() {
        let overlay = PolicyOverlay {
            id: "blank-pub".into(),
            publisher_id: Some("".into()),
            auction_triggered: Some(AuctionTriggered::Forbidden),
            ..Default::default()
        };
        let ev = evaluate(EvaluateRequest {
            vast_xml: load("ok-https-mp4.xml"),
            pack: Pack::None,
            overlays: vec![overlay],
            entry_path: EntryPath::AuctionTriggered,
            ..Default::default()
        });
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "blank-pub.auction-triggered-forbidden"));
        assert!(ev.findings.iter().all(|f| f.publisher.is_none()));
    }

    #[test]
    fn device_overlay_without_device_fails_closed_on_evaluate() {
        let overlay =
            parse_overlay(include_str!("../../../fixtures/overlays/device-ctv.json")).unwrap();
        let miss = evaluate(EvaluateRequest {
            vast_xml: load("ok-https-mp4.xml"),
            pack: Pack::None,
            overlays: vec![overlay.clone()],
            ..Default::default()
        });
        assert!(miss
            .findings
            .iter()
            .any(|f| f.requirement_id == "device-ctv.family"));
        let hit = evaluate(EvaluateRequest {
            vast_xml: load("ok-https-mp4.xml"),
            pack: Pack::None,
            overlays: vec![overlay.clone()],
            device_family: Some("ctv".into()),
            ..Default::default()
        });
        assert!(!hit
            .findings
            .iter()
            .any(|f| f.requirement_id == "device-ctv.family"));
        let padded = evaluate(EvaluateRequest {
            vast_xml: load("ok-https-mp4.xml"),
            pack: Pack::None,
            overlays: vec![overlay],
            device_family: Some("  ctv ".into()),
            ..Default::default()
        });
        assert!(!padded
            .findings
            .iter()
            .any(|f| f.requirement_id == "device-ctv.family"));
    }

    #[test]
    fn reuse_does_not_mark_ready_when_policy_fails() {
        let xml = load("ok-https-mp4.xml");
        let mut store = IdentityStore::new();
        let first = evaluate_xml(xml, Pack::None);
        store.insert(first.identity.clone(), first);
        let reused = evaluate_with_store(
            EvaluateRequest {
                vast_xml: xml,
                pack: Pack::LiveSsai,
                ..Default::default()
            },
            &store,
        );
        assert_eq!(reused.dimensions.policy, DimensionState::NotReady);
        assert_ne!(reused.lifecycle, Lifecycle::OperationallyReady);
        assert_ne!(reused.lifecycle, Lifecycle::TechnicallyReady);
        assert!(reused
            .findings
            .iter()
            .any(|f| f.requirement_id == "identity.reused"));
    }
}
