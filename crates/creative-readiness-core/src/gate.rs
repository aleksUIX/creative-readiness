//! DSP/SSP gate: unknown may bid; serve needs ready + eligible. We do not choose treatment.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{
    eligibility, fail_closed, DimensionState, Finding, Lifecycle, Opportunity, ReadinessEvidence,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BidGate {
    pub may_bid: bool,
    pub may_serve: bool,
    pub reasons: Vec<Finding>,
}

pub fn prebid_gate(ev: &ReadinessEvidence, opp: Option<&Opportunity>) -> BidGate {
    gate(ev, opp, OffsetDateTime::now_utc())
}

pub fn gate(ev: &ReadinessEvidence, opp: Option<&Opportunity>, now: OffsetDateTime) -> BidGate {
    let mut reasons = Vec::new();
    if fail_closed(ev, now) {
        reasons.push(find(
            "gate.expired",
            "evidence is expired or pulled; fail closed.",
        ));
    }
    if ev.lifecycle == Lifecycle::Rejected {
        reasons.push(find(
            "gate.rejected",
            "lifecycle is rejected; do not bid or serve on this evidence.",
        ));
    }
    let may_bid = reasons.is_empty();

    let mut serve_reasons = reasons.clone();
    if !serve_ready_lifecycle(ev) {
        serve_reasons.push(find(
            "gate.not-ready-to-serve",
            "lifecycle is not technically_ready, operationally_ready, or approved; unknown or pending may bid but not serve.",
        ));
    }
    if ev.dimensions.policy == DimensionState::NotReady {
        serve_reasons.push(find(
            "gate.policy-not-ready",
            "policy dimension is not ready for this environment.",
        ));
    }
    if ev.dimensions.technical != DimensionState::Ready {
        serve_reasons.push(find(
            "gate.technical-not-ready",
            "technical dimension is not ready.",
        ));
    }
    if !operational_ok(ev) {
        serve_reasons.push(find(
            "gate.operational-not-ready",
            "operational dimension is not ready to serve (SSAI prep missing, or SGAI execution not ready).",
        ));
    }
    if let Some(opp) = opp {
        let el = eligibility(ev, opp, now);
        if !el.eligible {
            serve_reasons.extend(el.reasons);
        }
    }
    let may_serve = may_bid
        && serve_ready_lifecycle(ev)
        && ev.dimensions.policy != DimensionState::NotReady
        && ev.dimensions.technical == DimensionState::Ready
        && operational_ok(ev)
        && opp
            .map(|o| eligibility(ev, o, now).eligible)
            .unwrap_or(true);

    BidGate {
        may_bid,
        may_serve,
        reasons: if may_serve { Vec::new() } else { serve_reasons },
    }
}

fn serve_ready_lifecycle(ev: &ReadinessEvidence) -> bool {
    matches!(
        ev.lifecycle,
        Lifecycle::TechnicallyReady | Lifecycle::OperationallyReady | Lifecycle::Approved
    )
}

fn operational_ok(ev: &ReadinessEvidence) -> bool {
    match ev.dimensions.operational {
        DimensionState::Ready => true,
        DimensionState::NotReady => false,
        DimensionState::Unknown | DimensionState::NotEvaluated => match &ev.sgai {
            Some(s) => s.execution == DimensionState::Ready,
            None => true,
        },
    }
}

fn find(id: &str, message: &str) -> Finding {
    Finding {
        requirement_id: id.into(),
        severity: "error".into(),
        message: message.into(),
        remediable: Some(true),
        dimension: Some("contextual".into()),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{evaluate, EvaluateRequest, OperationalClaims, Pack};

    fn ssai_ready() -> ReadinessEvidence {
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
    fn unknown_pending_may_bid_but_not_serve() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let ev = crate::observe(crate::Observation {
            vast_xml: Some(xml.into()),
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
        let g = gate(&ev, None, ev.issued_at);
        assert!(g.may_bid);
        assert!(!g.may_serve);
    }

    #[test]
    fn operationally_ready_may_serve_thirty_second_break() {
        let ev = ssai_ready();
        let opp = Opportunity {
            break_duration_seconds: Some(30),
            ..Default::default()
        };
        let g = gate(&ev, Some(&opp), ev.issued_at);
        assert!(g.may_bid);
        assert!(g.may_serve);
    }

    #[test]
    fn ready_may_bid_but_not_serve_short_break() {
        let ev = ssai_ready();
        let opp = Opportunity {
            break_duration_seconds: Some(15),
            ..Default::default()
        };
        let g = gate(&ev, Some(&opp), ev.issued_at);
        assert!(g.may_bid);
        assert!(!g.may_serve);
        crate::schema_catalog::assert_schema("bid-gate.json", &serde_json::to_value(&g).unwrap());
    }

    #[test]
    fn leftover_empty_deal_id_does_not_block_serve() {
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
        let g = gate(
            &ev,
            Some(&Opportunity {
                deal_id: Some("  ".into()),
                break_duration_seconds: Some(30),
                ..Default::default()
            }),
            ev.issued_at,
        );
        assert!(g.may_bid);
        assert!(g.may_serve);
    }
}
