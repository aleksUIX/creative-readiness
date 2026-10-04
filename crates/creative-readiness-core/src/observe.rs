//! Auction-triggered observe. Records seller treatment; does not choose it.

use serde::{Deserialize, Serialize};

use crate::{
    eligibility::nonempty_owned, evaluate, media_fingerprint, select_overlays, CreativeIdentity,
    DimensionState, EntryPath, EvaluateRequest, Finding, Lifecycle, Opportunity, Pack,
    PolicyOverlay, ReadinessEvidence,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SellerTreatment {
    Serve,
    Fallback,
    Reject,
    Defer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    #[serde(default)]
    pub vast_xml: Option<String>,
    #[serde(default)]
    pub identity: Option<CreativeIdentity>,
    #[serde(default)]
    pub pack: Pack,
    pub treatment: SellerTreatment,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub won: Option<bool>,
    #[serde(default)]
    pub authority_id: Option<String>,
    #[serde(default)]
    pub auction_context: Option<AuctionContext>,
    /// OpenRTB 2.x bid-response fragment (seatbid/bid/imp). Optional; not a pacing engine.
    #[serde(default)]
    pub openrtb_json: Option<String>,
    #[serde(default)]
    pub overlays: Vec<PolicyOverlay>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AuctionContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bidid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dealid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxduration: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_crid: Option<String>,
}

pub fn parse_openrtb_lite(json: &str) -> Result<AuctionContext, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let bid = v
        .pointer("/seatbid/0/bid/0")
        .cloned()
        .or_else(|| v.get("bid").cloned())
        .unwrap_or(serde_json::Value::Null);
    let publisher = v
        .pointer("/site/publisher/id")
        .or_else(|| v.pointer("/app/publisher/id"))
        .and_then(|x| x.as_str());
    Ok(AuctionContext {
        bidid: v
            .get("id")
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .or_else(|| bid.get("id").and_then(|x| x.as_str()).map(str::to_string)),
        crid: nonempty_owned(bid.get("crid").and_then(|x| x.as_str()).map(str::to_string)),
        dealid: nonempty_owned(
            bid.get("dealid")
                .and_then(|x| x.as_str())
                .map(str::to_string),
        ),
        publisher_id: nonempty_owned(publisher.map(str::to_string)),
        maxduration: v
            .pointer("/imp/0/video/maxduration")
            .and_then(|x| x.as_u64())
            .or_else(|| {
                v.pointer("/imp/0/video/maxduration")
                    .and_then(|x| x.as_i64())
                    .map(|n| n as u64)
            }),
        fallback_crid: nonempty_owned(
            bid.pointer("/ext/fallback_crid")
                .and_then(|x| x.as_str())
                .map(str::to_string),
        ),
    })
}

pub fn observe(obs: Observation) -> ReadinessEvidence {
    let mut ctx = obs.auction_context.clone();
    if ctx.is_none() {
        if let Some(raw) = obs.openrtb_json.as_deref() {
            ctx = parse_openrtb_lite(raw).ok();
        }
    }
    let xml = obs.vast_xml.as_deref().unwrap_or("");
    let deal_id = nonempty_owned(ctx.as_ref().and_then(|c| c.dealid.clone()));
    let publisher_id = nonempty_owned(ctx.as_ref().and_then(|c| c.publisher_id.clone()));
    let overlays = select_overlays(
        &obs.overlays,
        &Opportunity {
            deal_id: deal_id.clone(),
            publisher_id: publisher_id.clone(),
            ..Default::default()
        },
    );
    let mut ev = if xml.trim().is_empty() {
        let mut ev = evaluate(EvaluateRequest {
            vast_xml: "<VAST version=\"4.2\"></VAST>",
            pack: Pack::None,
            authority_id: obs.authority_id.as_deref().unwrap_or("local"),
            entry_path: EntryPath::AuctionTriggered,
            deal_id: deal_id.clone(),
            publisher_id: publisher_id.clone(),
            overlays: overlays.clone(),
            ..Default::default()
        });
        ev.findings.clear();
        ev.dimensions.technical = DimensionState::NotEvaluated;
        ev.dimensions.operational = DimensionState::NotEvaluated;
        ev.dimensions.policy = DimensionState::Unknown;
        ev
    } else {
        evaluate(EvaluateRequest {
            vast_xml: xml,
            pack: obs.pack,
            authority_id: obs.authority_id.as_deref().unwrap_or("local"),
            entry_path: EntryPath::AuctionTriggered,
            deal_id: deal_id.clone(),
            publisher_id: publisher_id.clone(),
            overlays,
            ..Default::default()
        })
    };

    if let Some(id) = obs.identity {
        if let Some(v) = nonempty_owned(id.crid) {
            ev.identity.crid = Some(v);
        }
        if let Some(v) = nonempty_owned(id.universal_ad_id) {
            ev.identity.universal_ad_id = Some(v);
        }
        if let Some(v) = nonempty_owned(id.media_url) {
            ev.identity.media_url = Some(v);
        }
        if let Some(v) = nonempty_owned(id.content_hash) {
            ev.identity.content_hash = Some(v);
        }
        if let Some(v) = nonempty_owned(id.mid) {
            ev.identity.mid = Some(v);
        }
        if let Some(v) = nonempty_owned(id.mezzanine_url) {
            ev.identity.mezzanine_url = Some(v);
        }
    }
    if let Some(ref ctx) = ctx {
        if let Some(v) = ctx.crid.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            ev.identity.crid = Some(v.to_string());
        }
        if let Some(max) = ctx.maxduration {
            if let Some(dur) = ev.facts.as_ref().and_then(|f| f.duration_seconds) {
                if dur > max {
                    ev.findings.push(Finding {
                        requirement_id: "observe.imp-maxduration".into(),
                        severity: "warning".into(),
                        message: format!(
                            "creative is {dur}s; OpenRTB imp.video.maxduration is {max}s."
                        ),
                        remediable: Some(true),
                        dimension: Some("contextual".into()),
                        ..Default::default()
                    });
                }
            }
        }
    }
    ev.identity.normalize();
    let fp = media_fingerprint(&ev.identity);
    ev.identity.fingerprint = if fp.is_empty() { None } else { Some(fp) };

    ev.entry_path = EntryPath::AuctionTriggered;
    ev.seller_treatment = Some(obs.treatment);
    ev.pattern = obs.pattern.clone();
    ev.auction_context = ctx;
    ev.lifecycle = if xml.trim().is_empty() {
        Lifecycle::Observed
    } else {
        Lifecycle::PendingReview
    };

    if obs.pack.policy().is_some_and(|p| p.proactive_required) {
        ev.findings.push(Finding {
            requirement_id: "observe.proactive-required".into(),
            severity: "info".into(),
            message:
                "inventory prefers proactive prep; observe still records the auction sighting."
                    .into(),
            remediable: Some(false),
            ..Default::default()
        });
    }
    ev
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_obs(name: &str) -> Observation {
        let raw = match name {
            "a" => include_str!("../../../fixtures/observe/pattern-a.json"),
            "b" => include_str!("../../../fixtures/observe/pattern-b.json"),
            "c" => include_str!("../../../fixtures/observe/pattern-c.json"),
            "d" => include_str!("../../../fixtures/observe/pattern-d.json"),
            "e" => include_str!("../../../fixtures/observe/pattern-e.json"),
            _ => panic!("unknown {name}"),
        };
        let mut obs: Observation = serde_json::from_str(raw).unwrap();
        obs.vast_xml = Some(include_str!("../../../fixtures/ok-https-mp4.xml").to_string());
        obs
    }

    #[test]
    fn patterns_echo_fixture_treatment_and_stop_at_pending_review() {
        let cases = [
            ("a", SellerTreatment::Defer),
            ("b", SellerTreatment::Defer),
            ("c", SellerTreatment::Serve),
            ("d", SellerTreatment::Fallback),
            ("e", SellerTreatment::Reject),
        ];
        for (name, treatment) in cases {
            let ev = observe(load_obs(name));
            assert_eq!(ev.entry_path, EntryPath::AuctionTriggered, "{name}");
            assert_eq!(ev.seller_treatment, Some(treatment), "{name}");
            assert_eq!(ev.lifecycle, Lifecycle::PendingReview, "{name}");
            assert_ne!(ev.lifecycle, Lifecycle::Approved, "{name}");
            assert_ne!(ev.lifecycle, Lifecycle::OperationallyReady, "{name}");
        }
    }

    #[test]
    fn observe_without_xml_is_observed() {
        let obs: Observation =
            serde_json::from_str(include_str!("../../../fixtures/observe/pattern-a.json")).unwrap();
        let ev = observe(obs);
        assert_eq!(ev.lifecycle, Lifecycle::Observed);
        assert_eq!(ev.seller_treatment, Some(SellerTreatment::Defer));
        assert_eq!(ev.dimensions.technical, DimensionState::NotEvaluated);
    }

    #[test]
    fn observe_records_even_when_pack_wants_proactive() {
        let mut obs = load_obs("e");
        obs.pack = Pack::LiveSsai;
        let ev = observe(obs);
        assert_eq!(ev.lifecycle, Lifecycle::PendingReview);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "observe.proactive-required"));
    }

    #[test]
    fn openrtb_lite_fills_crid_and_warns_on_maxduration() {
        let mut obs = load_obs("a");
        obs.openrtb_json =
            Some(include_str!("../../../fixtures/observe/openrtb-lite.json").to_string());
        obs.identity = None;
        let ev = observe(obs);
        assert_eq!(ev.identity.crid.as_deref(), Some("crid-9"));
        assert_eq!(
            ev.auction_context.as_ref().and_then(|c| c.maxduration),
            Some(15)
        );
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "observe.imp-maxduration"));
    }

    #[test]
    fn observe_passes_auction_entry_and_deal_overlay() {
        let mut obs = load_obs("a");
        obs.openrtb_json =
            Some(include_str!("../../../fixtures/observe/openrtb-lite.json").to_string());
        obs.overlays = vec![crate::parse_overlay(include_str!(
            "../../../fixtures/overlays/deal-premium.json"
        ))
        .unwrap()];
        let ev = observe(obs);
        assert_eq!(ev.entry_path, EntryPath::AuctionTriggered);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "deal-premium.auction-triggered-forbidden"));
    }

    #[test]
    fn observe_recomputes_fingerprint_after_identity_merge() {
        let mut obs = load_obs("a");
        obs.identity = Some(CreativeIdentity {
            media_url: Some("https://cdn.example.com/other.mp4".into()),
            universal_ad_id: Some("UAD-OTHER".into()),
            ..Default::default()
        });
        let ev = observe(obs);
        let fp = ev.identity.fingerprint.as_deref().unwrap();
        assert!(fp.starts_with("url:"));
        assert!(fp.contains("|uad:UAD-OTHER"));
    }

    #[test]
    fn observe_blank_mid_is_dropped() {
        let mut obs = load_obs("a");
        obs.identity = Some(CreativeIdentity {
            mid: Some("   ".into()),
            crid: Some("".into()),
            ..Default::default()
        });
        let ev = observe(obs);
        assert_eq!(ev.identity.mid.as_deref(), Some("srv-001"));
        assert_eq!(ev.identity.crid, None);
    }

    #[test]
    fn observe_merges_nonempty_mid() {
        let mut obs = load_obs("a");
        obs.identity = Some(CreativeIdentity {
            mid: Some(" mapped-9 ".into()),
            ..Default::default()
        });
        let ev = observe(obs);
        assert_eq!(ev.identity.mid.as_deref(), Some("mapped-9"));
    }
}
