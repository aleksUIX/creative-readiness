//! Ad-ops view: dimensions, gates, remediable finding ids.

use serde::{Deserialize, Serialize};

use crate::{prebid_gate, BidGate, DimensionState, Opportunity, ReadinessEvidence};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadinessSummary {
    pub lifecycle: crate::Lifecycle,
    pub may_bid: bool,
    pub may_serve: bool,
    pub policy: DimensionState,
    pub technical: DimensionState,
    pub operational: DimensionState,
    pub error_ids: Vec<String>,
    pub remediable_ids: Vec<String>,
    pub environments_ready: Vec<String>,
    pub environments_not_ready: Vec<String>,
}

pub fn summarize(ev: &ReadinessEvidence, opp: Option<&Opportunity>) -> ReadinessSummary {
    let gate: BidGate = prebid_gate(ev, opp);
    let mut error_ids = Vec::new();
    let mut remediable_ids = Vec::new();
    for f in &ev.findings {
        if f.severity == "error" {
            error_ids.push(f.requirement_id.clone());
            if f.remediable == Some(true) {
                remediable_ids.push(f.requirement_id.clone());
            }
        }
    }
    let mut environments_ready = Vec::new();
    let mut environments_not_ready = Vec::new();
    for env in &ev.environments {
        let ok = env.dimensions.technical == DimensionState::Ready
            && env.dimensions.policy != DimensionState::NotReady;
        if ok {
            environments_ready.push(env.id.clone());
        } else {
            environments_not_ready.push(env.id.clone());
        }
    }
    ReadinessSummary {
        lifecycle: ev.lifecycle,
        may_bid: gate.may_bid,
        may_serve: gate.may_serve,
        policy: ev.dimensions.policy,
        technical: ev.dimensions.technical,
        operational: ev.dimensions.operational,
        error_ids,
        remediable_ids,
        environments_ready,
        environments_not_ready,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{evaluate_paths, EnvironmentSpec, OperationalClaims, Pack};

    #[test]
    fn three_path_summary_splits_ready_and_not_ready() {
        let deal =
            crate::parse_overlay(include_str!("../../../fixtures/overlays/deal-premium.json"))
                .unwrap();
        let pub_strict = crate::parse_overlay(include_str!(
            "../../../fixtures/overlays/publisher-strict.json"
        ))
        .unwrap();
        let claims: OperationalClaims =
            serde_json::from_str(include_str!("../../../fixtures/claims-ok.json")).unwrap();
        let ev = evaluate_paths(
            include_str!("../../../fixtures/ok-mezzanine.xml"),
            Some(claims),
            &[
                EnvironmentSpec {
                    id: "ssai-a".into(),
                    pack: Pack::LiveSsai,
                    overlays: vec![deal.clone()],
                },
                EnvironmentSpec {
                    id: "ssai-b".into(),
                    pack: Pack::LiveSsai,
                    overlays: vec![deal, pub_strict],
                },
            ],
        );
        let s = summarize(&ev, None);
        assert!(s.environments_ready.contains(&"ssai-a".into()));
        assert!(s.environments_not_ready.contains(&"ssai-b".into()));
        assert!(s.error_ids.iter().any(|id| id.contains("viewable")));
        crate::schema_catalog::assert_schema(
            "readiness-summary.json",
            &serde_json::to_value(&s).unwrap(),
        );
    }
}
