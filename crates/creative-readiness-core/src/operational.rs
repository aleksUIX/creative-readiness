//! Claim-side operational readiness (no ffmpeg). The probe crate produces
//! [`PreparedMedia`]; this module only checks assertions against a ladder.

use serde::{Deserialize, Serialize};

use crate::policy::TranscodeLadder;
use crate::{DimensionState, Finding};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OperationalClaims {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcoded: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packaged: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rendition_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_ready: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub received: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trackers_preserved: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mapped_ad_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PreparedMedia {
    pub ladder_id: String,
    pub source_hash: String,
    pub renditions: Vec<PreparedRendition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packaging: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trackers_preserved: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mapped_ad_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreparedRendition {
    pub name: String,
    pub sha256: String,
}

pub fn apply_prepared_media(
    ladder: Option<&TranscodeLadder>,
    media: &PreparedMedia,
    findings: &mut Vec<Finding>,
) -> DimensionState {
    if media.source_hash.is_empty()
        || media
            .renditions
            .iter()
            .any(|r| r.name.is_empty() || r.sha256.is_empty())
    {
        findings.push(Finding {
            requirement_id: "operational.prepared-media-incomplete".into(),
            severity: "error".into(),
            message: "prepared media is missing a source or rendition hash.".into(),
            remediable: Some(true),
            dimension: Some("operational".into()),
            ..Default::default()
        });
        return DimensionState::NotReady;
    }
    if media.trackers_preserved == Some(false) {
        findings.push(Finding {
            requirement_id: "operational.trackers-not-preserved".into(),
            severity: "error".into(),
            message: "prepared media asserts trackers were not preserved.".into(),
            remediable: Some(true),
            dimension: Some("operational".into()),
            ..Default::default()
        });
        return DimensionState::NotReady;
    }
    let Some(ladder) = ladder else {
        return DimensionState::Ready;
    };
    if media.ladder_id != ladder.id {
        findings.push(Finding {
            requirement_id: "operational.ladder-mismatch".into(),
            severity: "error".into(),
            message: format!(
                "prepared media ladder {} does not match policy ladder {}.",
                media.ladder_id, ladder.id
            ),
            remediable: Some(true),
            dimension: Some("operational".into()),
            ..Default::default()
        });
        return DimensionState::NotReady;
    }
    if let (Some(want), Some(got)) = (ladder.packaging.as_deref(), media.packaging.as_deref()) {
        if !want.eq_ignore_ascii_case(got) {
            findings.push(Finding {
                requirement_id: "operational.packaging-mismatch".into(),
                severity: "error".into(),
                message: format!(
                    "prepared media packaging {got} does not match policy packaging {want}."
                ),
                remediable: Some(true),
                dimension: Some("operational".into()),
                ..Default::default()
            });
            return DimensionState::NotReady;
        }
    }
    if media.renditions.len() < ladder.renditions.len() {
        findings.push(Finding {
            requirement_id: "operational.renditions-incomplete".into(),
            severity: "error".into(),
            message: format!(
                "ladder needs {} rendition(s); prepared media has {}.",
                ladder.renditions.len(),
                media.renditions.len()
            ),
            remediable: Some(true),
            dimension: Some("operational".into()),
            ..Default::default()
        });
        return DimensionState::NotReady;
    }
    DimensionState::Ready
}

pub fn apply_claims(
    ladder: Option<&TranscodeLadder>,
    claims: &OperationalClaims,
    findings: &mut Vec<Finding>,
) -> DimensionState {
    if claims.transcoded != Some(true) {
        findings.push(Finding {
            requirement_id: "operational.not-transcoded".into(),
            severity: "error".into(),
            message: "operational claims do not assert transcoding is complete.".into(),
            remediable: Some(true),
            dimension: Some("operational".into()),
            ..Default::default()
        });
        return DimensionState::NotReady;
    }
    if ladder.is_some() {
        if claims.packaged != Some(true) {
            findings.push(Finding {
                requirement_id: "operational.not-packaged".into(),
                severity: "error".into(),
                message: "operational claims do not assert packaging is complete.".into(),
                remediable: Some(true),
                dimension: Some("operational".into()),
                ..Default::default()
            });
            return DimensionState::NotReady;
        }
        if claims.cached != Some(true) {
            findings.push(Finding {
                requirement_id: "operational.not-cached".into(),
                severity: "error".into(),
                message: "operational claims do not assert a cache assertion.".into(),
                remediable: Some(true),
                dimension: Some("operational".into()),
                ..Default::default()
            });
            return DimensionState::NotReady;
        }
    }
    if claims.received == Some(false) {
        findings.push(Finding {
            requirement_id: "operational.not-received".into(),
            severity: "error".into(),
            message: "operational claims assert the asset was not received.".into(),
            remediable: Some(true),
            dimension: Some("operational".into()),
            ..Default::default()
        });
        return DimensionState::NotReady;
    }
    if claims.accessible == Some(false) {
        findings.push(Finding {
            requirement_id: "operational.not-accessible".into(),
            severity: "error".into(),
            message: "operational claims assert the asset is not accessible.".into(),
            remediable: Some(true),
            dimension: Some("operational".into()),
            ..Default::default()
        });
        return DimensionState::NotReady;
    }
    if claims.trackers_preserved == Some(false) {
        findings.push(Finding {
            requirement_id: "operational.trackers-not-preserved".into(),
            severity: "error".into(),
            message: "operational claims assert trackers were not preserved.".into(),
            remediable: Some(true),
            dimension: Some("operational".into()),
            ..Default::default()
        });
        return DimensionState::NotReady;
    }
    let needed = ladder.map(|l| l.renditions.len()).unwrap_or(0);
    let got = claims.rendition_count.unwrap_or(0);
    if needed > 0 && got < needed {
        findings.push(Finding {
            requirement_id: "operational.renditions-incomplete".into(),
            severity: "error".into(),
            message: format!("ladder needs {needed} rendition(s); claims report {got}."),
            remediable: Some(true),
            dimension: Some("operational".into()),
            ..Default::default()
        });
        return DimensionState::NotReady;
    }
    DimensionState::Ready
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::discover_policy;

    #[test]
    fn live_ssai_claims_need_rendition() {
        let p = discover_policy("live-ssai").unwrap();
        let mut findings = Vec::new();
        let state = apply_claims(
            p.transcode_ladder.as_ref(),
            &OperationalClaims {
                transcoded: Some(true),
                packaged: Some(true),
                cached: Some(true),
                rendition_count: Some(0),
                ..Default::default()
            },
            &mut findings,
        );
        assert_eq!(state, DimensionState::NotReady);
        assert!(findings
            .iter()
            .any(|f| f.requirement_id == "operational.renditions-incomplete"));
    }

    #[test]
    fn live_ssai_claims_ok() {
        let p = discover_policy("live-ssai").unwrap();
        let mut findings = Vec::new();
        let state = apply_claims(
            p.transcode_ladder.as_ref(),
            &OperationalClaims {
                transcoded: Some(true),
                packaged: Some(true),
                cached: Some(true),
                rendition_count: Some(1),
                ..Default::default()
            },
            &mut findings,
        );
        assert_eq!(state, DimensionState::Ready);
    }

    #[test]
    fn prepared_media_must_match_ladder() {
        let p = discover_policy("live-ssai").unwrap();
        let mut findings = Vec::new();
        let state = apply_prepared_media(
            p.transcode_ladder.as_ref(),
            &PreparedMedia {
                ladder_id: "other".into(),
                source_hash: "aa".into(),
                renditions: vec![PreparedRendition {
                    name: "720p".into(),
                    sha256: "bb".into(),
                }],
                packaging: None,
                trackers_preserved: None,
                mapped_ad_id: None,
            },
            &mut findings,
        );
        assert_eq!(state, DimensionState::NotReady);
        assert!(findings
            .iter()
            .any(|f| f.requirement_id == "operational.ladder-mismatch"));
    }
}
