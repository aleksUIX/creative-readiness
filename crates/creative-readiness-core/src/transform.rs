//! W16: ladder encodes inherit; media replacement mints a new identity.

use serde::{Deserialize, Serialize};

use crate::{
    apply_prepared_media, DimensionState, Finding, Lifecycle, PreparedMedia, ReadinessEvidence,
    TranscodeLadder, TransformationRights,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformationKind {
    Encode,
    MediaReplacement,
}

pub fn transform(
    prior: &ReadinessEvidence,
    kind: TransformationKind,
    prepared: Option<&PreparedMedia>,
    rights: Option<&TransformationRights>,
    ladder: Option<&TranscodeLadder>,
) -> ReadinessEvidence {
    let rights = rights.cloned().unwrap_or_default();
    let inherit = match kind {
        TransformationKind::Encode => rights.inherit_on_encode,
        TransformationKind::MediaReplacement => !rights.mint_on_media_replacement,
    };
    let mut ev = prior.clone();
    if inherit {
        if let Some(media) = prepared {
            ev.dimensions.operational = apply_prepared_media(ladder, media, &mut ev.findings);
            ev.prepared_media = Some(media.clone());
        }
        ev.findings.push(Finding {
            requirement_id: "transform.encode-inherited".into(),
            severity: "info".into(),
            message:
                "ladder encode inherited technical readiness; operational bound to new renditions."
                    .into(),
            remediable: Some(false),
            dimension: Some("operational".into()),
            ..Default::default()
        });
        ev.lifecycle = if ev.dimensions.operational == DimensionState::Ready
            && ev.dimensions.technical == DimensionState::Ready
            && ev.dimensions.policy != DimensionState::NotReady
        {
            Lifecycle::OperationallyReady
        } else if ev.dimensions.technical == DimensionState::Ready
            && ev.dimensions.policy != DimensionState::NotReady
        {
            Lifecycle::TechnicallyReady
        } else {
            ev.lifecycle
        };
        return ev;
    }
    ev.prepared_media = None;
    ev.operational_claims = None;
    ev.dimensions.technical = DimensionState::Unknown;
    ev.dimensions.operational = DimensionState::Unknown;
    ev.dimensions.policy = DimensionState::Unknown;
    ev.lifecycle = Lifecycle::Unknown;
    ev.identity.fingerprint = None;
    ev.identity.content_hash = None;
    ev.findings.push(Finding {
        requirement_id: "transform.mint-identity".into(),
        severity: "error".into(),
        message: "media replacement mints a new identity; prior evidence does not carry.".into(),
        remediable: Some(true),
        dimension: Some("technical".into()),
        ..Default::default()
    });
    ev
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{evaluate, EvaluateRequest, OperationalClaims, Pack};

    fn prior() -> ReadinessEvidence {
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
    fn encode_inherits_technical() {
        let prior = prior();
        let media: PreparedMedia =
            serde_json::from_str(include_str!("../../../fixtures/prepared-ok.json")).unwrap();
        let ev = transform(&prior, TransformationKind::Encode, Some(&media), None, None);
        assert_eq!(ev.dimensions.technical, DimensionState::Ready);
        assert_eq!(ev.dimensions.operational, DimensionState::Ready);
        assert!(ev.prepared_media.is_some());
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "transform.encode-inherited"));
    }

    #[test]
    fn encode_with_mismatched_ladder_is_not_ready() {
        let prior = prior();
        let media: PreparedMedia =
            serde_json::from_str(include_str!("../../../fixtures/prepared-ok.json")).unwrap();
        let ladder = TranscodeLadder {
            id: "other-ladder".into(),
            packaging: Some("hls".into()),
            renditions: Vec::new(),
        };
        let ev = transform(
            &prior,
            TransformationKind::Encode,
            Some(&media),
            None,
            Some(&ladder),
        );
        assert_eq!(ev.dimensions.operational, DimensionState::NotReady);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "operational.ladder-mismatch"));
    }

    #[test]
    fn media_replacement_mints() {
        let prior = prior();
        let ev = transform(
            &prior,
            TransformationKind::MediaReplacement,
            None,
            None,
            None,
        );
        assert_eq!(ev.lifecycle, Lifecycle::Unknown);
        assert!(ev.prepared_media.is_none());
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "transform.mint-identity"));
    }
}
