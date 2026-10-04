//! Last-minute diff defaults (W20) until the WG votes invalidation_rules.

use serde::{Deserialize, Serialize};

use crate::{evaluate_xml, media_fingerprint, Pack};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Invalidation {
    pub requirement_id: String,
    pub dimension: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DiffResult {
    pub tracking_only: bool,
    pub keep_transcode: bool,
    pub invalidations: Vec<Invalidation>,
}

pub fn diff_vast(old_xml: &str, new_xml: &str) -> DiffResult {
    diff_vast_with_policy(old_xml, new_xml, None)
}

pub fn diff_vast_with_policy(
    old_xml: &str,
    new_xml: &str,
    policy: Option<&crate::PolicyDocument>,
) -> DiffResult {
    let rules = crate::invalidation_rules(policy);
    if old_xml == new_xml {
        return DiffResult {
            tracking_only: false,
            keep_transcode: true,
            invalidations: Vec::new(),
        };
    }
    let old = evaluate_xml(old_xml, Pack::None);
    let new = evaluate_xml(new_xml, Pack::None);
    let old_fp = media_fingerprint(&old.identity);
    let new_fp = media_fingerprint(&new.identity);
    let media_changed = !old_fp.is_empty() && !new_fp.is_empty() && old_fp != new_fp;
    let old_wrap = is_wrapper(old_xml);
    let new_wrap = is_wrapper(new_xml);
    let wrapper_changed = old_wrap != new_wrap || wrapper_uri(old_xml) != wrapper_uri(new_xml);
    let body_changed = creative_body_changed(old.facts.as_ref(), new.facts.as_ref());

    if media_changed && has_action(&rules, "media_change", "invalidate_technical_operational") {
        return DiffResult {
            tracking_only: false,
            keep_transcode: false,
            invalidations: vec![
                Invalidation {
                    requirement_id: "diff.media-changed".into(),
                    dimension: "technical".into(),
                    message: "media URL or hash changed; re-evaluate technical.".into(),
                },
                Invalidation {
                    requirement_id: "diff.media-changed".into(),
                    dimension: "operational".into(),
                    message: "media URL or hash changed; transcode and cache claims do not carry."
                        .into(),
                },
            ],
        };
    }

    if wrapper_changed && has_action(&rules, "wrapper_change", "revaluate_technical") {
        return DiffResult {
            tracking_only: false,
            keep_transcode: true,
            invalidations: vec![Invalidation {
                requirement_id: "diff.wrapper-changed".into(),
                dimension: "technical".into(),
                message: "wrapper changed; re-evaluate technical. Transcode may be kept.".into(),
            }],
        };
    }

    if body_changed {
        return DiffResult {
            tracking_only: false,
            keep_transcode: true,
            invalidations: vec![Invalidation {
                requirement_id: "diff.creative-body-changed".into(),
                dimension: "technical".into(),
                message: "declared creative body changed (duration, codec, bitrate, category, or size); re-evaluate technical. Same media URL may keep transcode.".into(),
            }],
        };
    }

    let keep = has_action(&rules, "tracking_only", "keep_transcode");
    DiffResult {
        tracking_only: keep,
        keep_transcode: keep,
        invalidations: Vec::new(),
    }
}

fn creative_body_changed(old: Option<&crate::MediaFacts>, new: Option<&crate::MediaFacts>) -> bool {
    match (old, new) {
        (Some(a), Some(b)) => {
            a.duration_seconds != b.duration_seconds
                || a.mime != b.mime
                || a.codec != b.codec
                || a.audio_codec != b.audio_codec
                || a.container != b.container
                || a.width != b.width
                || a.height != b.height
                || a.bitrate != b.bitrate
                || a.categories != b.categories
                || a.mezzanine_url != b.mezzanine_url
        }
        _ => false,
    }
}

fn has_action(rules: &[crate::InvalidationRule], when: &str, action: &str) -> bool {
    rules.iter().any(|r| r.when == when && r.action == action)
}

fn is_wrapper(xml: &str) -> bool {
    crate::media::open_after_elem(xml, "Wrapper").is_some()
}

fn wrapper_uri(xml: &str) -> Option<String> {
    crate::media::elem_text(xml, "VASTAdTagURI")
}

pub fn apply_diff(ev: &mut crate::ReadinessEvidence, diff: &DiffResult) {
    for inv in &diff.invalidations {
        match inv.dimension.as_str() {
            "technical" => ev.dimensions.technical = crate::DimensionState::NotReady,
            "operational" => ev.dimensions.operational = crate::DimensionState::NotReady,
            "policy" => ev.dimensions.policy = crate::DimensionState::NotReady,
            _ => {}
        }
        ev.findings.push(crate::Finding {
            requirement_id: inv.requirement_id.clone(),
            severity: "error".into(),
            message: inv.message.clone(),
            remediable: Some(true),
            dimension: Some(inv.dimension.clone()),
            ..Default::default()
        });
    }
    if !diff.keep_transcode {
        ev.prepared_media = None;
        ev.operational_claims = None;
        ev.dimensions.operational = crate::DimensionState::Unknown;
    }
    if !diff.invalidations.is_empty() {
        ev.lifecycle = crate::Lifecycle::RevalidationRequired;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracking_only_keeps_transcode() {
        let old = include_str!("../../../fixtures/ok-https-mp4.xml");
        let new = include_str!("../../../fixtures/ok-https-mp4-tracking.xml");
        let d = diff_vast(old, new);
        assert!(d.tracking_only);
        assert!(d.keep_transcode);
        assert!(d.invalidations.is_empty());
    }

    #[test]
    fn duration_change_is_not_tracking_only() {
        let old = include_str!("../../../fixtures/ok-https-mp4.xml");
        let new = old.replace("00:00:30", "00:00:15");
        let d = diff_vast(old, &new);
        assert!(!d.tracking_only);
        assert!(d.keep_transcode);
        assert!(d
            .invalidations
            .iter()
            .any(|i| i.requirement_id == "diff.creative-body-changed"));
    }

    #[test]
    fn bitrate_change_is_not_tracking_only() {
        let old = include_str!("../../../fixtures/ok-https-mp4.xml");
        let new = old.replace("bitrate=\"1500\"", "bitrate=\"500\"");
        let d = diff_vast(old, &new);
        assert!(!d.tracking_only);
        assert!(d.keep_transcode);
        assert!(d
            .invalidations
            .iter()
            .any(|i| i.requirement_id == "diff.creative-body-changed"));
    }

    #[test]
    fn media_url_change_invalidates_technical_and_operational() {
        let old = include_str!("../../../fixtures/ok-https-mp4.xml");
        let new = include_str!("../../../fixtures/ok-mezzanine-new-media.xml");
        let d = diff_vast(old, new);
        assert!(!d.keep_transcode);
        assert_eq!(d.invalidations.len(), 2);
        assert!(d.invalidations.iter().any(|i| i.dimension == "technical"));
        assert!(d.invalidations.iter().any(|i| i.dimension == "operational"));
    }

    #[test]
    fn new_wrapper_revaluates_technical() {
        let old = include_str!("../../../fixtures/ok-https-mp4.xml");
        let new = include_str!("../../../fixtures/wrapper.xml");
        let d = diff_vast(old, new);
        assert!(d.keep_transcode);
        assert!(d
            .invalidations
            .iter()
            .any(|i| i.requirement_id == "diff.wrapper-changed"));
    }

    #[test]
    fn mixed_case_wrapper_uri_is_a_wrapper_change() {
        let old = include_str!("../../../fixtures/ok-https-mp4.xml");
        let new = r#"<vast version="4.2"><ad><wrapper>
<vastadtaguri><![cdata[https://ad.example.com/other.xml]]></vastadtaguri>
</wrapper></ad></vast>"#;
        let d = diff_vast(old, new);
        assert!(d.keep_transcode);
        assert!(d
            .invalidations
            .iter()
            .any(|i| i.requirement_id == "diff.wrapper-changed"));
    }

    #[test]
    fn four_last_minute_fixtures() {
        let cases: [(&str, serde_json::Value); 4] = [
            (
                "tracking-only.json",
                serde_json::from_str(include_str!(
                    "../../../fixtures/last-minute/tracking-only.json"
                ))
                .unwrap(),
            ),
            (
                "media-change.json",
                serde_json::from_str(include_str!(
                    "../../../fixtures/last-minute/media-change.json"
                ))
                .unwrap(),
            ),
            (
                "wrapper.json",
                serde_json::from_str(include_str!("../../../fixtures/last-minute/wrapper.json"))
                    .unwrap(),
            ),
            (
                "emergency-pull.json",
                serde_json::from_str(include_str!(
                    "../../../fixtures/last-minute/emergency-pull.json"
                ))
                .unwrap(),
            ),
        ];
        for (name, spec) in cases {
            if spec["kind"] == "pull" {
                let mut ev = crate::evaluate_xml(
                    include_str!("../../../fixtures/ok-https-mp4.xml"),
                    Pack::None,
                );
                crate::apply_pull(&mut ev, spec["reason"].as_str().unwrap());
                assert_eq!(ev.lifecycle, crate::Lifecycle::Expired, "{name}");
                assert!(crate::fail_closed(&ev, ev.issued_at), "{name}");
                continue;
            }
            let old = xml(spec["old"].as_str().unwrap());
            let new = xml(spec["new"].as_str().unwrap());
            let policy = crate::discover_policy("live-ssai").ok();
            let d = diff_vast_with_policy(old, new, policy.as_ref());
            assert_eq!(
                d.tracking_only,
                spec["expect"]["tracking_only"].as_bool().unwrap(),
                "{name}"
            );
            assert_eq!(
                d.keep_transcode,
                spec["expect"]["keep_transcode"].as_bool().unwrap(),
                "{name}"
            );
            let ids: Vec<&str> = spec["expect"]["invalidation_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            for id in ids {
                assert!(
                    d.invalidations.iter().any(|i| i.requirement_id == id),
                    "{name} missing {id}"
                );
            }
            if spec["expect"]["revalidation"].as_bool() == Some(true) {
                let mut ev = crate::evaluate_xml(old, Pack::None);
                if d.invalidations
                    .iter()
                    .any(|i| i.requirement_id == "diff.wrapper-changed")
                {
                    ev.lifecycle = crate::Lifecycle::RevalidationRequired;
                }
                assert_eq!(
                    ev.lifecycle,
                    crate::Lifecycle::RevalidationRequired,
                    "{name}"
                );
            }
        }
    }

    #[test]
    fn apply_diff_marks_revalidation_and_drops_transcode_on_media_change() {
        let old = include_str!("../../../fixtures/ok-https-mp4.xml");
        let new = include_str!("../../../fixtures/ok-mezzanine-new-media.xml");
        let d = diff_vast(old, new);
        let mut ev = crate::evaluate_xml(old, Pack::None);
        ev.prepared_media = Some(crate::PreparedMedia {
            ladder_id: "live-ssai-default".into(),
            source_hash: "aa".into(),
            renditions: vec![crate::PreparedRendition {
                name: "720p".into(),
                sha256: "bb".into(),
            }],
            ..Default::default()
        });
        apply_diff(&mut ev, &d);
        assert_eq!(ev.lifecycle, crate::Lifecycle::RevalidationRequired);
        assert!(ev.prepared_media.is_none());
        assert_eq!(ev.dimensions.technical, crate::DimensionState::NotReady);
    }

    fn xml(name: &str) -> &'static str {
        match name {
            "ok-https-mp4.xml" => include_str!("../../../fixtures/ok-https-mp4.xml"),
            "ok-https-mp4-tracking.xml" => {
                include_str!("../../../fixtures/ok-https-mp4-tracking.xml")
            }
            "ok-mezzanine-new-media.xml" => {
                include_str!("../../../fixtures/ok-mezzanine-new-media.xml")
            }
            "wrapper.xml" => include_str!("../../../fixtures/wrapper.xml"),
            other => panic!("unknown fixture {other}"),
        }
    }
}
