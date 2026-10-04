//! Clock and emergency pull. Expired evidence fails closed.

use time::OffsetDateTime;

use crate::{Finding, Lifecycle, ReadinessEvidence};

pub fn apply_clock(ev: &mut ReadinessEvidence, now: OffsetDateTime) {
    if now >= ev.expires_at && ev.lifecycle != Lifecycle::Expired {
        ev.lifecycle = Lifecycle::Expired;
        ev.findings.push(Finding {
            requirement_id: "evidence.expired".into(),
            severity: "error".into(),
            message: "evidence is past expires_at; fail closed even if dimensions were green."
                .into(),
            remediable: Some(true),
            ..Default::default()
        });
    }
}

/// Emergency pull: expire regardless of remaining TTL.
pub fn apply_pull(ev: &mut ReadinessEvidence, reason: &str) {
    ev.lifecycle = Lifecycle::Expired;
    ev.findings.push(Finding {
        requirement_id: "diff.emergency-pull".into(),
        severity: "error".into(),
        message: format!("emergency pull: {reason}"),
        remediable: Some(false),
        ..Default::default()
    });
}

/// True when this evidence must not be used to serve.
pub fn fail_closed(ev: &ReadinessEvidence, now: OffsetDateTime) -> bool {
    ev.lifecycle == Lifecycle::Expired || now >= ev.expires_at
}

/// Apply optional pull then the clock. Returns whether the evidence fails closed.
pub fn revalidate(ev: &mut ReadinessEvidence, pull: Option<&str>) -> bool {
    if let Some(reason) = pull {
        apply_pull(ev, reason);
    }
    let now = OffsetDateTime::now_utc();
    apply_clock(ev, now);
    fail_closed(ev, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{evaluate_xml, Pack};
    use time::Duration;

    #[test]
    fn expiry_fixture_fails_closed() {
        let spec: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/last-minute/expiry.json"))
                .unwrap();
        assert_eq!(spec["id"], "w11-expired");
        let mut ev = evaluate_xml(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            Pack::None,
        );
        assert_eq!(ev.dimensions.technical, crate::DimensionState::Ready);
        ev.expires_at = ev.issued_at - Duration::seconds(1);
        let now = ev.issued_at;
        assert!(fail_closed(&ev, now));
        apply_clock(&mut ev, now);
        assert_eq!(ev.lifecycle, Lifecycle::Expired);
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "evidence.expired"));
        assert!(fail_closed(&ev, now));
    }

    #[test]
    fn emergency_pull_expires_green_evidence() {
        let mut ev = evaluate_xml(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            Pack::None,
        );
        apply_pull(&mut ev, "advertiser pull");
        assert_eq!(ev.lifecycle, Lifecycle::Expired);
        assert!(fail_closed(&ev, ev.issued_at));
        assert!(ev
            .findings
            .iter()
            .any(|f| f.requirement_id == "diff.emergency-pull"));
    }
}
