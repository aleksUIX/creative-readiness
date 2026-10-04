//! Option C capability surface for this module (AMA is one adapter onto it).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilitySurface {
    pub submit_or_reference: bool,
    pub known_creative_lookup: bool,
    pub validation_context_id: bool,
    pub requirement_discovery: bool,
    pub identity_version: bool,
    pub multi_authority_status: bool,
    pub lifecycle_states: Vec<String>,
    pub structured_failure: bool,
    pub readiness_retrieval: bool,
    pub deal_publisher_environment: bool,
    pub update_revalidation: bool,
    pub eligibility: bool,
    pub last_minute_diff: bool,
    pub prebid_gate: bool,
    pub openrtb_lite_observe: bool,
    pub overlay_select: bool,
    pub fallback_lookup: bool,
    pub review_queue: bool,
}

pub fn module_surface() -> CapabilitySurface {
    CapabilitySurface {
        submit_or_reference: true,
        known_creative_lookup: true,
        validation_context_id: true,
        requirement_discovery: true,
        identity_version: true,
        multi_authority_status: true,
        lifecycle_states: vec![
            "unknown".into(),
            "observed".into(),
            "pending_review".into(),
            "approved".into(),
            "rejected".into(),
            "technically_ready".into(),
            "operationally_ready".into(),
            "expired".into(),
            "revalidation_required".into(),
        ],
        structured_failure: true,
        readiness_retrieval: true,
        deal_publisher_environment: true,
        update_revalidation: true,
        eligibility: true,
        last_minute_diff: true,
        prebid_gate: true,
        openrtb_lite_observe: true,
        overlay_select: true,
        fallback_lookup: true,
        review_queue: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_matches_schema() {
        let s = module_surface();
        crate::schema_catalog::assert_schema(
            "capability-surface.json",
            &serde_json::to_value(&s).unwrap(),
        );
        assert!(s.eligibility);
        assert!(s.prebid_gate);
        assert!(s.overlay_select);
        assert!(s.review_queue);
    }
}
