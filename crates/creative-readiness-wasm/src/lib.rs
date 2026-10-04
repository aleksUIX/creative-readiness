//! Evaluate and lookup without the ffmpeg probe. Playground site can wait.

use creative_readiness_core::{
    check_eligibility, evaluate, evaluate_authorities, observe, prebid_gate, select_overlays,
    summarize, AuctionContext, AuthoritySpec, CreativeIdentity, EvaluateRequest, IdentityStore,
    LookupHit, Observation, Opportunity, Pack, PolicyOverlay, ReadinessEvidence,
};

pub fn evaluate_json(xml: &str, pack: &str) -> Result<String, String> {
    let pack = Pack::parse(pack).unwrap_or(Pack::None);
    let ev = evaluate(EvaluateRequest {
        vast_xml: xml,
        pack,
        ..Default::default()
    });
    serde_json::to_string(&ev).map_err(|e| e.to_string())
}

pub fn lookup_json(store_json: &str, identity_json: &str) -> Result<String, String> {
    let store = IdentityStore::from_json(store_json)?;
    let identity: CreativeIdentity =
        serde_json::from_str(identity_json).map_err(|e| e.to_string())?;
    let hit = match store.lookup(&identity) {
        LookupHit::Unknown => serde_json::json!({"hit": "unknown"}),
        LookupHit::Known { evidence } => {
            serde_json::json!({"hit": "known", "evidence": evidence})
        }
        LookupHit::MediaChanged {
            previous_fingerprint,
        } => serde_json::json!({
            "hit": "media_changed",
            "previous_fingerprint": previous_fingerprint
        }),
    };
    serde_json::to_string(&hit).map_err(|e| e.to_string())
}

pub fn eligibility_json(evidence_json: &str, opportunity_json: &str) -> Result<String, String> {
    let ev: ReadinessEvidence = serde_json::from_str(evidence_json).map_err(|e| e.to_string())?;
    let opp: Opportunity = serde_json::from_str(opportunity_json).map_err(|e| e.to_string())?;
    serde_json::to_string(&check_eligibility(&ev, &opp)).map_err(|e| e.to_string())
}

pub fn surface_json() -> String {
    serde_json::to_string(&creative_readiness_core::module_surface())
        .unwrap_or_else(|_| "{}".into())
}

pub fn gate_json(evidence_json: &str, opportunity_json: &str) -> Result<String, String> {
    let ev: ReadinessEvidence = serde_json::from_str(evidence_json).map_err(|e| e.to_string())?;
    let opp: Opportunity = serde_json::from_str(opportunity_json).map_err(|e| e.to_string())?;
    serde_json::to_string(&prebid_gate(&ev, Some(&opp))).map_err(|e| e.to_string())
}

pub fn summary_json(evidence_json: &str, opportunity_json: &str) -> Result<String, String> {
    let ev: ReadinessEvidence = serde_json::from_str(evidence_json).map_err(|e| e.to_string())?;
    let opp: Opportunity = serde_json::from_str(opportunity_json).map_err(|e| e.to_string())?;
    serde_json::to_string(&summarize(&ev, Some(&opp))).map_err(|e| e.to_string())
}

pub fn observe_json(observation_json: &str) -> Result<String, String> {
    let obs: Observation = serde_json::from_str(observation_json).map_err(|e| e.to_string())?;
    serde_json::to_string(&observe(obs)).map_err(|e| e.to_string())
}

pub fn select_overlays_json(catalog_json: &str, opportunity_json: &str) -> Result<String, String> {
    let catalog: Vec<PolicyOverlay> =
        serde_json::from_str(catalog_json).map_err(|e| e.to_string())?;
    let opp: Opportunity = serde_json::from_str(opportunity_json).map_err(|e| e.to_string())?;
    serde_json::to_string(&select_overlays(&catalog, &opp)).map_err(|e| e.to_string())
}

pub fn queue_json(store_json: &str) -> Result<String, String> {
    let store = IdentityStore::from_json(store_json)?;
    serde_json::to_string(&store.review_queue()).map_err(|e| e.to_string())
}

pub fn lookup_fallback_json(store_json: &str, fallback_crid: &str) -> Result<String, String> {
    let store = IdentityStore::from_json(store_json)?;
    let hit = store.lookup_fallback(&AuctionContext {
        fallback_crid: Some(fallback_crid.into()),
        ..Default::default()
    });
    let out = match hit {
        LookupHit::Unknown => serde_json::json!({"hit": "unknown", "fallback_crid": fallback_crid}),
        LookupHit::Known { evidence } => {
            serde_json::json!({"hit": "known", "fallback_crid": fallback_crid, "evidence": evidence})
        }
        LookupHit::MediaChanged {
            previous_fingerprint,
        } => serde_json::json!({
            "hit": "media_changed",
            "fallback_crid": fallback_crid,
            "previous_fingerprint": previous_fingerprint
        }),
    };
    serde_json::to_string(&out).map_err(|e| e.to_string())
}

pub fn evaluate_authorities_json(xml: &str, authorities_json: &str) -> Result<String, String> {
    let authorities: Vec<AuthoritySpec> =
        serde_json::from_str(authorities_json).map_err(|e| e.to_string())?;
    let ev = evaluate_authorities(xml, &authorities, None);
    serde_json::to_string(&ev).map_err(|e| e.to_string())
}

#[cfg(target_arch = "wasm32")]
mod wasm_api {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn evaluate(xml: &str, pack: &str) -> Result<String, JsValue> {
        super::evaluate_json(xml, pack).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn lookup(store_json: &str, identity_json: &str) -> Result<String, JsValue> {
        super::lookup_json(store_json, identity_json).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn eligibility(evidence_json: &str, opportunity_json: &str) -> Result<String, JsValue> {
        super::eligibility_json(evidence_json, opportunity_json).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn surface() -> String {
        super::surface_json()
    }

    #[wasm_bindgen]
    pub fn gate(evidence_json: &str, opportunity_json: &str) -> Result<String, JsValue> {
        super::gate_json(evidence_json, opportunity_json).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn summary(evidence_json: &str, opportunity_json: &str) -> Result<String, JsValue> {
        super::summary_json(evidence_json, opportunity_json).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn observe(observation_json: &str) -> Result<String, JsValue> {
        super::observe_json(observation_json).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn select_overlays(catalog_json: &str, opportunity_json: &str) -> Result<String, JsValue> {
        super::select_overlays_json(catalog_json, opportunity_json)
            .map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn queue(store_json: &str) -> Result<String, JsValue> {
        super::queue_json(store_json).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn lookup_fallback(store_json: &str, fallback_crid: &str) -> Result<String, JsValue> {
        super::lookup_fallback_json(store_json, fallback_crid).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn evaluate_authorities(xml: &str, authorities_json: &str) -> Result<String, JsValue> {
        super::evaluate_authorities_json(xml, authorities_json).map_err(|e| JsValue::from_str(&e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluate_json_is_technically_ready() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let json = evaluate_json(xml, "none").unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["dimensions"]["technical"], "ready");
    }

    #[test]
    fn lookup_json_unknown_on_empty_store() {
        let id = serde_json::json!({"crid": "x", "media_url": "https://cdn.example.com/ad.mp4"});
        let hit = lookup_json("[]", &id.to_string()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&hit).unwrap();
        assert_eq!(v["hit"], "unknown");
    }

    #[test]
    fn eligibility_json_flags_short_break() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let ev = evaluate_json(xml, "none").unwrap();
        let hit = eligibility_json(&ev, r#"{"break_duration_seconds":15}"#).unwrap();
        let v: serde_json::Value = serde_json::from_str(&hit).unwrap();
        assert_eq!(v["eligible"], false);
        let s: serde_json::Value = serde_json::from_str(&surface_json()).unwrap();
        assert_eq!(s["eligibility"], true);
        let g: serde_json::Value =
            serde_json::from_str(&gate_json(&ev, r#"{"break_duration_seconds":15}"#).unwrap())
                .unwrap();
        assert_eq!(g["may_bid"], true);
        assert_eq!(g["may_serve"], false);
    }

    #[test]
    fn observe_select_and_queue_json() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let obs_in = serde_json::json!({
            "treatment": "defer",
            "pack": "none",
            "pattern": "A",
            "vast_xml": xml
        });
        let obs = observe_json(&obs_in.to_string()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&obs).unwrap();
        assert_eq!(v["lifecycle"], "pending_review");
        let catalog = include_str!("../../../fixtures/overlays/catalog.json");
        let selected = select_overlays_json(catalog, r#"{"event_id":"game-1"}"#).unwrap();
        let arr: serde_json::Value = serde_json::from_str(&selected).unwrap();
        assert!(arr
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["id"] == "event-sports"));
        let ev: ReadinessEvidence = serde_json::from_str(&obs).unwrap();
        let mut store = IdentityStore::new();
        store.insert(ev.identity.clone(), ev);
        let q: serde_json::Value =
            serde_json::from_str(&queue_json(&store.to_json().unwrap()).unwrap()).unwrap();
        assert_eq!(q.as_array().unwrap().len(), 1);
        let miss: serde_json::Value =
            serde_json::from_str(&lookup_fallback_json(&store.to_json().unwrap(), "nope").unwrap())
                .unwrap();
        assert_eq!(miss["hit"], "unknown");
    }

    #[test]
    fn evaluate_authorities_json_keeps_conflict() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let spec = include_str!("../../../fixtures/authorities/marketplace-publisher.json");
        let json = evaluate_authorities_json(xml, spec).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["requirement_id"] == "policy.authority-conflict"));
    }
}
