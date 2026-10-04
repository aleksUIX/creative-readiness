//! Contextual eligibility. Ready is not eligible-for-this-break.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{fail_closed, media_facts, Finding, ReadinessEvidence};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Opportunity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub break_duration_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deal_id: Option<String>,
    /// programmatic | direct. Echoed; not a pacing decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    /// Seller-declared competitive conflict. We echo it; we do not score content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub competitive_conflict: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vast_xml: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_family: Option<String>,
    /// Echoed; not a pacing or supply-path decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supply_path: Option<String>,
}

pub(crate) fn nonempty_str(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|v| !v.is_empty())
}

pub(crate) fn nonempty_owned(s: Option<String>) -> Option<String> {
    nonempty_str(s.as_deref()).map(str::to_string)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadyRef {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    pub evidence_version: String,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Eligibility {
    pub ready_ref: ReadyRef,
    pub context: Opportunity,
    pub eligible: bool,
    pub reasons: Vec<Finding>,
}

pub fn check_eligibility(ev: &ReadinessEvidence, opp: &Opportunity) -> Eligibility {
    eligibility(ev, opp, OffsetDateTime::now_utc())
}

pub fn eligibility(ev: &ReadinessEvidence, opp: &Opportunity, now: OffsetDateTime) -> Eligibility {
    let mut reasons = Vec::new();
    if fail_closed(ev, now) {
        reasons.push(Finding {
            requirement_id: "eligibility.expired".into(),
            severity: "error".into(),
            message: "evidence is expired or pulled; fail closed for this opportunity.".into(),
            remediable: Some(true),
            dimension: Some("contextual".into()),
            ..Default::default()
        });
    }
    let duration = ev
        .facts
        .as_ref()
        .and_then(|f| f.duration_seconds)
        .or_else(|| {
            opp.vast_xml
                .as_deref()
                .and_then(|xml| media_facts(xml).duration_seconds)
        });
    if let (Some(limit), Some(dur)) = (opp.break_duration_seconds, duration) {
        if dur > limit {
            reasons.push(Finding {
                requirement_id: "eligibility.break-duration".into(),
                severity: "error".into(),
                message: format!("creative is {dur}s; break is {limit}s."),
                remediable: Some(true),
                dimension: Some("contextual".into()),
                ..Default::default()
            });
        }
    }
    if opp.competitive_conflict == Some(true) {
        reasons.push(Finding {
            requirement_id: "eligibility.competitive-conflict".into(),
            severity: "error".into(),
            message: "seller declared a competitive conflict for this opportunity.".into(),
            remediable: Some(true),
            dimension: Some("contextual".into()),
            ..Default::default()
        });
    }
    scope_mismatch(
        &mut reasons,
        "eligibility.deal-mismatch",
        "deal",
        opp.deal_id.as_deref(),
        ev,
    );
    scope_mismatch(
        &mut reasons,
        "eligibility.event-mismatch",
        "event",
        opp.event_id.as_deref(),
        ev,
    );
    scope_mismatch(
        &mut reasons,
        "eligibility.publisher-mismatch",
        "publisher",
        opp.publisher_id.as_deref(),
        ev,
    );
    Eligibility {
        ready_ref: ReadyRef {
            fingerprint: ev.identity.fingerprint.clone(),
            evidence_version: ev.version.clone(),
            expires_at: ev.expires_at,
        },
        context: Opportunity {
            vast_xml: None,
            ..opp.clone()
        },
        eligible: reasons.is_empty(),
        reasons,
    }
}

fn scope_mismatch(
    reasons: &mut Vec<Finding>,
    requirement_id: &str,
    kind: &str,
    want: Option<&str>,
    ev: &ReadinessEvidence,
) {
    let Some(want) = nonempty_str(want) else {
        return;
    };
    if ev.scope.kind != kind {
        return;
    }
    if want == ev.scope.id.trim() {
        return;
    }
    reasons.push(Finding {
        requirement_id: requirement_id.into(),
        severity: "error".into(),
        message: format!(
            "opportunity {kind} {want} does not match evidence {kind} {}.",
            ev.scope.id
        ),
        remediable: Some(true),
        dimension: Some("contextual".into()),
        ..Default::default()
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{evaluate, EvaluateRequest, OperationalClaims, Pack};
    use time::Duration;

    fn ready_ev() -> crate::ReadinessEvidence {
        let xml = include_str!("../../../fixtures/ok-mezzanine.xml");
        let claims: OperationalClaims =
            serde_json::from_str(include_str!("../../../fixtures/claims-ok.json")).unwrap();
        evaluate(EvaluateRequest {
            vast_xml: xml,
            pack: Pack::LiveSsai,
            claims: Some(claims),
            ..Default::default()
        })
    }

    #[test]
    fn ready_thirty_is_ineligible_for_fifteen_second_break() {
        let ev = ready_ev();
        assert_eq!(ev.dimensions.technical, crate::DimensionState::Ready);
        assert_eq!(ev.dimensions.operational, crate::DimensionState::Ready);
        let spec: Opportunity =
            serde_json::from_str(include_str!("../../../fixtures/eligibility/break-15s.json"))
                .unwrap();
        let el = eligibility(&ev, &spec, ev.issued_at);
        assert!(!el.eligible);
        assert!(el
            .reasons
            .iter()
            .any(|f| f.requirement_id == "eligibility.break-duration"));
        crate::schema_catalog::assert_schema(
            "eligibility.json",
            &serde_json::to_value(&el).unwrap(),
        );
    }

    #[test]
    fn ready_thirty_fits_thirty_second_break() {
        let ev = ready_ev();
        let spec: Opportunity =
            serde_json::from_str(include_str!("../../../fixtures/eligibility/break-30s.json"))
                .unwrap();
        let el = eligibility(&ev, &spec, ev.issued_at);
        assert!(el.eligible);
        assert!(el.reasons.is_empty());
    }

    #[test]
    fn expired_evidence_is_ineligible() {
        let mut ev = ready_ev();
        ev.expires_at = ev.issued_at - Duration::seconds(1);
        let spec = Opportunity {
            break_duration_seconds: Some(30),
            ..Default::default()
        };
        let el = eligibility(&ev, &spec, ev.issued_at);
        assert!(!el.eligible);
        assert!(el
            .reasons
            .iter()
            .any(|f| f.requirement_id == "eligibility.expired"));
    }

    #[test]
    fn deal_and_event_scope_mismatch_are_ineligible() {
        let xml = include_str!("../../../fixtures/ok-mezzanine.xml");
        let claims: OperationalClaims =
            serde_json::from_str(include_str!("../../../fixtures/claims-ok.json")).unwrap();
        let ev = evaluate(EvaluateRequest {
            vast_xml: xml,
            pack: Pack::LiveSsai,
            claims: Some(claims),
            deal_id: Some("deal-9".into()),
            ..Default::default()
        });
        assert_eq!(ev.scope.kind, "deal");
        let miss = eligibility(
            &ev,
            &Opportunity {
                deal_id: Some("deal-8".into()),
                break_duration_seconds: Some(30),
                ..Default::default()
            },
            ev.issued_at,
        );
        assert!(!miss.eligible);
        assert!(miss
            .reasons
            .iter()
            .any(|f| f.requirement_id == "eligibility.deal-mismatch"));
        let hit = eligibility(
            &ev,
            &Opportunity {
                deal_id: Some("deal-9".into()),
                break_duration_seconds: Some(30),
                ..Default::default()
            },
            ev.issued_at,
        );
        assert!(hit.eligible);
        let event_ev = evaluate(EvaluateRequest {
            vast_xml: xml,
            pack: Pack::None,
            event_id: Some("game-1".into()),
            ..Default::default()
        });
        let event_miss = eligibility(
            &event_ev,
            &Opportunity {
                event_id: Some("news-1".into()),
                ..Default::default()
            },
            event_ev.issued_at,
        );
        assert!(!event_miss.eligible);
        assert!(event_miss
            .reasons
            .iter()
            .any(|f| f.requirement_id == "eligibility.event-mismatch"));
    }

    #[test]
    fn leftover_empty_deal_id_is_not_a_scope_mismatch() {
        let ev = evaluate(EvaluateRequest {
            vast_xml: include_str!("../../../fixtures/ok-https-mp4.xml"),
            pack: Pack::None,
            deal_id: Some("deal-9".into()),
            ..Default::default()
        });
        assert_eq!(ev.scope.kind, "deal");
        let el = eligibility(
            &ev,
            &Opportunity {
                deal_id: Some("  ".into()),
                ..Default::default()
            },
            ev.issued_at,
        );
        assert!(el.eligible);
        let blank_ev = evaluate(EvaluateRequest {
            vast_xml: include_str!("../../../fixtures/ok-https-mp4.xml"),
            pack: Pack::None,
            deal_id: Some("  ".into()),
            ..Default::default()
        });
        assert_eq!(blank_ev.scope.kind, "environment");
    }
}
