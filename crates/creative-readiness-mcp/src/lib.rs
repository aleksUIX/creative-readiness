//! MCP tool implementations. The stdio server lives in `server`.

use creative_readiness_core::{
    check_eligibility, diff_vast_with_policy, discover_policy, evaluate, evaluate_authorities,
    evaluate_with_store, media_fingerprint, module_surface, observe, parse_policy, prebid_gate,
    select_overlays, summarize, transform, AuctionContext, AuthoritySpec, CreativeIdentity,
    EntryPath, EvaluateRequest, IdentityStore, LookupHit, Observation, Opportunity, Pack,
    PolicyDocument, PolicyOverlay, PreparedMedia, ReadinessEvidence, TransformationKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub mod server;

pub const TOOLS: &[&str] = &[
    "discover_policy",
    "evaluate_creative",
    "prepare_creative",
    "known_creative_lookup",
    "readiness_report",
    "diff_creative",
    "check_eligibility",
    "capability_surface",
    "transform_creative",
    "prebid_gate",
    "summarize_readiness",
    "observe_creative",
    "select_overlays",
    "review_queue",
    "lookup_fallback",
    "evaluate_authorities",
];

#[derive(Debug, Deserialize, Default)]
pub struct ToolCall {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

pub fn call_tool(name: &str, arguments: Value) -> Result<Value, String> {
    match name {
        "discover_policy" => discover(arguments),
        "evaluate_creative" => evaluate_creative(arguments),
        "prepare_creative" => prepare_creative(arguments),
        "known_creative_lookup" => known_lookup(arguments),
        "readiness_report" => readiness_report(arguments),
        "diff_creative" => diff_creative(arguments),
        "check_eligibility" => check_eligibility_tool(arguments),
        "capability_surface" => Ok(serde_json::to_value(module_surface()).unwrap()),
        "transform_creative" => transform_creative(arguments),
        "prebid_gate" => prebid_gate_tool(arguments),
        "summarize_readiness" => summarize_tool(arguments),
        "observe_creative" => observe_tool(arguments),
        "select_overlays" => select_overlays_tool(arguments),
        "review_queue" => review_queue_tool(arguments),
        "lookup_fallback" => lookup_fallback_tool(arguments),
        "evaluate_authorities" => evaluate_authorities_tool(arguments),
        other => Err(format!("unknown tool {other}")),
    }
}

fn pack_from(v: &Value) -> Pack {
    v.get("pack")
        .and_then(|p| p.as_str())
        .and_then(|s| Pack::parse(s).ok())
        .unwrap_or(Pack::LiveSsai)
}

fn discover(args: Value) -> Result<Value, String> {
    let id = args
        .get("policy_id")
        .and_then(|v| v.as_str())
        .unwrap_or("live-ssai");
    let p = discover_policy(id)?;
    serde_json::to_value(p).map_err(|e| e.to_string())
}

fn opt_str(args: &Value, names: &[&str]) -> Option<String> {
    names.iter().find_map(|k| {
        args.get(*k)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    })
}

fn entry_path_from(args: &Value) -> EntryPath {
    match opt_str(args, &["entry_path"]).as_deref() {
        Some("auction_triggered") | Some("auction-triggered") => EntryPath::AuctionTriggered,
        _ if args.get("auction_triggered").and_then(|v| v.as_bool()) == Some(true) => {
            EntryPath::AuctionTriggered
        }
        _ => EntryPath::Proactive,
    }
}

fn policy_from(args: &Value) -> Result<Option<PolicyDocument>, String> {
    if let Some(raw) = args.get("policy_json").and_then(|v| v.as_str()) {
        return parse_policy(raw).map(Some);
    }
    if let Some(v) = args.get("policy") {
        if !v.is_null() {
            return serde_json::from_value(v.clone())
                .map(Some)
                .map_err(|e| e.to_string());
        }
    }
    Ok(None)
}

fn overlays_from(args: &Value) -> Result<Vec<PolicyOverlay>, String> {
    match args.get("overlays") {
        Some(v) if !v.is_null() => serde_json::from_value(v.clone()).map_err(|e| e.to_string()),
        _ => Ok(Vec::new()),
    }
}

fn lookup_hit_json(hit: LookupHit, extra: Value) -> Value {
    let mut out = match hit {
        LookupHit::Unknown => json!({"hit": "unknown"}),
        LookupHit::Known { evidence } => json!({"hit": "known", "evidence": evidence}),
        LookupHit::MediaChanged {
            previous_fingerprint,
        } => json!({
            "hit": "media_changed",
            "previous_fingerprint": previous_fingerprint
        }),
    };
    if let Value::Object(extra) = extra {
        if let Value::Object(map) = &mut out {
            map.extend(extra);
        }
    }
    out
}

fn evaluate_creative(args: Value) -> Result<Value, String> {
    let xml = args
        .get("xml")
        .and_then(|v| v.as_str())
        .ok_or("xml required")?;
    let pack = pack_from(&args);
    let deal_id = opt_str(&args, &["deal_id", "deal"]);
    let publisher_id = opt_str(&args, &["publisher_id", "publisher"]);
    let event_id = opt_str(&args, &["event_id", "event"]);
    let device_family = opt_str(&args, &["device_family", "device"]);
    let opp = Opportunity {
        deal_id: deal_id.clone(),
        publisher_id: publisher_id.clone(),
        event_id: event_id.clone(),
        device_family: device_family.clone(),
        ..Default::default()
    };
    let overlays = select_overlays(&overlays_from(&args)?, &opp);
    let ev = evaluate(EvaluateRequest {
        vast_xml: xml,
        pack,
        transaction: opt_str(&args, &["transaction"]),
        deal_id,
        publisher_id,
        event_id,
        device_family,
        entry_path: entry_path_from(&args),
        policy: policy_from(&args)?,
        overlays,
        ..Default::default()
    });
    serde_json::to_value(ev).map_err(|e| e.to_string())
}

fn prepare_creative(args: Value) -> Result<Value, String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("path required (local mezzanine; reference probe, not a farm)")?;
    let pack = pack_from(&args);
    let policy_id = pack.as_str();
    if policy_id == "none" {
        return Err("prepare requires a pack with a transcode_ladder".into());
    }
    let policy = discover_policy(policy_id)?;
    let ladder = policy
        .transcode_ladder
        .ok_or_else(|| format!("pack {policy_id} has no transcode_ladder"))?;
    let out = args
        .get("out_dir")
        .and_then(|v| v.as_str())
        .unwrap_or("target/prepared");
    let media = creative_readiness_probe::prepare(
        std::path::Path::new(path),
        &ladder,
        std::path::Path::new(out),
    )?;
    serde_json::to_value(media).map_err(|e| e.to_string())
}

fn known_lookup(args: Value) -> Result<Value, String> {
    let store = match args.get("store_json").and_then(|v| v.as_str()) {
        Some(raw) => IdentityStore::from_json(raw)?,
        None => IdentityStore::new(),
    };
    if let Some(crid) = args.get("fallback_crid").and_then(|v| v.as_str()) {
        return Ok(lookup_hit_json(
            store.lookup_fallback(&AuctionContext {
                fallback_crid: Some(crid.into()),
                ..Default::default()
            }),
            json!({"fallback_crid": crid}),
        ));
    }
    let identity: CreativeIdentity = if let Some(crid) = args.get("crid").and_then(|v| v.as_str()) {
        return Ok(lookup_hit_json(
            store.lookup_crid(crid),
            json!({"crid": crid}),
        ));
    } else if let Some(id) = args.get("identity") {
        serde_json::from_value(id.clone()).map_err(|e| e.to_string())?
    } else if let Some(xml) = args.get("xml").and_then(|v| v.as_str()) {
        evaluate(EvaluateRequest {
            vast_xml: xml,
            pack: Pack::None,
            ..Default::default()
        })
        .identity
    } else {
        return Err("identity, xml, crid, or fallback_crid required".into());
    };
    let fp = media_fingerprint(&identity);
    Ok(lookup_hit_json(
        store.lookup(&identity),
        json!({"fingerprint": fp}),
    ))
}

fn readiness_report(args: Value) -> Result<Value, String> {
    let xml = args
        .get("xml")
        .and_then(|v| v.as_str())
        .ok_or("xml required")?;
    let pack = pack_from(&args);
    let mut store = match args.get("store_json").and_then(|v| v.as_str()) {
        Some(raw) => IdentityStore::from_json(raw)?,
        None => IdentityStore::new(),
    };
    let ev = evaluate_with_store(
        EvaluateRequest {
            vast_xml: xml,
            pack,
            ..Default::default()
        },
        &store,
    );
    store.insert(ev.identity.clone(), ev.clone());
    let fix = vastlint_core::fix(xml);
    Ok(json!({
        "evidence": ev,
        "structural_fix": {
            "applied": fix.applied.len(),
            "xml": if fix.applied.is_empty() { Value::Null } else { Value::String(fix.xml) }
        },
        "store_json": store.to_json()?,
    }))
}

fn diff_creative(args: Value) -> Result<Value, String> {
    let old = args
        .get("old_xml")
        .and_then(|v| v.as_str())
        .ok_or("old_xml required")?;
    let new = args
        .get("new_xml")
        .and_then(|v| v.as_str())
        .ok_or("new_xml required")?;
    let pack = args
        .get("pack")
        .and_then(|v| v.as_str())
        .unwrap_or("live-ssai");
    let policy = if pack == "none" {
        None
    } else {
        discover_policy(pack).ok()
    };
    serde_json::to_value(diff_vast_with_policy(old, new, policy.as_ref()))
        .map_err(|e| e.to_string())
}

fn check_eligibility_tool(args: Value) -> Result<Value, String> {
    let ev: ReadinessEvidence = if let Some(ev) = args.get("evidence") {
        serde_json::from_value(ev.clone()).map_err(|e| e.to_string())?
    } else if let Some(xml) = args.get("xml").and_then(|v| v.as_str()) {
        evaluate(EvaluateRequest {
            vast_xml: xml,
            pack: pack_from(&args),
            ..Default::default()
        })
    } else {
        return Err("evidence or xml required".into());
    };
    let opp: Opportunity = serde_json::from_value(
        args.get("opportunity")
            .cloned()
            .ok_or("opportunity required")?,
    )
    .map_err(|e| e.to_string())?;
    serde_json::to_value(check_eligibility(&ev, &opp)).map_err(|e| e.to_string())
}

fn transform_creative(args: Value) -> Result<Value, String> {
    let ev: ReadinessEvidence =
        serde_json::from_value(args.get("evidence").cloned().ok_or("evidence required")?)
            .map_err(|e| e.to_string())?;
    let kind = match args
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("encode")
    {
        "encode" => TransformationKind::Encode,
        "media_replacement" | "media-replacement" => TransformationKind::MediaReplacement,
        other => {
            return Err(format!(
                "kind {other:?}; expected encode or media_replacement"
            ))
        }
    };
    let prepared = match args.get("prepared") {
        Some(v) if !v.is_null() => {
            Some(serde_json::from_value::<PreparedMedia>(v.clone()).map_err(|e| e.to_string())?)
        }
        _ => None,
    };
    let pack = pack_from(&args);
    let policy = if pack.as_str() == "none" {
        None
    } else {
        discover_policy(pack.as_str()).ok()
    };
    let rights = policy.as_ref().map(|p| p.transformation_rights.clone());
    let ladder = policy.as_ref().and_then(|p| p.transcode_ladder.clone());
    serde_json::to_value(transform(
        &ev,
        kind,
        prepared.as_ref(),
        rights.as_ref(),
        ladder.as_ref(),
    ))
    .map_err(|e| e.to_string())
}

fn evidence_from(args: &Value) -> Result<ReadinessEvidence, String> {
    if let Some(ev) = args.get("evidence") {
        serde_json::from_value(ev.clone()).map_err(|e| e.to_string())
    } else if let Some(xml) = args.get("xml").and_then(|v| v.as_str()) {
        Ok(evaluate(EvaluateRequest {
            vast_xml: xml,
            pack: pack_from(args),
            ..Default::default()
        }))
    } else {
        Err("evidence or xml required".into())
    }
}

fn opportunity_from(args: &Value) -> Option<Opportunity> {
    args.get("opportunity")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
}

fn prebid_gate_tool(args: Value) -> Result<Value, String> {
    let ev = evidence_from(&args)?;
    serde_json::to_value(prebid_gate(&ev, opportunity_from(&args).as_ref()))
        .map_err(|e| e.to_string())
}

fn summarize_tool(args: Value) -> Result<Value, String> {
    let ev = evidence_from(&args)?;
    serde_json::to_value(summarize(&ev, opportunity_from(&args).as_ref()))
        .map_err(|e| e.to_string())
}

fn observe_tool(args: Value) -> Result<Value, String> {
    let obs: Observation = if args.get("treatment").is_some() {
        serde_json::from_value(args).map_err(|e| e.to_string())?
    } else if let Some(o) = args.get("observation") {
        serde_json::from_value(o.clone()).map_err(|e| e.to_string())?
    } else {
        return Err("observation or treatment required".into());
    };
    serde_json::to_value(observe(obs)).map_err(|e| e.to_string())
}

fn select_overlays_tool(args: Value) -> Result<Value, String> {
    let catalog: Vec<PolicyOverlay> = args
        .get("catalog")
        .cloned()
        .ok_or_else(|| "catalog required".to_string())
        .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))?;
    let opp = opportunity_from(&args).ok_or_else(|| "opportunity required".to_string())?;
    serde_json::to_value(select_overlays(&catalog, &opp)).map_err(|e| e.to_string())
}

fn review_queue_tool(args: Value) -> Result<Value, String> {
    let store = match args.get("store_json").and_then(|v| v.as_str()) {
        Some(raw) => IdentityStore::from_json(raw)?,
        None => IdentityStore::new(),
    };
    serde_json::to_value(store.review_queue()).map_err(|e| e.to_string())
}

fn lookup_fallback_tool(args: Value) -> Result<Value, String> {
    let store = match args.get("store_json").and_then(|v| v.as_str()) {
        Some(raw) => IdentityStore::from_json(raw)?,
        None => IdentityStore::new(),
    };
    let ctx: AuctionContext = if let Some(ctx) = args.get("auction_context") {
        serde_json::from_value(ctx.clone()).map_err(|e| e.to_string())?
    } else if let Some(crid) = opt_str(&args, &["fallback_crid"]) {
        AuctionContext {
            fallback_crid: Some(crid),
            ..Default::default()
        }
    } else {
        return Err("auction_context or fallback_crid required".into());
    };
    Ok(lookup_hit_json(
        store.lookup_fallback(&ctx),
        json!({"fallback_crid": ctx.fallback_crid}),
    ))
}

fn evaluate_authorities_tool(args: Value) -> Result<Value, String> {
    let xml = args
        .get("xml")
        .and_then(|v| v.as_str())
        .ok_or("xml required")?;
    let authorities: Vec<AuthoritySpec> = args
        .get("authorities")
        .cloned()
        .ok_or_else(|| "authorities required".to_string())
        .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))?;
    let claims = match args.get("claims") {
        Some(v) if !v.is_null() => {
            Some(serde_json::from_value(v.clone()).map_err(|e| e.to_string())?)
        }
        _ => None,
    };
    serde_json::to_value(evaluate_authorities(xml, &authorities, claims)).map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct ToolMeta {
    pub name: &'static str,
    pub description: &'static str,
}

pub fn tool_catalog() -> Vec<ToolMeta> {
    vec![
        ToolMeta {
            name: "discover_policy",
            description: "Load a builtin PolicyDocument (live-ssai, live-sgai, live-csai, live-hybrid).",
        },
        ToolMeta {
            name: "evaluate_creative",
            description: "Evaluate VAST XML against a pack and return ReadinessEvidence.",
        },
        ToolMeta {
            name: "prepare_creative",
            description: "Reference ffmpeg probe against the pack ladder. Not a transcode farm.",
        },
        ToolMeta {
            name: "known_creative_lookup",
            description: "Local identity lookup. Same media hash reuses evidence; new media misses.",
        },
        ToolMeta {
            name: "readiness_report",
            description: "Evaluate, optionally reuse from a store, and attach vastlint structural fix XML when applied.",
        },
        ToolMeta {
            name: "diff_creative",
            description: "Diff two VAST documents into an invalidation set (tracking-only vs media vs wrapper).",
        },
        ToolMeta {
            name: "check_eligibility",
            description: "Contextual eligibility for a break or event. Ready is not eligible-for-this-break.",
        },
        ToolMeta {
            name: "capability_surface",
            description: "Option C capability surface for this module. AMA is one adapter onto it.",
        },
        ToolMeta {
            name: "transform_creative",
            description: "Ladder encode inherits technical readiness; media replacement mints a new identity.",
        },
        ToolMeta {
            name: "prebid_gate",
            description: "Unknown may bid; serve needs ready plus eligible. Does not choose seller treatment.",
        },
        ToolMeta {
            name: "summarize_readiness",
            description: "Ad-ops summary: may_bid, may_serve, remediable finding ids, per-path ready/not-ready.",
        },
        ToolMeta {
            name: "observe_creative",
            description: "Auction-triggered observe. Seller treatment is an input; the module does not choose it.",
        },
        ToolMeta {
            name: "select_overlays",
            description: "Pick deal, publisher, event, and device overlays from a catalog. Deal first.",
        },
        ToolMeta {
            name: "review_queue",
            description: "Observed and pending-review rows from a local identity store. Not a hosted graph.",
        },
        ToolMeta {
            name: "lookup_fallback",
            description: "Lookup seller-supplied fallback_crid. Returns prior evidence if known. Does not choose fallback.",
        },
        ToolMeta {
            name: "evaluate_authorities",
            description: "Multi-authority evaluate. Disagreement keeps both findings.",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lists_roadmap_tools() {
        let names: Vec<_> = tool_catalog().iter().map(|t| t.name).collect();
        for required in TOOLS {
            assert!(names.contains(required), "missing {required}");
        }
    }

    #[test]
    fn evaluate_and_diff_tools() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let ev = call_tool("evaluate_creative", json!({"xml": xml, "pack": "none"})).unwrap();
        assert_eq!(ev["dimensions"]["technical"], "ready");
        let tracking = include_str!("../../../fixtures/ok-https-mp4-tracking.xml");
        let d = call_tool(
            "diff_creative",
            json!({"old_xml": xml, "new_xml": tracking}),
        )
        .unwrap();
        assert_eq!(d["tracking_only"], true);
        let p = call_tool("discover_policy", json!({"policy_id": "live-ssai"})).unwrap();
        assert_eq!(p["id"], "live-ssai");
        let surface = call_tool("capability_surface", json!({})).unwrap();
        assert_eq!(surface["eligibility"], true);
        let el = call_tool(
            "check_eligibility",
            json!({
                "xml": xml,
                "pack": "none",
                "opportunity": {"break_duration_seconds": 15}
            }),
        )
        .unwrap();
        assert_eq!(el["eligible"], false);
        let g = call_tool(
            "prebid_gate",
            json!({"xml": xml, "pack": "none", "opportunity": {"break_duration_seconds": 15}}),
        )
        .unwrap();
        assert_eq!(g["may_bid"], true);
        assert_eq!(g["may_serve"], false);
        let obs = call_tool(
            "observe_creative",
            json!({"treatment": "defer", "pack": "none", "pattern": "A"}),
        )
        .unwrap();
        assert_eq!(obs["lifecycle"], "observed");
        let catalog: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/overlays/catalog.json")).unwrap();
        let selected = call_tool(
            "select_overlays",
            json!({"catalog": catalog, "opportunity": {"event_id": "game-1"}}),
        )
        .unwrap();
        assert!(selected
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["id"] == "event-sports"));
        let expired: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/expired-pack.json")).unwrap();
        let closed = call_tool(
            "evaluate_creative",
            json!({"xml": xml, "pack": "none", "policy": expired, "event_id": "game-1"}),
        )
        .unwrap();
        assert_eq!(closed["scope"]["kind"], "event");
        assert!(closed["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["requirement_id"] == "policy.pack-expired"));
        let miss = call_tool(
            "evaluate_creative",
            json!({"xml": xml, "pack": "none", "overlays": catalog}),
        )
        .unwrap();
        assert!(!miss["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["requirement_id"] == "event-sports.max-duration"));
        let hit = call_tool(
            "evaluate_creative",
            json!({"xml": xml, "pack": "none", "overlays": catalog, "event_id": "game-1"}),
        )
        .unwrap();
        assert!(hit["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["requirement_id"] == "event-sports.max-duration"));
        let authorities: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/authorities/marketplace-publisher.json"
        ))
        .unwrap();
        let multi = call_tool(
            "evaluate_authorities",
            json!({"xml": xml, "authorities": authorities}),
        )
        .unwrap();
        assert!(multi["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["requirement_id"] == "policy.authority-conflict"));
    }

    #[test]
    fn lookup_unknown_without_store() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let hit = call_tool("known_creative_lookup", json!({"xml": xml})).unwrap();
        assert_eq!(hit["hit"], "unknown");
    }
}
