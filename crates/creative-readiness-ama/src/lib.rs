//! AMA 1.1 adapter. AMA is one ingest API onto [`CapabilitySurface`], not the
//! Creative Readiness standard.

use std::collections::BTreeMap;

use creative_readiness_core::{
    evaluate_xml, module_surface, Authority, CapabilitySurface, CreativeIdentity, DimensionState,
    Dimensions, EntryPath, Finding, Lifecycle, Pack, ReadinessEvidence, Scope, VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use time::{Duration, OffsetDateTime};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdaptOptions {
    /// Map AMA 1.2 publisher/deal fields onto evidence scope. Additive.
    #[serde(default)]
    pub experimental: bool,
}

pub fn parse_ama_value(value: &Value) -> Result<Vec<AmaAd>, String> {
    if let Some(arr) = value.get("ads").and_then(|a| a.as_array()) {
        return arr
            .iter()
            .map(|v| serde_json::from_value(v.clone()).map_err(|e| e.to_string()))
            .collect();
    }
    let ad: AmaAd = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    Ok(vec![ad])
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AmaAd {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit: Option<AmaAudit>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AmaAudit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback: Option<AuditFeedback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub init: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lastmod: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corr: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deal_id: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AuditFeedback {
    One(String),
    Many(Vec<String>),
}

impl AuditFeedback {
    pub fn lines(&self) -> Vec<String> {
        match self {
            AuditFeedback::One(s) => vec![s.clone()],
            AuditFeedback::Many(v) => v.clone(),
        }
    }
}

pub fn ama_1_1_surface() -> CapabilitySurface {
    let mut s = module_surface();
    s.known_creative_lookup = false;
    s.deal_publisher_environment = false;
    s.fallback_lookup = false;
    s.review_queue = false;
    s.overlay_select = false;
    s
}

/// Experimental AMA 1.2 candidate: lookup + publisher/deal on audit.
pub fn ama_1_2_surface() -> CapabilitySurface {
    let mut s = ama_1_1_surface();
    s.known_creative_lookup = true;
    s.deal_publisher_environment = true;
    s.fallback_lookup = true;
    s
}

pub fn parse_ama(json: &str) -> Result<Vec<AmaAd>, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    parse_ama_value(&value)
}

pub fn adapt_ama(json: &str, opts: AdaptOptions) -> Result<Vec<ReadinessEvidence>, String> {
    Ok(parse_ama(json)?
        .into_iter()
        .map(|ad| adapt_ad(&ad, &opts))
        .collect())
}

pub fn adapt_ad(ad: &AmaAd, opts: &AdaptOptions) -> ReadinessEvidence {
    let audit = ad.audit.clone().unwrap_or_default();
    let (lifecycle, policy) = map_audit_status(audit.status);
    let mut findings = Vec::new();
    if let Some(fb) = &audit.feedback {
        for line in fb.lines() {
            findings.push(Finding {
                requirement_id: format!("ama.audit.status-{}", audit.status.unwrap_or(0)),
                severity: if matches!(audit.status, Some(4)) {
                    "error".into()
                } else {
                    "info".into()
                },
                message: line,
                remediable: Some(true),
                path: Some("audit.feedback".into()),
                ..Default::default()
            });
        }
    }
    if audit.corr.is_some() {
        findings.push(Finding {
            requirement_id: "ama.audit.corrected".into(),
            severity: "info".into(),
            message: "auditor supplied a correction object on the AMA audit.".into(),
            remediable: Some(false),
            path: Some("audit.corr".into()),
            ..Default::default()
        });
    }

    let mut technical = DimensionState::NotEvaluated;
    let mut operational = DimensionState::NotEvaluated;
    let mut extra_findings = Vec::new();
    if let Some(xml) = vast_adm(ad) {
        let vast = evaluate_xml(&xml, Pack::None);
        technical = vast.dimensions.technical;
        operational = vast.dimensions.operational;
        extra_findings = vast.findings;
    }
    findings.extend(extra_findings);

    let issued = audit
        .lastmod
        .or(audit.init)
        .and_then(millis_to_odt)
        .unwrap_or_else(OffsetDateTime::now_utc);

    let mut scope = Scope {
        kind: "environment".into(),
        id: ad.id.clone().unwrap_or_else(|| "ama".into()),
    };
    if opts.experimental {
        if let Some(pub_id) = audit
            .publisher_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            scope.kind = "publisher".into();
            scope.id = pub_id.to_string();
        } else if let Some(deal) = audit
            .deal_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            scope.kind = "deal".into();
            scope.id = deal.to_string();
        }
    }

    let identity = CreativeIdentity {
        crid: ad.id.clone(),
        media_url: ad
            .extra
            .get("iurl")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        version: Some("1".into()),
        ..Default::default()
    };

    ReadinessEvidence {
        authority: Authority {
            id: "ama".into(),
            role: "ssp".into(),
            acts_on_behalf_of: Vec::new(),
        },
        scope,
        version: VERSION.into(),
        issued_at: issued,
        expires_at: issued + Duration::hours(24),
        identity,
        dimensions: Dimensions {
            policy,
            technical,
            operational,
            contextual: DimensionState::NotEvaluated,
        },
        lifecycle,
        findings,
        entry_path: EntryPath::Proactive,
        prepared_media: None,
        operational_claims: None,
        seller_treatment: None,
        unknown_creative_treatment: None,
        pattern: None,
        environments: Vec::new(),
        facts: None,
        sgai: None,
        transaction: None,
        auction_context: None,
    }
}

fn map_audit_status(status: Option<i64>) -> (Lifecycle, DimensionState) {
    match status {
        Some(1) | Some(2) => (Lifecycle::PendingReview, DimensionState::Unknown),
        Some(3) => (Lifecycle::Approved, DimensionState::Ready),
        Some(4) => (Lifecycle::Rejected, DimensionState::NotReady),
        Some(5) => (Lifecycle::RevalidationRequired, DimensionState::Unknown),
        Some(6) => (Lifecycle::Expired, DimensionState::NotReady),
        _ => (Lifecycle::Unknown, DimensionState::Unknown),
    }
}

fn vast_adm(ad: &AmaAd) -> Option<String> {
    for key in ["video", "display"] {
        let adm = ad.extra.get(key)?.get("adm")?.as_str()?;
        if adm.contains("<VAST") {
            return Some(adm.to_string());
        }
    }
    None
}

fn millis_to_odt(ms: i64) -> Option<OffsetDateTime> {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(ms) * 1_000_000).ok()
}

/// Serialize an ad back to JSON, preserving unknown fields.
pub fn round_trip_ad(ad: &AmaAd) -> Result<Value, String> {
    serde_json::to_value(ad).map_err(|e| e.to_string())
}

pub fn surface_as_map(s: &CapabilitySurface) -> BTreeMap<String, Value> {
    serde_json::from_value(serde_json::to_value(s).unwrap()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(name: &str) -> &'static str {
        match name {
            "submit-response-preapproved.json" => {
                include_str!("../../../fixtures/ama/submit-response-preapproved.json")
            }
            "webhook-denied.json" => include_str!("../../../fixtures/ama/webhook-denied.json"),
            "approved-with-corr.json" => {
                include_str!("../../../fixtures/ama/approved-with-corr.json")
            }
            "expired.json" => include_str!("../../../fixtures/ama/expired.json"),
            "submit-display.json" => include_str!("../../../fixtures/ama/submit-display.json"),
            "v12-publisher-deal.json" => {
                include_str!("../../../fixtures/ama/v12-publisher-deal.json")
            }
            "video-vast.json" => include_str!("../../../fixtures/ama/video-vast.json"),
            _ => panic!("unknown fixture {name}"),
        }
    }

    #[test]
    fn spec_submit_response_is_pending() {
        let ads = adapt_ama(
            load("submit-response-preapproved.json"),
            AdaptOptions::default(),
        )
        .unwrap();
        assert_eq!(ads.len(), 1);
        assert_eq!(ads[0].lifecycle, Lifecycle::PendingReview);
        assert_eq!(ads[0].dimensions.policy, DimensionState::Unknown);
        assert_eq!(ads[0].identity.crid.as_deref(), Some("557391"));
    }

    #[test]
    fn spec_denied_webhook_is_rejected() {
        let ads = adapt_ama(load("webhook-denied.json"), AdaptOptions::default()).unwrap();
        assert_eq!(ads[0].lifecycle, Lifecycle::Rejected);
        assert_eq!(ads[0].dimensions.policy, DimensionState::NotReady);
        assert!(ads[0]
            .findings
            .iter()
            .any(|f| f.message.contains("disallowed")));
    }

    #[test]
    fn spec_approved_with_corr_round_trips() {
        let raw = load("approved-with-corr.json");
        let ads = parse_ama(raw).unwrap();
        let back = round_trip_ad(&ads[0]).unwrap();
        assert_eq!(back["id"], "557391");
        assert_eq!(back["audit"]["status"], 3);
        assert!(back["audit"]["corr"]["cat"].is_string());
        let ev = adapt_ad(&ads[0], &AdaptOptions::default());
        assert_eq!(ev.lifecycle, Lifecycle::Approved);
        assert_eq!(ev.dimensions.policy, DimensionState::Ready);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "ama.audit.corrected"));
    }

    #[test]
    fn spec_expired_maps_expired() {
        let ads = adapt_ama(load("expired.json"), AdaptOptions::default()).unwrap();
        assert_eq!(ads[0].lifecycle, Lifecycle::Expired);
    }

    #[test]
    fn submit_without_audit_is_unknown() {
        let ads = adapt_ama(load("submit-display.json"), AdaptOptions::default()).unwrap();
        assert_eq!(ads[0].lifecycle, Lifecycle::Unknown);
        assert_eq!(
            ads[0].identity.media_url.as_deref(),
            Some("http://cdn.dsp.com/iurls/557391.gif")
        );
        let parsed = parse_ama(load("submit-display.json")).unwrap();
        let back = round_trip_ad(&parsed[0]).unwrap();
        assert_eq!(back["display"]["w"], 300);
    }

    #[test]
    fn v12_fields_ignored_without_experimental() {
        let ads = adapt_ama(load("v12-publisher-deal.json"), AdaptOptions::default()).unwrap();
        assert_eq!(ads[0].scope.kind, "environment");
        assert_eq!(ads[0].scope.id, "557391");
    }

    #[test]
    fn v12_fields_map_with_experimental() {
        let ads = adapt_ama(
            load("v12-publisher-deal.json"),
            AdaptOptions { experimental: true },
        )
        .unwrap();
        assert_eq!(ads[0].scope.kind, "publisher");
        assert_eq!(ads[0].scope.id, "pub-42");
    }

    #[test]
    fn video_adm_vast_fills_technical() {
        let ads = adapt_ama(load("video-vast.json"), AdaptOptions::default()).unwrap();
        assert_eq!(ads[0].dimensions.technical, DimensionState::Ready);
        assert_eq!(ads[0].lifecycle, Lifecycle::Approved);
    }

    #[test]
    fn ama_1_1_surface_does_not_require_lookup() {
        let s = ama_1_1_surface();
        assert!(!s.known_creative_lookup);
        assert!(!s.deal_publisher_environment);
        assert!(!s.overlay_select);
        let s2 = ama_1_2_surface();
        assert!(s2.known_creative_lookup);
    }
}
