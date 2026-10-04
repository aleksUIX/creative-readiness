//! Multi-path and multi-authority evaluate.

use serde::{Deserialize, Serialize};

use crate::{
    eligibility::nonempty_owned, evaluate, DimensionState, Dimensions, EntryPath, EvaluateRequest,
    Finding, Lifecycle, Pack, PolicyOverlay, ReadinessEvidence,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentSpec {
    pub id: String,
    pub pack: Pack,
    #[serde(default)]
    pub overlays: Vec<PolicyOverlay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentResult {
    pub id: String,
    pub architecture: String,
    pub dimensions: Dimensions,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sgai: Option<SgaiSplit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SgaiSplit {
    pub server: DimensionState,
    pub signaling: DimensionState,
    pub client_capability: DimensionState,
    pub execution: DimensionState,
}

pub fn sgai_split(
    xml: &str,
    server_ready: bool,
    claims: Option<&crate::OperationalClaims>,
) -> SgaiSplit {
    let server = if server_ready {
        DimensionState::Ready
    } else {
        DimensionState::NotReady
    };
    let signaling = if crate::media::universal_ad_id_text(xml).is_some()
        && crate::media::ad_serving_id_text(xml).is_some()
    {
        DimensionState::Ready
    } else {
        DimensionState::NotReady
    };
    let client_capability = match claims.and_then(|c| c.client_ready) {
        Some(true) => DimensionState::Ready,
        Some(false) => DimensionState::NotReady,
        None => DimensionState::NotEvaluated,
    };
    let execution = match (server, signaling, client_capability) {
        (DimensionState::Ready, DimensionState::Ready, DimensionState::Ready) => {
            DimensionState::Ready
        }
        (DimensionState::Ready, DimensionState::Ready, DimensionState::NotEvaluated) => {
            DimensionState::NotEvaluated
        }
        _ => DimensionState::NotReady,
    };
    SgaiSplit {
        server,
        signaling,
        client_capability,
        execution,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthoritySpec {
    pub id: String,
    pub role: String,
    #[serde(default)]
    pub acts_on_behalf_of: Vec<String>,
    pub pack: Pack,
    #[serde(default)]
    pub overlays: Vec<PolicyOverlay>,
}

#[derive(Debug, Clone, Default)]
pub struct PathContext {
    pub deal_id: Option<String>,
    pub publisher_id: Option<String>,
    pub event_id: Option<String>,
    pub device_family: Option<String>,
    pub entry_path: EntryPath,
    pub policy: Option<crate::PolicyDocument>,
}

/// One creative, many delivery environments. Top-level technical is not a global bit:
/// it is Ready only when every path that evaluates technical is Ready.
pub fn evaluate_paths(
    xml: &str,
    claims: Option<crate::OperationalClaims>,
    paths: &[EnvironmentSpec],
) -> ReadinessEvidence {
    evaluate_paths_ctx(xml, claims, paths, &PathContext::default())
}

pub fn evaluate_paths_ctx(
    xml: &str,
    claims: Option<crate::OperationalClaims>,
    paths: &[EnvironmentSpec],
    ctx: &PathContext,
) -> ReadinessEvidence {
    let mut environments = Vec::new();
    let mut findings = Vec::new();
    for spec in paths {
        let mut ev = evaluate(EvaluateRequest {
            vast_xml: xml,
            pack: spec.pack,
            claims: claims.clone(),
            overlays: spec.overlays.clone(),
            deal_id: ctx.deal_id.clone(),
            publisher_id: ctx.publisher_id.clone(),
            event_id: ctx.event_id.clone(),
            device_family: ctx.device_family.clone(),
            entry_path: ctx.entry_path,
            policy: ctx.policy.clone(),
            ..Default::default()
        });
        let mut note = None;
        if spec.pack == Pack::LiveSgai {
            ev.dimensions.operational = DimensionState::NotEvaluated;
            if ev.dimensions.technical == DimensionState::Ready
                && ev.dimensions.policy != DimensionState::NotReady
            {
                note = Some("server-ready only".into());
            }
        }
        for f in &mut ev.findings {
            f.environment = Some(spec.id.clone());
            if let Some(deal) = spec
                .overlays
                .iter()
                .find_map(|o| nonempty_owned(o.deal_id.clone()))
            {
                if f.deal.is_none() {
                    f.deal = Some(deal);
                }
            }
        }
        findings.extend(ev.findings.clone());
        environments.push(EnvironmentResult {
            id: spec.id.clone(),
            architecture: spec
                .pack
                .policy()
                .and_then(|p| p.architecture)
                .unwrap_or_else(|| spec.pack.as_str().into()),
            dimensions: ev.dimensions,
            note,
            sgai: ev.sgai,
        });
    }

    let technical = if environments
        .iter()
        .any(|e| e.dimensions.technical == DimensionState::NotReady)
    {
        DimensionState::NotReady
    } else if environments
        .iter()
        .any(|e| e.dimensions.technical == DimensionState::Ready)
    {
        DimensionState::Ready
    } else {
        DimensionState::Unknown
    };
    let operational = if environments
        .iter()
        .filter(|e| e.dimensions.operational != DimensionState::NotEvaluated)
        .any(|e| e.dimensions.operational == DimensionState::NotReady)
    {
        DimensionState::NotReady
    } else if environments
        .iter()
        .any(|e| e.dimensions.operational == DimensionState::Ready)
    {
        DimensionState::Ready
    } else {
        DimensionState::Unknown
    };
    let policy_state = if environments
        .iter()
        .any(|e| e.dimensions.policy == DimensionState::NotReady)
    {
        DimensionState::NotReady
    } else if environments
        .iter()
        .any(|e| e.dimensions.policy == DimensionState::Ready)
    {
        DimensionState::Ready
    } else {
        DimensionState::Unknown
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

    let mut out = evaluate(EvaluateRequest {
        vast_xml: xml,
        pack: paths.first().map(|p| p.pack).unwrap_or_default(),
        claims: claims.clone(),
        overlays: paths
            .first()
            .map(|p| p.overlays.clone())
            .unwrap_or_default(),
        deal_id: ctx.deal_id.clone(),
        publisher_id: ctx.publisher_id.clone(),
        event_id: ctx.event_id.clone(),
        device_family: ctx.device_family.clone(),
        entry_path: ctx.entry_path,
        policy: ctx.policy.clone(),
        ..Default::default()
    });
    out.dimensions.technical = technical;
    out.dimensions.operational = operational;
    out.dimensions.policy = policy_state;
    out.lifecycle = lifecycle;
    out.findings = findings;
    out.environments = environments;
    if let Some(deal) = paths
        .iter()
        .flat_map(|p| p.overlays.iter())
        .find_map(|o| nonempty_owned(o.deal_id.clone()))
    {
        out.scope.kind = "deal".into();
        out.scope.id = deal;
    }
    out
}

/// Marketplace vs publisher: keep both sides. Disagreement is two findings, not a silent win.
pub fn evaluate_authorities(
    xml: &str,
    authorities: &[AuthoritySpec],
    claims: Option<crate::OperationalClaims>,
) -> ReadinessEvidence {
    let mut findings = Vec::new();
    let mut ready = Vec::new();
    let mut not_ready = Vec::new();
    let mut acts = Vec::new();
    for spec in authorities {
        let ev = evaluate(EvaluateRequest {
            vast_xml: xml,
            pack: spec.pack,
            claims: claims.clone(),
            overlays: spec.overlays.clone(),
            authority_id: &spec.id,
            authority_role: Some(spec.role.clone()),
            acts_on_behalf_of: spec.acts_on_behalf_of.clone(),
            ..Default::default()
        });
        let ok = ev.dimensions.technical == DimensionState::Ready
            && ev.dimensions.policy != DimensionState::NotReady;
        if ok {
            ready.push(spec.id.clone());
            findings.push(Finding {
                requirement_id: format!("authority.{}.ready", spec.id),
                severity: "info".into(),
                message: format!("{} ({}) asserts technically ready.", spec.id, spec.role),
                remediable: Some(false),
                publisher: (spec.role == "publisher").then(|| spec.id.clone()),
                dimension: Some("policy".into()),
                ..Default::default()
            });
        } else {
            not_ready.push(spec.id.clone());
            findings.push(Finding {
                requirement_id: format!("authority.{}.not-ready", spec.id),
                severity: "error".into(),
                message: format!("{} ({}) asserts not technically ready.", spec.id, spec.role),
                remediable: Some(true),
                publisher: (spec.role == "publisher").then(|| spec.id.clone()),
                dimension: Some("policy".into()),
                ..Default::default()
            });
        }
        findings.extend(ev.findings);
        acts.extend(spec.acts_on_behalf_of.iter().cloned());
    }
    if !ready.is_empty() && !not_ready.is_empty() {
        findings.push(Finding {
            requirement_id: "policy.authority-conflict".into(),
            severity: "error".into(),
            message: format!(
                "authorities disagree (ready: {}; not ready: {}); both findings kept.",
                ready.join(","),
                not_ready.join(",")
            ),
            remediable: Some(true),
            dimension: Some("policy".into()),
            ..Default::default()
        });
    }

    let mut out = evaluate(EvaluateRequest {
        vast_xml: xml,
        pack: Pack::None,
        claims: claims.clone(),
        ..Default::default()
    });
    out.authority.role = authorities
        .first()
        .map(|a| a.role.clone())
        .unwrap_or_else(|| "other".into());
    out.authority.id = authorities
        .first()
        .map(|a| a.id.clone())
        .unwrap_or_else(|| "local".into());
    out.authority.acts_on_behalf_of = acts;
    out.findings = findings;
    out.dimensions.technical = if not_ready.is_empty() {
        DimensionState::Ready
    } else {
        DimensionState::NotReady
    };
    out.dimensions.policy = if !ready.is_empty() && !not_ready.is_empty() {
        DimensionState::NotReady
    } else {
        DimensionState::Unknown
    };
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{evaluate, EntryPath, OperationalClaims};

    fn mezz() -> &'static str {
        include_str!("../../../fixtures/ok-mezzanine.xml")
    }

    fn https() -> &'static str {
        include_str!("../../../fixtures/ok-https-mp4.xml")
    }

    fn deal() -> PolicyOverlay {
        crate::parse_overlay(include_str!("../../../fixtures/overlays/deal-premium.json")).unwrap()
    }

    fn publisher_strict() -> PolicyOverlay {
        crate::parse_overlay(include_str!(
            "../../../fixtures/overlays/publisher-strict.json"
        ))
        .unwrap()
    }

    fn claims_ok() -> OperationalClaims {
        serde_json::from_str(include_str!("../../../fixtures/claims-ok.json")).unwrap()
    }

    #[test]
    fn one_creative_three_environments_and_deal_overlay() {
        let deal = deal();
        let paths = [
            EnvironmentSpec {
                id: "ssai-a".into(),
                pack: Pack::LiveSsai,
                overlays: vec![deal.clone()],
            },
            EnvironmentSpec {
                id: "ssai-b".into(),
                pack: Pack::LiveSsai,
                overlays: vec![deal.clone(), publisher_strict()],
            },
            EnvironmentSpec {
                id: "sgai".into(),
                pack: Pack::LiveSgai,
                overlays: vec![deal.clone()],
            },
        ];
        let ev = evaluate_paths(mezz(), Some(claims_ok()), &paths);
        assert_eq!(ev.environments.len(), 3);
        assert_eq!(ev.scope.kind, "deal");
        assert_eq!(ev.scope.id, "deal-9");
        let a = ev.environments.iter().find(|e| e.id == "ssai-a").unwrap();
        assert_eq!(a.dimensions.technical, DimensionState::Ready);
        assert_eq!(a.dimensions.operational, DimensionState::Ready);
        let b = ev.environments.iter().find(|e| e.id == "ssai-b").unwrap();
        assert_eq!(b.dimensions.policy, DimensionState::NotReady);
        assert!(ev.findings.iter().any(|f| {
            f.environment.as_deref() == Some("ssai-b")
                && f.requirement_id == "publisher-strict.viewable-required"
        }));
        let g = ev.environments.iter().find(|e| e.id == "sgai").unwrap();
        assert_eq!(g.dimensions.technical, DimensionState::Ready);
        assert_eq!(g.dimensions.operational, DimensionState::NotEvaluated);
        assert_eq!(g.note.as_deref(), Some("server-ready only"));
        assert_eq!(
            g.sgai.as_ref().unwrap().execution,
            DimensionState::NotEvaluated
        );
        crate::schema_catalog::assert_schema(
            "readiness-evidence.json",
            &serde_json::to_value(&ev).unwrap(),
        );
    }

    #[test]
    fn leftover_empty_deal_id_does_not_set_deal_scope() {
        let leftover = PolicyOverlay {
            id: "leftover".into(),
            deal_id: Some("".into()),
            ..Default::default()
        };
        let paths = [EnvironmentSpec {
            id: "ssai-a".into(),
            pack: Pack::None,
            overlays: vec![leftover],
        }];
        let ev = evaluate_paths(https(), None, &paths);
        assert_ne!(ev.scope.kind, "deal");
        assert!(ev.findings.iter().all(|f| f.deal.as_deref() != Some("")));
    }

    #[test]
    fn aggregator_publisher_conflict_keeps_both_findings() {
        let authorities = [
            AuthoritySpec {
                id: "marketplace".into(),
                role: "marketplace".into(),
                acts_on_behalf_of: vec!["pub-1".into()],
                pack: Pack::LiveSgai,
                overlays: vec![],
            },
            AuthoritySpec {
                id: "pub-1".into(),
                role: "publisher".into(),
                acts_on_behalf_of: vec![],
                pack: Pack::LiveSsai,
                overlays: vec![],
            },
        ];
        let ev = evaluate_authorities(https(), &authorities, None);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "authority.marketplace.ready"));
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "authority.pub-1.not-ready"));
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "live-ssai.mezzanine-required"));
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "policy.authority-conflict"));
        assert_eq!(ev.authority.acts_on_behalf_of, vec!["pub-1".to_string()]);
        let kept = ev
            .findings
            .iter()
            .filter(|f| {
                f.requirement_id == "authority.marketplace.ready"
                    || f.requirement_id == "authority.pub-1.not-ready"
            })
            .count();
        assert_eq!(kept, 2);
    }

    #[test]
    fn deal_forbids_auction_triggered() {
        let ev = evaluate(EvaluateRequest {
            vast_xml: mezz(),
            pack: Pack::LiveSgai,
            overlays: vec![deal()],
            entry_path: EntryPath::AuctionTriggered,
            ..Default::default()
        });
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "deal-premium.auction-triggered-forbidden"));
        assert_eq!(ev.entry_path, EntryPath::AuctionTriggered);
    }

    #[test]
    fn resolve_overlays_deal_then_publisher() {
        let stacked = crate::resolve_overlays(Some(&deal()), &[publisher_strict()]);
        assert_eq!(stacked[0].id, "deal-premium");
        assert_eq!(stacked[1].id, "publisher-strict");
    }
}
