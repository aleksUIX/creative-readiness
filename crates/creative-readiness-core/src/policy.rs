//! Builtin policy packs loaded from `packs/*.json`.

use serde::{Deserialize, Serialize};

use crate::eligibility::nonempty_str;
use crate::media::{aspect_matches, elem_text, media_facts, xml_has_macro, MediaFacts};
use crate::Finding;

const LIVE_SSAI: &str = include_str!("../../../packs/live-ssai.json");
const LIVE_SGAI: &str = include_str!("../../../packs/live-sgai.json");
const LIVE_CSAI: &str = include_str!("../../../packs/live-csai.json");
const LIVE_HYBRID: &str = include_str!("../../../packs/live-hybrid.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PolicyDocument {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub architecture: Option<String>,
    #[serde(default)]
    pub requirements: Vec<PolicyRequirement>,
    #[serde(default)]
    pub transcode_ladder: Option<TranscodeLadder>,
    #[serde(default)]
    pub proactive_required: bool,
    #[serde(default)]
    pub invalidation_rules: Vec<InvalidationRule>,
    #[serde(default)]
    pub transformation_rights: TransformationRights,
    #[serde(default)]
    pub unknown_creative_treatment: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub overlays: Vec<PolicyOverlay>,
    #[serde(default)]
    pub authority: Option<crate::Authority>,
    #[serde(default)]
    pub environments: Vec<PolicyEnvironment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PolicyEnvironment {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_infrastructure: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransformationRights {
    #[serde(default = "default_true")]
    pub inherit_on_encode: bool,
    #[serde(default = "default_true")]
    pub mint_on_media_replacement: bool,
}

fn default_true() -> bool {
    true
}

impl Default for TransformationRights {
    fn default() -> Self {
        Self {
            inherit_on_encode: true,
            mint_on_media_replacement: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvalidationRule {
    pub id: String,
    /// tracking_only | media_change | wrapper_change | emergency_pull
    pub when: String,
    /// keep_transcode | invalidate_technical_operational | revaluate_technical | expire
    pub action: String,
}

pub fn default_invalidation_rules() -> Vec<InvalidationRule> {
    vec![
        InvalidationRule {
            id: "provisional.tracking-only".into(),
            when: "tracking_only".into(),
            action: "keep_transcode".into(),
        },
        InvalidationRule {
            id: "provisional.media-change".into(),
            when: "media_change".into(),
            action: "invalidate_technical_operational".into(),
        },
        InvalidationRule {
            id: "provisional.wrapper-change".into(),
            when: "wrapper_change".into(),
            action: "revaluate_technical".into(),
        },
        InvalidationRule {
            id: "provisional.emergency-pull".into(),
            when: "emergency_pull".into(),
            action: "expire".into(),
        },
    ]
}

pub fn invalidation_rules(policy: Option<&PolicyDocument>) -> Vec<InvalidationRule> {
    match policy {
        Some(p) if !p.invalidation_rules.is_empty() => p.invalidation_rules.clone(),
        _ => default_invalidation_rules(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuctionTriggered {
    Allowed,
    Forbidden,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PolicyOverlay {
    pub id: String,
    #[serde(default)]
    pub deal_id: Option<String>,
    #[serde(default)]
    pub publisher_id: Option<String>,
    #[serde(default)]
    pub event_id: Option<String>,
    #[serde(default)]
    pub device_family: Option<String>,
    #[serde(default)]
    pub auction_triggered: Option<AuctionTriggered>,
    #[serde(default)]
    pub requirements: Vec<PolicyRequirement>,
}

pub fn parse_overlay(json: &str) -> Result<PolicyOverlay, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

/// One overlay object or a catalog array.
pub fn parse_overlays(json: &str) -> Result<Vec<PolicyOverlay>, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if v.is_array() {
        serde_json::from_value(v).map_err(|e| e.to_string())
    } else {
        parse_overlay(json).map(|o| vec![o])
    }
}

fn overlay_matches(o: &PolicyOverlay, opp: &crate::Opportunity) -> bool {
    if let Some(d) = nonempty_str(o.deal_id.as_deref()) {
        if nonempty_str(opp.deal_id.as_deref()) != Some(d) {
            return false;
        }
    }
    if let Some(p) = nonempty_str(o.publisher_id.as_deref()) {
        if nonempty_str(opp.publisher_id.as_deref()) != Some(p) {
            return false;
        }
    }
    if let Some(e) = nonempty_str(o.event_id.as_deref()) {
        if nonempty_str(opp.event_id.as_deref()) != Some(e) {
            return false;
        }
    }
    if let Some(dev) = nonempty_str(o.device_family.as_deref()) {
        match nonempty_str(opp.device_family.as_deref()) {
            Some(got) if got.eq_ignore_ascii_case(dev) => {}
            _ => return false,
        }
    }
    true
}

fn overlay_rank(o: &PolicyOverlay) -> u8 {
    if nonempty_str(o.deal_id.as_deref()).is_some() {
        1
    } else if nonempty_str(o.publisher_id.as_deref()).is_some() {
        2
    } else if nonempty_str(o.event_id.as_deref()).is_some() {
        3
    } else if nonempty_str(o.device_family.as_deref()).is_some() {
        4
    } else {
        0
    }
}

/// Deal, then publisher, then event, then device. Unkeyed overlays always apply.
pub fn select_overlays(catalog: &[PolicyOverlay], opp: &crate::Opportunity) -> Vec<PolicyOverlay> {
    let mut matched: Vec<&PolicyOverlay> =
        catalog.iter().filter(|o| overlay_matches(o, opp)).collect();
    matched.sort_by_key(|o| overlay_rank(o));
    matched.into_iter().cloned().collect()
}

pub fn pack_expired(policy: &PolicyDocument, now: time::OffsetDateTime) -> bool {
    let Some(raw) = policy.expires_at.as_deref() else {
        return false;
    };
    time::OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
        .map(|t| now >= t)
        .unwrap_or(true)
}

/// Deal overlays first, then publisher overlays. Requirements are appended, never dropped.
pub fn resolve_overlays(
    deal: Option<&PolicyOverlay>,
    publishers: &[PolicyOverlay],
) -> Vec<PolicyOverlay> {
    let mut out = Vec::new();
    if let Some(d) = deal {
        out.push(d.clone());
    }
    out.extend(publishers.iter().cloned());
    out
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TranscodeLadder {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packaging: Option<String>,
    pub renditions: Vec<LadderRendition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LadderRendition {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub vcodec: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyRequirement {
    pub id: String,
    pub kind: String,
    pub message: String,
    #[serde(default)]
    pub element: Option<String>,
    #[serde(default)]
    pub max: Option<u64>,
    #[serde(default)]
    pub min: Option<u64>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub values: Option<Vec<String>>,
}

pub fn parse_policy(json: &str) -> Result<PolicyDocument, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

pub fn builtin_policy_ids() -> &'static [&'static str] {
    &["live-ssai", "live-sgai", "live-csai", "live-hybrid"]
}

/// Named builtin pack (`live-ssai`, `live-sgai`, `live-csai`, `live-hybrid`).
pub fn discover_policy(id: &str) -> Result<PolicyDocument, String> {
    let json = match id.trim().to_ascii_lowercase().as_str() {
        "live-ssai" => LIVE_SSAI,
        "live-sgai" => LIVE_SGAI,
        "live-csai" => LIVE_CSAI,
        "live-hybrid" => LIVE_HYBRID,
        other => return Err(format!("unknown policy {other:?}")),
    };
    parse_policy(json)
}

pub fn apply_policy(policy: &PolicyDocument, xml: &str, findings: &mut Vec<Finding>) {
    apply_policy_for(policy, xml, None, findings);
}

pub fn apply_policy_for(
    policy: &PolicyDocument,
    xml: &str,
    device_family: Option<&str>,
    findings: &mut Vec<Finding>,
) {
    let facts = media_facts(xml);
    let sibling = sibling_media(&policy.requirements);
    for req in &policy.requirements {
        let hit = requirement_hit(req, xml, &facts, device_family, &sibling, findings);
        if !hit {
            continue;
        }
        findings.retain(|f| {
            if req.kind == "forbid_vpaid" {
                !f.requirement_id.to_ascii_lowercase().contains("vpaid")
            } else if req.kind == "require_element"
                && nonempty_str(req.element.as_deref()) == Some("Mezzanine")
            {
                f.requirement_id != "VAST-4.1-mezzanine-recommended"
            } else {
                true
            }
        });
        findings.push(Finding {
            requirement_id: req.id.clone(),
            severity: "error".into(),
            message: req.message.clone(),
            remediable: Some(true),
            dimension: Some("policy".into()),
            ..Default::default()
        });
    }
}

fn trimmed_values(req: &PolicyRequirement) -> Vec<String> {
    req.values
        .clone()
        .unwrap_or_else(|| req.value.iter().cloned().collect())
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn skip_missing_names(findings: &mut Vec<Finding>, id: &str, message: &str) -> bool {
    findings.push(Finding {
        requirement_id: id.into(),
        severity: "warning".into(),
        message: message.into(),
        remediable: Some(true),
        dimension: Some("policy".into()),
        ..Default::default()
    });
    false
}

fn sibling_bitrate_bounds(reqs: &[PolicyRequirement]) -> (Option<u64>, Option<u64>) {
    let floor = reqs
        .iter()
        .filter(|r| r.kind == "min_bitrate")
        .filter_map(|r| r.min)
        .max();
    let cap = reqs
        .iter()
        .filter(|r| r.kind == "max_bitrate")
        .filter_map(|r| r.max)
        .min();
    (floor, cap)
}

struct SiblingMedia {
    bitrate_floor: Option<u64>,
    bitrate_cap: Option<u64>,
    min_width: Option<u64>,
    min_height: Option<u64>,
    max_width: Option<u64>,
    max_height: Option<u64>,
    aspect: Option<String>,
    codecs: Vec<String>,
    audio_codecs: Vec<String>,
    mimes: Vec<String>,
    containers: Vec<String>,
    deliveries: Vec<String>,
    min_companion_width: Option<u64>,
    min_companion_height: Option<u64>,
    max_companion_width: Option<u64>,
    max_companion_height: Option<u64>,
    min_companion_expanded_width: Option<u64>,
    min_companion_expanded_height: Option<u64>,
    max_companion_expanded_width: Option<u64>,
    max_companion_expanded_height: Option<u64>,
    min_companion_asset_width: Option<u64>,
    min_companion_asset_height: Option<u64>,
    max_companion_asset_width: Option<u64>,
    max_companion_asset_height: Option<u64>,
    min_nonlinear_expanded_width: Option<u64>,
    min_nonlinear_expanded_height: Option<u64>,
    max_nonlinear_expanded_width: Option<u64>,
    max_nonlinear_expanded_height: Option<u64>,
    min_nonlinear_width: Option<u64>,
    min_nonlinear_height: Option<u64>,
    max_nonlinear_width: Option<u64>,
    max_nonlinear_height: Option<u64>,
    min_mezzanine_width: Option<u64>,
    min_mezzanine_height: Option<u64>,
    max_mezzanine_width: Option<u64>,
    max_mezzanine_height: Option<u64>,
    min_mezzanine_bitrate: Option<u64>,
    max_mezzanine_bitrate: Option<u64>,
    min_icon_width: Option<u64>,
    min_icon_height: Option<u64>,
    max_icon_width: Option<u64>,
    max_icon_height: Option<u64>,
    min_icon_click_fallback_width: Option<u64>,
    min_icon_click_fallback_height: Option<u64>,
    max_icon_click_fallback_width: Option<u64>,
    max_icon_click_fallback_height: Option<u64>,
}

fn sibling_media(reqs: &[PolicyRequirement]) -> SiblingMedia {
    let (bitrate_floor, bitrate_cap) = sibling_bitrate_bounds(reqs);
    SiblingMedia {
        bitrate_floor,
        bitrate_cap,
        min_width: reqs
            .iter()
            .filter(|r| r.kind == "min_width")
            .filter_map(|r| r.min)
            .max(),
        min_height: reqs
            .iter()
            .filter(|r| r.kind == "min_height")
            .filter_map(|r| r.min)
            .max(),
        max_width: reqs
            .iter()
            .filter(|r| r.kind == "max_width")
            .filter_map(|r| r.max)
            .min(),
        max_height: reqs
            .iter()
            .filter(|r| r.kind == "max_height")
            .filter_map(|r| r.max)
            .min(),
        aspect: reqs
            .iter()
            .find(|r| r.kind == "aspect_ratio")
            .and_then(|r| {
                nonempty_str(r.value.as_deref())
                    .map(str::to_string)
                    .or_else(|| trimmed_values(r).into_iter().next())
            }),
        codecs: reqs
            .iter()
            .filter(|r| r.kind == "codec")
            .flat_map(trimmed_values)
            .collect(),
        audio_codecs: reqs
            .iter()
            .filter(|r| r.kind == "audio_codec")
            .flat_map(trimmed_values)
            .collect(),
        mimes: reqs
            .iter()
            .filter(|r| r.kind == "media_type")
            .flat_map(trimmed_values)
            .collect(),
        containers: reqs
            .iter()
            .filter(|r| r.kind == "container")
            .flat_map(trimmed_values)
            .collect(),
        deliveries: reqs
            .iter()
            .filter(|r| r.kind == "delivery")
            .flat_map(trimmed_values)
            .collect(),
        min_companion_width: reqs
            .iter()
            .filter(|r| r.kind == "min_companion_width")
            .filter_map(|r| r.min)
            .max(),
        min_companion_height: reqs
            .iter()
            .filter(|r| r.kind == "min_companion_height")
            .filter_map(|r| r.min)
            .max(),
        max_companion_width: reqs
            .iter()
            .filter(|r| r.kind == "max_companion_width")
            .filter_map(|r| r.max)
            .min(),
        max_companion_height: reqs
            .iter()
            .filter(|r| r.kind == "max_companion_height")
            .filter_map(|r| r.max)
            .min(),
        min_companion_expanded_width: reqs
            .iter()
            .filter(|r| r.kind == "min_companion_expanded_width")
            .filter_map(|r| r.min)
            .max(),
        min_companion_expanded_height: reqs
            .iter()
            .filter(|r| r.kind == "min_companion_expanded_height")
            .filter_map(|r| r.min)
            .max(),
        max_companion_expanded_width: reqs
            .iter()
            .filter(|r| r.kind == "max_companion_expanded_width")
            .filter_map(|r| r.max)
            .min(),
        max_companion_expanded_height: reqs
            .iter()
            .filter(|r| r.kind == "max_companion_expanded_height")
            .filter_map(|r| r.max)
            .min(),
        min_companion_asset_width: reqs
            .iter()
            .filter(|r| r.kind == "min_companion_asset_width")
            .filter_map(|r| r.min)
            .max(),
        min_companion_asset_height: reqs
            .iter()
            .filter(|r| r.kind == "min_companion_asset_height")
            .filter_map(|r| r.min)
            .max(),
        max_companion_asset_width: reqs
            .iter()
            .filter(|r| r.kind == "max_companion_asset_width")
            .filter_map(|r| r.max)
            .min(),
        max_companion_asset_height: reqs
            .iter()
            .filter(|r| r.kind == "max_companion_asset_height")
            .filter_map(|r| r.max)
            .min(),
        min_nonlinear_expanded_width: reqs
            .iter()
            .filter(|r| r.kind == "min_nonlinear_expanded_width")
            .filter_map(|r| r.min)
            .max(),
        min_nonlinear_expanded_height: reqs
            .iter()
            .filter(|r| r.kind == "min_nonlinear_expanded_height")
            .filter_map(|r| r.min)
            .max(),
        max_nonlinear_expanded_width: reqs
            .iter()
            .filter(|r| r.kind == "max_nonlinear_expanded_width")
            .filter_map(|r| r.max)
            .min(),
        max_nonlinear_expanded_height: reqs
            .iter()
            .filter(|r| r.kind == "max_nonlinear_expanded_height")
            .filter_map(|r| r.max)
            .min(),
        min_nonlinear_width: reqs
            .iter()
            .filter(|r| r.kind == "min_nonlinear_width")
            .filter_map(|r| r.min)
            .max(),
        min_nonlinear_height: reqs
            .iter()
            .filter(|r| r.kind == "min_nonlinear_height")
            .filter_map(|r| r.min)
            .max(),
        max_nonlinear_width: reqs
            .iter()
            .filter(|r| r.kind == "max_nonlinear_width")
            .filter_map(|r| r.max)
            .min(),
        max_nonlinear_height: reqs
            .iter()
            .filter(|r| r.kind == "max_nonlinear_height")
            .filter_map(|r| r.max)
            .min(),
        min_mezzanine_width: reqs
            .iter()
            .filter(|r| r.kind == "min_mezzanine_width")
            .filter_map(|r| r.min)
            .max(),
        min_mezzanine_height: reqs
            .iter()
            .filter(|r| r.kind == "min_mezzanine_height")
            .filter_map(|r| r.min)
            .max(),
        max_mezzanine_width: reqs
            .iter()
            .filter(|r| r.kind == "max_mezzanine_width")
            .filter_map(|r| r.max)
            .min(),
        max_mezzanine_height: reqs
            .iter()
            .filter(|r| r.kind == "max_mezzanine_height")
            .filter_map(|r| r.max)
            .min(),
        min_mezzanine_bitrate: reqs
            .iter()
            .filter(|r| r.kind == "min_mezzanine_bitrate")
            .filter_map(|r| r.min)
            .max(),
        max_mezzanine_bitrate: reqs
            .iter()
            .filter(|r| r.kind == "max_mezzanine_bitrate")
            .filter_map(|r| r.max)
            .min(),
        min_icon_width: reqs
            .iter()
            .filter(|r| r.kind == "min_icon_width")
            .filter_map(|r| r.min)
            .max(),
        min_icon_height: reqs
            .iter()
            .filter(|r| r.kind == "min_icon_height")
            .filter_map(|r| r.min)
            .max(),
        max_icon_width: reqs
            .iter()
            .filter(|r| r.kind == "max_icon_width")
            .filter_map(|r| r.max)
            .min(),
        max_icon_height: reqs
            .iter()
            .filter(|r| r.kind == "max_icon_height")
            .filter_map(|r| r.max)
            .min(),
        min_icon_click_fallback_width: reqs
            .iter()
            .filter(|r| r.kind == "min_icon_click_fallback_width")
            .filter_map(|r| r.min)
            .max(),
        min_icon_click_fallback_height: reqs
            .iter()
            .filter(|r| r.kind == "min_icon_click_fallback_height")
            .filter_map(|r| r.min)
            .max(),
        max_icon_click_fallback_width: reqs
            .iter()
            .filter(|r| r.kind == "max_icon_click_fallback_width")
            .filter_map(|r| r.max)
            .min(),
        max_icon_click_fallback_height: reqs
            .iter()
            .filter(|r| r.kind == "max_icon_click_fallback_height")
            .filter_map(|r| r.max)
            .min(),
    }
}

fn hay_has_any(hay: &str, names: &[String]) -> bool {
    names.iter().any(|a| hay.contains(&a.to_ascii_lowercase()))
}

fn facts_media_row(facts: &MediaFacts) -> crate::media::MediaFileRow {
    let hay = [
        facts.codec.as_deref(),
        facts.codecs.as_deref(),
        facts.audio_codec.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(",")
    .to_ascii_lowercase();
    crate::media::MediaFileRow {
        mime: facts.mime.clone(),
        container: facts.container.clone(),
        width: facts.width,
        height: facts.height,
        bitrate: facts.bitrate,
        codecs_hay: if hay.is_empty() { None } else { Some(hay) },
        delivery: None,
    }
}

fn media_rows(xml: &str, facts: &MediaFacts) -> Vec<crate::media::MediaFileRow> {
    let mut rows = crate::media::media_file_rows(xml);
    if rows.is_empty() {
        rows.push(facts_media_row(facts));
    }
    rows
}

fn row_bitrate_ok(row: &crate::media::MediaFileRow, min: Option<u64>, max: Option<u64>) -> bool {
    match row.bitrate {
        Some(b) => min.map(|m| b >= m).unwrap_or(true) && max.map(|m| b <= m).unwrap_or(true),
        None => min.is_none() && max.is_none(),
    }
}

fn row_codec_ok(row: &crate::media::MediaFileRow, video: &[String], audio: &[String]) -> bool {
    let hay = row.codecs_hay.as_deref().unwrap_or("");
    (video.is_empty() || hay_has_any(hay, video)) && (audio.is_empty() || hay_has_any(hay, audio))
}

fn row_size_ok(
    row: &crate::media::MediaFileRow,
    min_w: Option<u64>,
    min_h: Option<u64>,
    max_w: Option<u64>,
    max_h: Option<u64>,
    aspect: Option<&str>,
) -> bool {
    let w_ok = min_w
        .map(|m| row.width.map(|w| u64::from(w) >= m).unwrap_or(false))
        .unwrap_or(true);
    let h_ok = min_h
        .map(|m| row.height.map(|h| u64::from(h) >= m).unwrap_or(false))
        .unwrap_or(true);
    let max_w_ok = max_w
        .map(|m| row.width.map(|w| u64::from(w) <= m).unwrap_or(false))
        .unwrap_or(true);
    let max_h_ok = max_h
        .map(|m| row.height.map(|h| u64::from(h) <= m).unwrap_or(false))
        .unwrap_or(true);
    let a_ok = aspect
        .map(|a| aspect_matches(row.width, row.height, a))
        .unwrap_or(true);
    w_ok && h_ok && max_w_ok && max_h_ok && a_ok
}

fn row_type_ok(
    row: &crate::media::MediaFileRow,
    mimes: &[String],
    containers: &[String],
    deliveries: &[String],
) -> bool {
    let mime_ok = mimes.is_empty()
        || row
            .mime
            .as_ref()
            .is_some_and(|m| mimes.iter().any(|a| a.eq_ignore_ascii_case(m)));
    let box_ok = containers.is_empty()
        || row
            .container
            .as_ref()
            .is_some_and(|c| containers.iter().any(|a| a.eq_ignore_ascii_case(c)));
    let delivery_ok = deliveries.is_empty()
        || row
            .delivery
            .as_ref()
            .is_some_and(|d| deliveries.iter().any(|a| a.eq_ignore_ascii_case(d)));
    mime_ok && box_ok && delivery_ok
}

fn row_meets(
    row: &crate::media::MediaFileRow,
    min_w: Option<u64>,
    min_h: Option<u64>,
    max_w: Option<u64>,
    max_h: Option<u64>,
    aspect: Option<&str>,
    min_br: Option<u64>,
    max_br: Option<u64>,
    mimes: &[String],
    containers: &[String],
    deliveries: &[String],
    video: &[String],
    audio: &[String],
) -> bool {
    row_size_ok(row, min_w, min_h, max_w, max_h, aspect)
        && row_bitrate_ok(row, min_br, max_br)
        && row_codec_ok(row, video, audio)
        && row_type_ok(row, mimes, containers, deliveries)
}

fn codec_pair_misses(
    xml: &str,
    facts: &MediaFacts,
    sibling: &SiblingMedia,
    video: &[String],
    audio: &[String],
) -> bool {
    !media_rows(xml, facts).iter().any(|row| {
        row_meets(
            row,
            sibling.min_width,
            sibling.min_height,
            sibling.max_width,
            sibling.max_height,
            sibling.aspect.as_deref(),
            sibling.bitrate_floor,
            sibling.bitrate_cap,
            &sibling.mimes,
            &sibling.containers,
            &sibling.deliveries,
            video,
            audio,
        )
    })
}

fn bitrate_misses_range(
    xml: &str,
    facts: &MediaFacts,
    sibling: &SiblingMedia,
    min: Option<u64>,
    max: Option<u64>,
) -> bool {
    !media_rows(xml, facts).iter().any(|row| {
        row_meets(
            row,
            sibling.min_width,
            sibling.min_height,
            sibling.max_width,
            sibling.max_height,
            sibling.aspect.as_deref(),
            min,
            max,
            &sibling.mimes,
            &sibling.containers,
            &sibling.deliveries,
            &sibling.codecs,
            &sibling.audio_codecs,
        )
    })
}

fn size_misses(
    xml: &str,
    facts: &MediaFacts,
    sibling: &SiblingMedia,
    min_w: Option<u64>,
    min_h: Option<u64>,
    max_w: Option<u64>,
    max_h: Option<u64>,
    aspect: Option<&str>,
) -> bool {
    !media_rows(xml, facts).iter().any(|row| {
        row_meets(
            row,
            min_w,
            min_h,
            max_w,
            max_h,
            aspect,
            sibling.bitrate_floor,
            sibling.bitrate_cap,
            &sibling.mimes,
            &sibling.containers,
            &sibling.deliveries,
            &sibling.codecs,
            &sibling.audio_codecs,
        )
    })
}

fn type_container_misses(
    xml: &str,
    facts: &MediaFacts,
    sibling: &SiblingMedia,
    mimes: &[String],
    containers: &[String],
    deliveries: &[String],
) -> bool {
    !media_rows(xml, facts).iter().any(|row| {
        row_meets(
            row,
            sibling.min_width,
            sibling.min_height,
            sibling.max_width,
            sibling.max_height,
            sibling.aspect.as_deref(),
            sibling.bitrate_floor,
            sibling.bitrate_cap,
            mimes,
            containers,
            deliveries,
            &sibling.codecs,
            &sibling.audio_codecs,
        )
    })
}

fn requirement_hit(
    req: &PolicyRequirement,
    xml: &str,
    facts: &MediaFacts,
    device_family: Option<&str>,
    sibling: &SiblingMedia,
    findings: &mut Vec<Finding>,
) -> bool {
    match req.kind.as_str() {
        "forbid_vpaid" => crate::media::has_vpaid_framework(xml),
        "require_element" => {
            let Some(elem) = nonempty_str(req.element.as_deref()) else {
                findings.push(Finding {
                    requirement_id: "policy.require-element-missing-name".into(),
                    severity: "warning".into(),
                    message: "require_element has no element name; skipped.".into(),
                    remediable: Some(true),
                    dimension: Some("policy".into()),
                    ..Default::default()
                });
                return false;
            };
            if elem.eq_ignore_ascii_case("UniversalAdId") {
                crate::media::universal_ad_id_text(xml).is_none()
            } else {
                elem_text(xml, elem).is_none()
            }
        }
        "media_https" => {
            let media_ok = crate::media::any_https_mediafile(xml);
            let mezz_ok = crate::media::any_https_mezzanine(xml);
            !(media_ok && mezz_ok)
        }
        "max_duration_seconds" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-duration-missing-max",
                    "max_duration_seconds has no max; skipped.",
                );
            };
            crate::media::duration_exceeds(xml, max)
        }
        "min_duration_seconds" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-duration-missing-min",
                    "min_duration_seconds has no min; skipped.",
                );
            };
            crate::media::duration_below(xml, min)
        }
        "max_skipoffset_seconds" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-skipoffset-missing-max",
                    "max_skipoffset_seconds has no max; skipped.",
                );
            };
            crate::media::skipoffset_exceeds(xml, max)
        }
        "min_suggested_duration_seconds" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-suggested-duration-missing-min",
                    "min_suggested_duration_seconds has no min; skipped.",
                );
            };
            crate::media::min_suggested_duration_below(xml, min)
        }
        "min_icon_duration_seconds" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-icon-duration-missing-min",
                    "min_icon_duration_seconds has no min; skipped.",
                );
            };
            crate::media::min_icon_duration_below(xml, min)
        }
        "max_icon_offset_seconds" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-icon-offset-missing-max",
                    "max_icon_offset_seconds has no max; skipped.",
                );
            };
            crate::media::max_icon_offset_exceeds(xml, max)
        }
        "min_companion_width" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-companion-width-missing-min",
                    "min_companion_width has no min; skipped.",
                );
            };
            crate::media::companion_size_range_misses(
                xml,
                Some(min),
                sibling.min_companion_height,
                sibling.max_companion_width,
                sibling.max_companion_height,
            )
        }
        "min_companion_height" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-companion-height-missing-min",
                    "min_companion_height has no min; skipped.",
                );
            };
            crate::media::companion_size_range_misses(
                xml,
                sibling.min_companion_width,
                Some(min),
                sibling.max_companion_width,
                sibling.max_companion_height,
            )
        }
        "min_icon_width" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-icon-width-missing-min",
                    "min_icon_width has no min; skipped.",
                );
            };
            crate::media::icon_size_range_misses(
                xml,
                Some(min),
                sibling.min_icon_height,
                sibling.max_icon_width,
                sibling.max_icon_height,
            )
        }
        "min_icon_height" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-icon-height-missing-min",
                    "min_icon_height has no min; skipped.",
                );
            };
            crate::media::icon_size_range_misses(
                xml,
                sibling.min_icon_width,
                Some(min),
                sibling.max_icon_width,
                sibling.max_icon_height,
            )
        }
        "min_icon_click_fallback_width" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-icon-click-fallback-width-missing-min",
                    "min_icon_click_fallback_width has no min; skipped.",
                );
            };
            crate::media::icon_click_fallback_size_range_misses(
                xml,
                Some(min),
                sibling.min_icon_click_fallback_height,
                sibling.max_icon_click_fallback_width,
                sibling.max_icon_click_fallback_height,
            )
        }
        "min_icon_click_fallback_height" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-icon-click-fallback-height-missing-min",
                    "min_icon_click_fallback_height has no min; skipped.",
                );
            };
            crate::media::icon_click_fallback_size_range_misses(
                xml,
                sibling.min_icon_click_fallback_width,
                Some(min),
                sibling.max_icon_click_fallback_width,
                sibling.max_icon_click_fallback_height,
            )
        }
        "min_companion_expanded_width" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-companion-expanded-width-missing-min",
                    "min_companion_expanded_width has no min; skipped.",
                );
            };
            crate::media::companion_expanded_size_range_misses(
                xml,
                Some(min),
                sibling.min_companion_expanded_height,
                sibling.max_companion_expanded_width,
                sibling.max_companion_expanded_height,
            )
        }
        "min_companion_expanded_height" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-companion-expanded-height-missing-min",
                    "min_companion_expanded_height has no min; skipped.",
                );
            };
            crate::media::companion_expanded_size_range_misses(
                xml,
                sibling.min_companion_expanded_width,
                Some(min),
                sibling.max_companion_expanded_width,
                sibling.max_companion_expanded_height,
            )
        }
        "min_companion_asset_width" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-companion-asset-width-missing-min",
                    "min_companion_asset_width has no min; skipped.",
                );
            };
            crate::media::companion_asset_size_range_misses(
                xml,
                Some(min),
                sibling.min_companion_asset_height,
                sibling.max_companion_asset_width,
                sibling.max_companion_asset_height,
            )
        }
        "min_companion_asset_height" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-companion-asset-height-missing-min",
                    "min_companion_asset_height has no min; skipped.",
                );
            };
            crate::media::companion_asset_size_range_misses(
                xml,
                sibling.min_companion_asset_width,
                Some(min),
                sibling.max_companion_asset_width,
                sibling.max_companion_asset_height,
            )
        }
        "min_nonlinear_expanded_width" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-nonlinear-expanded-width-missing-min",
                    "min_nonlinear_expanded_width has no min; skipped.",
                );
            };
            crate::media::nonlinear_expanded_size_range_misses(
                xml,
                Some(min),
                sibling.min_nonlinear_expanded_height,
                sibling.max_nonlinear_expanded_width,
                sibling.max_nonlinear_expanded_height,
            )
        }
        "min_nonlinear_expanded_height" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-nonlinear-expanded-height-missing-min",
                    "min_nonlinear_expanded_height has no min; skipped.",
                );
            };
            crate::media::nonlinear_expanded_size_range_misses(
                xml,
                sibling.min_nonlinear_expanded_width,
                Some(min),
                sibling.max_nonlinear_expanded_width,
                sibling.max_nonlinear_expanded_height,
            )
        }
        "min_nonlinear_width" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-nonlinear-width-missing-min",
                    "min_nonlinear_width has no min; skipped.",
                );
            };
            crate::media::nonlinear_size_range_misses(
                xml,
                Some(min),
                sibling.min_nonlinear_height,
                sibling.max_nonlinear_width,
                sibling.max_nonlinear_height,
            )
        }
        "min_nonlinear_height" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-nonlinear-height-missing-min",
                    "min_nonlinear_height has no min; skipped.",
                );
            };
            crate::media::nonlinear_size_range_misses(
                xml,
                sibling.min_nonlinear_width,
                Some(min),
                sibling.max_nonlinear_width,
                sibling.max_nonlinear_height,
            )
        }
        "min_mezzanine_file_size" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-mezzanine-file-size-missing-min",
                    "min_mezzanine_file_size has no min; skipped.",
                );
            };
            crate::media::mezzanine_file_size_below(xml, min)
        }
        "min_mezzanine_width" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-mezzanine-width-missing-min",
                    "min_mezzanine_width has no min; skipped.",
                );
            };
            crate::media::mezzanine_size_range_misses(
                xml,
                Some(min),
                sibling.min_mezzanine_height,
                sibling.max_mezzanine_width,
                sibling.max_mezzanine_height,
            )
        }
        "min_mezzanine_height" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-mezzanine-height-missing-min",
                    "min_mezzanine_height has no min; skipped.",
                );
            };
            crate::media::mezzanine_size_range_misses(
                xml,
                sibling.min_mezzanine_width,
                Some(min),
                sibling.max_mezzanine_width,
                sibling.max_mezzanine_height,
            )
        }
        "max_mezzanine_width" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-mezzanine-width-missing-max",
                    "max_mezzanine_width has no max; skipped.",
                );
            };
            crate::media::mezzanine_size_range_misses(
                xml,
                sibling.min_mezzanine_width,
                sibling.min_mezzanine_height,
                Some(max),
                sibling.max_mezzanine_height,
            )
        }
        "max_mezzanine_height" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-mezzanine-height-missing-max",
                    "max_mezzanine_height has no max; skipped.",
                );
            };
            crate::media::mezzanine_size_range_misses(
                xml,
                sibling.min_mezzanine_width,
                sibling.min_mezzanine_height,
                sibling.max_mezzanine_width,
                Some(max),
            )
        }
        "max_companion_width" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-companion-width-missing-max",
                    "max_companion_width has no max; skipped.",
                );
            };
            crate::media::companion_size_range_misses(
                xml,
                sibling.min_companion_width,
                sibling.min_companion_height,
                Some(max),
                sibling.max_companion_height,
            )
        }
        "max_companion_height" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-companion-height-missing-max",
                    "max_companion_height has no max; skipped.",
                );
            };
            crate::media::companion_size_range_misses(
                xml,
                sibling.min_companion_width,
                sibling.min_companion_height,
                sibling.max_companion_width,
                Some(max),
            )
        }
        "max_icon_width" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-icon-width-missing-max",
                    "max_icon_width has no max; skipped.",
                );
            };
            crate::media::icon_size_range_misses(
                xml,
                sibling.min_icon_width,
                sibling.min_icon_height,
                Some(max),
                sibling.max_icon_height,
            )
        }
        "max_icon_height" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-icon-height-missing-max",
                    "max_icon_height has no max; skipped.",
                );
            };
            crate::media::icon_size_range_misses(
                xml,
                sibling.min_icon_width,
                sibling.min_icon_height,
                sibling.max_icon_width,
                Some(max),
            )
        }
        "max_nonlinear_width" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-nonlinear-width-missing-max",
                    "max_nonlinear_width has no max; skipped.",
                );
            };
            crate::media::nonlinear_size_range_misses(
                xml,
                sibling.min_nonlinear_width,
                sibling.min_nonlinear_height,
                Some(max),
                sibling.max_nonlinear_height,
            )
        }
        "max_nonlinear_height" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-nonlinear-height-missing-max",
                    "max_nonlinear_height has no max; skipped.",
                );
            };
            crate::media::nonlinear_size_range_misses(
                xml,
                sibling.min_nonlinear_width,
                sibling.min_nonlinear_height,
                sibling.max_nonlinear_width,
                Some(max),
            )
        }
        "max_icon_click_fallback_width" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-icon-click-fallback-width-missing-max",
                    "max_icon_click_fallback_width has no max; skipped.",
                );
            };
            crate::media::icon_click_fallback_size_range_misses(
                xml,
                sibling.min_icon_click_fallback_width,
                sibling.min_icon_click_fallback_height,
                Some(max),
                sibling.max_icon_click_fallback_height,
            )
        }
        "max_icon_click_fallback_height" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-icon-click-fallback-height-missing-max",
                    "max_icon_click_fallback_height has no max; skipped.",
                );
            };
            crate::media::icon_click_fallback_size_range_misses(
                xml,
                sibling.min_icon_click_fallback_width,
                sibling.min_icon_click_fallback_height,
                sibling.max_icon_click_fallback_width,
                Some(max),
            )
        }
        "max_companion_expanded_width" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-companion-expanded-width-missing-max",
                    "max_companion_expanded_width has no max; skipped.",
                );
            };
            crate::media::companion_expanded_size_range_misses(
                xml,
                sibling.min_companion_expanded_width,
                sibling.min_companion_expanded_height,
                Some(max),
                sibling.max_companion_expanded_height,
            )
        }
        "max_companion_expanded_height" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-companion-expanded-height-missing-max",
                    "max_companion_expanded_height has no max; skipped.",
                );
            };
            crate::media::companion_expanded_size_range_misses(
                xml,
                sibling.min_companion_expanded_width,
                sibling.min_companion_expanded_height,
                sibling.max_companion_expanded_width,
                Some(max),
            )
        }
        "max_companion_asset_width" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-companion-asset-width-missing-max",
                    "max_companion_asset_width has no max; skipped.",
                );
            };
            crate::media::companion_asset_size_range_misses(
                xml,
                sibling.min_companion_asset_width,
                sibling.min_companion_asset_height,
                Some(max),
                sibling.max_companion_asset_height,
            )
        }
        "max_companion_asset_height" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-companion-asset-height-missing-max",
                    "max_companion_asset_height has no max; skipped.",
                );
            };
            crate::media::companion_asset_size_range_misses(
                xml,
                sibling.min_companion_asset_width,
                sibling.min_companion_asset_height,
                sibling.max_companion_asset_width,
                Some(max),
            )
        }
        "max_nonlinear_expanded_width" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-nonlinear-expanded-width-missing-max",
                    "max_nonlinear_expanded_width has no max; skipped.",
                );
            };
            crate::media::nonlinear_expanded_size_range_misses(
                xml,
                sibling.min_nonlinear_expanded_width,
                sibling.min_nonlinear_expanded_height,
                Some(max),
                sibling.max_nonlinear_expanded_height,
            )
        }
        "max_nonlinear_expanded_height" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-nonlinear-expanded-height-missing-max",
                    "max_nonlinear_expanded_height has no max; skipped.",
                );
            };
            crate::media::nonlinear_expanded_size_range_misses(
                xml,
                sibling.min_nonlinear_expanded_width,
                sibling.min_nonlinear_expanded_height,
                sibling.max_nonlinear_expanded_width,
                Some(max),
            )
        }
        "min_mezzanine_bitrate" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-mezzanine-bitrate-missing-min",
                    "min_mezzanine_bitrate has no min; skipped.",
                );
            };
            crate::media::mezzanine_bitrate_misses_range(
                xml,
                Some(min),
                sibling.max_mezzanine_bitrate,
            )
        }
        "max_mezzanine_bitrate" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-mezzanine-bitrate-missing-max",
                    "max_mezzanine_bitrate has no max; skipped.",
                );
            };
            crate::media::mezzanine_bitrate_misses_range(
                xml,
                sibling.min_mezzanine_bitrate,
                Some(max),
            )
        }
        "min_ad_serving_id_length" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-ad-serving-id-length-missing-min",
                    "min_ad_serving_id_length has no min; skipped.",
                );
            };
            crate::media::ad_serving_id_too_short(xml, min)
        }
        "min_description_length" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-description-length-missing-min",
                    "min_description_length has no min; skipped.",
                );
            };
            crate::media::description_too_short(xml, min)
        }
        "min_ad_title_length" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-ad-title-length-missing-min",
                    "min_ad_title_length has no min; skipped.",
                );
            };
            crate::media::ad_title_too_short(xml, min)
        }
        "min_ad_parameters_length" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-ad-parameters-length-missing-min",
                    "min_ad_parameters_length has no min; skipped.",
                );
            };
            crate::media::ad_parameters_too_short(xml, min)
        }
        "max_expires_seconds" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-expires-missing-max",
                    "max_expires_seconds has no max; skipped.",
                );
            };
            crate::media::expires_exceeds(xml, max)
        }
        "min_expires_seconds" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-expires-missing-min",
                    "min_expires_seconds has no min; skipped.",
                );
            };
            crate::media::expires_below(xml, min)
        }
        "media_type" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.media-type-missing-values",
                    "media_type has no MIME names; skipped.",
                );
            }
            type_container_misses(
                xml,
                facts,
                sibling,
                &allowed,
                &sibling.containers,
                &sibling.deliveries,
            )
        }
        "min_width" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-width-missing-min",
                    "min_width has no min; skipped.",
                );
            };
            size_misses(
                xml,
                facts,
                sibling,
                Some(min),
                sibling.min_height,
                sibling.max_width,
                sibling.max_height,
                sibling.aspect.as_deref(),
            )
        }
        "min_height" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-height-missing-min",
                    "min_height has no min; skipped.",
                );
            };
            size_misses(
                xml,
                facts,
                sibling,
                sibling.min_width,
                Some(min),
                sibling.max_width,
                sibling.max_height,
                sibling.aspect.as_deref(),
            )
        }
        "max_width" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-width-missing-max",
                    "max_width has no max; skipped.",
                );
            };
            size_misses(
                xml,
                facts,
                sibling,
                sibling.min_width,
                sibling.min_height,
                Some(max),
                sibling.max_height,
                sibling.aspect.as_deref(),
            )
        }
        "max_height" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-height-missing-max",
                    "max_height has no max; skipped.",
                );
            };
            size_misses(
                xml,
                facts,
                sibling,
                sibling.min_width,
                sibling.min_height,
                sibling.max_width,
                Some(max),
                sibling.aspect.as_deref(),
            )
        }
        "max_bitrate" => {
            let Some(max) = req.max else {
                return skip_missing_names(
                    findings,
                    "policy.max-bitrate-missing-max",
                    "max_bitrate has no max; skipped.",
                );
            };
            bitrate_misses_range(xml, facts, sibling, sibling.bitrate_floor, Some(max))
        }
        "min_bitrate" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-bitrate-missing-min",
                    "min_bitrate has no min; skipped.",
                );
            };
            bitrate_misses_range(xml, facts, sibling, Some(min), sibling.bitrate_cap)
        }
        "aspect_ratio" => {
            let want = nonempty_str(req.value.as_deref())
                .map(str::to_string)
                .or_else(|| trimmed_values(req).into_iter().next());
            let Some(want) = want else {
                return skip_missing_names(
                    findings,
                    "policy.aspect-ratio-missing-value",
                    "aspect_ratio has no ratio; skipped.",
                );
            };
            size_misses(
                xml,
                facts,
                sibling,
                sibling.min_width,
                sibling.min_height,
                sibling.max_width,
                sibling.max_height,
                Some(&want),
            )
        }
        "require_tracking_events" => {
            let needed: Vec<String> = req
                .values
                .clone()
                .unwrap_or_else(|| {
                    req.value
                        .as_deref()
                        .unwrap_or("")
                        .split(',')
                        .map(|s| s.to_string())
                        .collect()
                })
                .into_iter()
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            if needed.is_empty() {
                findings.push(Finding {
                    requirement_id: "policy.require-tracking-events-missing-names".into(),
                    severity: "warning".into(),
                    message: "require_tracking_events has no event names; skipped.".into(),
                    remediable: Some(true),
                    dimension: Some("policy".into()),
                    ..Default::default()
                });
                return false;
            }
            needed
                .iter()
                .any(|n| !facts.tracking_events.iter().any(|e| e == n))
        }
        "require_macros" => {
            let wanted = trimmed_values(req);
            if wanted.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.require-macros-missing-values",
                    "require_macros has no macro names; skipped.",
                );
            }
            wanted.iter().any(|m| !xml_has_macro(xml, m))
        }
        "codec" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.codec-missing-values",
                    "codec has no codec names; skipped.",
                );
            }
            codec_pair_misses(xml, facts, sibling, &allowed, &sibling.audio_codecs)
        }
        "audio_codec" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.audio-codec-missing-values",
                    "audio_codec has no codec names; skipped.",
                );
            }
            codec_pair_misses(xml, facts, sibling, &sibling.codecs, &allowed)
        }
        "container" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.container-missing-values",
                    "container has no container names; skipped.",
                );
            }
            type_container_misses(
                xml,
                facts,
                sibling,
                &sibling.mimes,
                &allowed,
                &sibling.deliveries,
            )
        }
        "delivery" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.delivery-missing-values",
                    "delivery has no delivery names; skipped.",
                );
            }
            type_container_misses(
                xml,
                facts,
                sibling,
                &sibling.mimes,
                &sibling.containers,
                &allowed,
            )
        }
        "category_forbidden" => {
            let blocked = trimmed_values(req);
            if blocked.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.category-forbidden-missing-values",
                    "category_forbidden has no category names; skipped.",
                );
            }
            facts.categories.iter().any(|c| {
                blocked.iter().any(|b| {
                    c.eq_ignore_ascii_case(b)
                        || c.to_ascii_uppercase().contains(&b.to_ascii_uppercase())
                })
            })
        }
        "device_family" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.device-family-missing-values",
                    "device_family has no device names; skipped.",
                );
            }
            device_family
                .and_then(|d| nonempty_str(Some(d)))
                .map(|d| !allowed.iter().any(|a| a.eq_ignore_ascii_case(d)))
                .unwrap_or(true)
        }
        "caption_language" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.caption-language-missing-values",
                    "caption_language has no language names; skipped.",
                );
            }
            crate::media::caption_language_misses(xml, &allowed)
        }
        "caption_type" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.caption-type-missing-values",
                    "caption_type has no MIME names; skipped.",
                );
            }
            crate::media::caption_type_misses(xml, &allowed)
        }
        "caption_file_status" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.caption-file-status-missing-values",
                    "caption_file_status has no fileStatus names; skipped.",
                );
            }
            crate::media::caption_file_status_misses(xml, &allowed)
        }
        "id_registry" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.id-registry-missing-values",
                    "id_registry has no registry names; skipped.",
                );
            }
            crate::media::id_registry_misses(xml, &allowed)
        }
        "icon_program" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-program-missing-values",
                    "icon_program has no program names; skipped.",
                );
            }
            crate::media::icon_program_misses(xml, &allowed)
        }
        "ad_system_version" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.ad-system-version-missing-values",
                    "ad_system_version has no version names; skipped.",
                );
            }
            crate::media::ad_system_version_misses(xml, &allowed)
        }
        "vast_version" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.vast-version-missing-values",
                    "vast_version has no version names; skipped.",
                );
            }
            crate::media::vast_version_misses(xml, &allowed)
        }
        "creative_sequence" => {
            let required = trimmed_values(req);
            if required.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.creative-sequence-missing-values",
                    "creative_sequence has no sequence names; skipped.",
                );
            }
            crate::media::creative_sequence_misses(xml, &required)
        }
        "ad_sequence" => {
            let required = trimmed_values(req);
            if required.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.ad-sequence-missing-values",
                    "ad_sequence has no sequence names; skipped.",
                );
            }
            crate::media::ad_sequence_misses(xml, &required)
        }
        "verification_vendor" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.verification-vendor-missing-values",
                    "verification_vendor has no vendor names; skipped.",
                );
            }
            crate::media::verification_vendor_misses(xml, &allowed)
        }
        "pricing_model" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.pricing-model-missing-values",
                    "pricing_model has no model names; skipped.",
                );
            }
            crate::media::pricing_model_misses(xml, &allowed)
        }
        "pricing_currency" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.pricing-currency-missing-values",
                    "pricing_currency has no currency names; skipped.",
                );
            }
            crate::media::pricing_currency_misses(xml, &allowed)
        }
        "wrapper_https" => crate::media::wrapper_https_misses(xml),
        "error_https" => crate::media::error_https_misses(xml),
        "viewable_https" => crate::media::viewable_https_misses(xml),
        "impression_https" => crate::media::impression_https_misses(xml),
        "clickthrough_https" => crate::media::clickthrough_https_misses(xml),
        "survey_https" => crate::media::survey_https_misses(xml),
        "clicktracking_https" => crate::media::clicktracking_https_misses(xml),
        "icon_clickthrough_https" => crate::media::icon_clickthrough_https_misses(xml),
        "category_authority" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.category-authority-missing-values",
                    "category_authority has no authority names; skipped.",
                );
            }
            crate::media::category_authority_misses(xml, &allowed)
        }
        "javascript_resource_https" => crate::media::javascript_resource_https_misses(xml),
        "companion_clickthrough_https" => crate::media::companion_clickthrough_https_misses(xml),
        "advertiser" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.advertiser-missing-values",
                    "advertiser has no advertiser names; skipped.",
                );
            }
            crate::media::advertiser_misses(xml, &allowed)
        }
        "advertiser_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.advertiser-id-missing-values",
                    "advertiser_id has no id names; skipped.",
                );
            }
            crate::media::advertiser_id_misses(xml, &allowed)
        }
        "executable_resource_https" => crate::media::executable_resource_https_misses(xml),
        "nonlinear_clickthrough_https" => crate::media::nonlinear_clickthrough_https_misses(xml),
        "static_resource_https" => crate::media::static_resource_https_misses(xml),
        "iframe_resource_https" => crate::media::iframe_resource_https_misses(xml),
        "html_resource_https" => crate::media::html_resource_https_misses(xml),
        "flash_resource_https" => crate::media::flash_resource_https_misses(xml),
        "closed_caption_https" => crate::media::closed_caption_https_misses(xml),
        "icon_viewtracking_https" => crate::media::icon_viewtracking_https_misses(xml),
        "ad_system" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.ad-system-missing-values",
                    "ad_system has no ad-system names; skipped.",
                );
            }
            crate::media::ad_system_misses(xml, &allowed)
        }
        "icon_clickfallback_https" => crate::media::icon_clickfallback_https_misses(xml),
        "verification_parameters_https" => crate::media::verification_parameters_https_misses(xml),
        "mezzanine_https" => crate::media::mezzanine_https_misses(xml),
        "category" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.category-missing-values",
                    "category has no category names; skipped.",
                );
            }
            crate::media::category_misses(xml, &allowed)
        }
        "tracking_https" => crate::media::tracking_https_misses(xml),
        "pricing" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.pricing-missing-values",
                    "pricing has no pricing names; skipped.",
                );
            }
            crate::media::pricing_misses(xml, &allowed)
        }
        "extension_https" => crate::media::extension_https_misses(xml),
        "interactive_creative_file_https" => {
            crate::media::interactive_creative_file_https_misses(xml)
        }
        "universal_ad_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.universal-ad-id-missing-values",
                    "universal_ad_id has no ad-id names; skipped.",
                );
            }
            crate::media::universal_ad_id_misses(xml, &allowed)
        }
        "api_framework" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.api-framework-missing-values",
                    "api_framework has no framework names; skipped.",
                );
            }
            crate::media::api_framework_misses(xml, &allowed)
        }
        "creative_adid" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.creative-adid-missing-values",
                    "creative_adid has no adid names; skipped.",
                );
            }
            crate::media::creative_adid_misses(xml, &allowed)
        }
        "interactive_creative_file_type" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.interactive-creative-file-type-missing-values",
                    "interactive_creative_file_type has no type names; skipped.",
                );
            }
            crate::media::interactive_creative_file_type_misses(xml, &allowed)
        }
        "viewable_impression_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.viewable-impression-id-missing-values",
                    "viewable_impression_id has no id names; skipped.",
                );
            }
            crate::media::viewable_impression_id_misses(xml, &allowed)
        }
        "companion_type" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-type-missing-values",
                    "companion_type has no type names; skipped.",
                );
            }
            crate::media::companion_type_misses(xml, &allowed)
        }
        "icon_type" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-type-missing-values",
                    "icon_type has no type names; skipped.",
                );
            }
            crate::media::icon_type_misses(xml, &allowed)
        }
        "ad_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.ad-id-missing-values",
                    "ad_id has no ad-id names; skipped.",
                );
            }
            crate::media::ad_id_misses(xml, &allowed)
        }
        "mediafile_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mediafile-id-missing-values",
                    "mediafile_id has no id names; skipped.",
                );
            }
            crate::media::mediafile_id_misses(xml, &allowed)
        }
        "companion_adslot_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-adslot-id-missing-values",
                    "companion_adslot_id has no slot names; skipped.",
                );
            }
            crate::media::companion_adslot_id_misses(xml, &allowed)
        }
        "icon_alt_text" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-alt-text-missing-values",
                    "icon_alt_text has no alt-text names; skipped.",
                );
            }
            crate::media::icon_alt_text_misses(xml, &allowed)
        }
        "companion_alt_text" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-alt-text-missing-values",
                    "companion_alt_text has no AltText names; skipped.",
                );
            }
            crate::media::companion_alt_text_misses(xml, &allowed)
        }
        "companion_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-id-missing-values",
                    "companion_id has no id names; skipped.",
                );
            }
            crate::media::companion_id_misses(xml, &allowed)
        }
        "icon_hover_text" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-hover-text-missing-values",
                    "icon_hover_text has no hover-text names; skipped.",
                );
            }
            crate::media::icon_hover_text_misses(xml, &allowed)
        }
        "nonlinear_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.nonlinear-id-missing-values",
                    "nonlinear_id has no id names; skipped.",
                );
            }
            crate::media::nonlinear_id_misses(xml, &allowed)
        }
        "companion_rendering_mode" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-rendering-mode-missing-values",
                    "companion_rendering_mode has no mode names; skipped.",
                );
            }
            crate::media::companion_rendering_mode_misses(xml, &allowed)
        }
        "icon_pxratio" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-pxratio-missing-values",
                    "icon_pxratio has no pxratio names; skipped.",
                );
            }
            crate::media::icon_pxratio_misses(xml, &allowed)
        }
        "nonlinear_scalable" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.nonlinear-scalable-missing-values",
                    "nonlinear_scalable has no scalable names; skipped.",
                );
            }
            crate::media::nonlinear_scalable_misses(xml, &allowed)
        }
        "nonlinear_maintain_aspect_ratio" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.nonlinear-maintain-aspect-ratio-missing-values",
                    "nonlinear_maintain_aspect_ratio has no aspect-ratio names; skipped.",
                );
            }
            crate::media::nonlinear_maintain_aspect_ratio_misses(xml, &allowed)
        }
        "companion_xml_encoded" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-xml-encoded-missing-values",
                    "companion_xml_encoded has no xmlEncoded names; skipped.",
                );
            }
            crate::media::companion_xml_encoded_misses(xml, &allowed)
        }
        "icon_xml_encoded" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-xml-encoded-missing-values",
                    "icon_xml_encoded has no xmlEncoded names; skipped.",
                );
            }
            crate::media::icon_xml_encoded_misses(xml, &allowed)
        }
        "nonlinear_xml_encoded" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.nonlinear-xml-encoded-missing-values",
                    "nonlinear_xml_encoded has no xmlEncoded names; skipped.",
                );
            }
            crate::media::nonlinear_xml_encoded_misses(xml, &allowed)
        }
        "companion_pxratio" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-pxratio-missing-values",
                    "companion_pxratio has no pxratio names; skipped.",
                );
            }
            crate::media::companion_pxratio_misses(xml, &allowed)
        }
        "wrapper_follow_additional_wrappers" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.wrapper-follow-additional-wrappers-missing-values",
                    "wrapper_follow_additional_wrappers has no followAdditionalWrappers names; skipped.",
                );
            }
            crate::media::wrapper_follow_additional_wrappers_misses(xml, &allowed)
        }
        "wrapper_allow_multiple_ads" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.wrapper-allow-multiple-ads-missing-values",
                    "wrapper_allow_multiple_ads has no allowMultipleAds names; skipped.",
                );
            }
            crate::media::wrapper_allow_multiple_ads_misses(xml, &allowed)
        }
        "wrapper_fallback_on_no_ad" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.wrapper-fallback-on-no-ad-missing-values",
                    "wrapper_fallback_on_no_ad has no fallbackOnNoAd names; skipped.",
                );
            }
            crate::media::wrapper_fallback_on_no_ad_misses(xml, &allowed)
        }
        "ad_conditional_ad" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.ad-conditional-ad-missing-values",
                    "ad_conditional_ad has no conditionalAd names; skipped.",
                );
            }
            crate::media::ad_conditional_ad_misses(xml, &allowed)
        }
        "mediafile_scalable" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mediafile-scalable-missing-values",
                    "mediafile_scalable has no scalable names; skipped.",
                );
            }
            crate::media::mediafile_scalable_misses(xml, &allowed)
        }
        "mediafile_maintain_aspect_ratio" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mediafile-maintain-aspect-ratio-missing-values",
                    "mediafile_maintain_aspect_ratio has no aspect-ratio names; skipped.",
                );
            }
            crate::media::mediafile_maintain_aspect_ratio_misses(xml, &allowed)
        }
        "executable_resource_type" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.executable-resource-type-missing-values",
                    "executable_resource_type has no type names; skipped.",
                );
            }
            crate::media::executable_resource_type_misses(xml, &allowed)
        }
        "javascript_resource_browser_optional" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.javascript-resource-browser-optional-missing-values",
                    "javascript_resource_browser_optional has no browserOptional names; skipped.",
                );
            }
            crate::media::javascript_resource_browser_optional_misses(xml, &allowed)
        }
        "extension_type" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.extension-type-missing-values",
                    "extension_type has no type names; skipped.",
                );
            }
            crate::media::extension_type_misses(xml, &allowed)
        }
        "mezzanine_type" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mezzanine-type-missing-values",
                    "mezzanine_type has no type names; skipped.",
                );
            }
            crate::media::mezzanine_type_misses(xml, &allowed)
        }
        "mezzanine_delivery" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mezzanine-delivery-missing-values",
                    "mezzanine_delivery has no delivery names; skipped.",
                );
            }
            crate::media::mezzanine_delivery_misses(xml, &allowed)
        }
        "ad_parameters_xml_encoded" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.ad-parameters-xml-encoded-missing-values",
                    "ad_parameters_xml_encoded has no xmlEncoded names; skipped.",
                );
            }
            crate::media::ad_parameters_xml_encoded_misses(xml, &allowed)
        }
        "mezzanine_codec" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mezzanine-codec-missing-values",
                    "mezzanine_codec has no codec names; skipped.",
                );
            }
            crate::media::mezzanine_codec_misses(xml, &allowed)
        }
        "mezzanine_media_type" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mezzanine-media-type-missing-values",
                    "mezzanine_media_type has no mediaType names; skipped.",
                );
            }
            crate::media::mezzanine_media_type_misses(xml, &allowed)
        }
        "mezzanine_scalable" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mezzanine-scalable-missing-values",
                    "mezzanine_scalable has no scalable names; skipped.",
                );
            }
            crate::media::mezzanine_scalable_misses(xml, &allowed)
        }
        "mezzanine_maintain_aspect_ratio" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mezzanine-maintain-aspect-ratio-missing-values",
                    "mezzanine_maintain_aspect_ratio has no maintainAspectRatio names; skipped.",
                );
            }
            crate::media::mezzanine_maintain_aspect_ratio_misses(xml, &allowed)
        }
        "mezzanine_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mezzanine-id-missing-values",
                    "mezzanine_id has no id names; skipped.",
                );
            }
            crate::media::mezzanine_id_misses(xml, &allowed)
        }
        "icon_x_position" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-x-position-missing-values",
                    "icon_x_position has no position names; skipped.",
                );
            }
            crate::media::icon_x_position_misses(xml, &allowed)
        }
        "icon_y_position" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-y-position-missing-values",
                    "icon_y_position has no position names; skipped.",
                );
            }
            crate::media::icon_y_position_misses(xml, &allowed)
        }
        "ad_type" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.ad-type-missing-values",
                    "ad_type has no adType names; skipped.",
                );
            }
            crate::media::ad_type_misses(xml, &allowed)
        }
        "impression_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.impression-id-missing-values",
                    "impression_id has no id names; skipped.",
                );
            }
            crate::media::impression_id_misses(xml, &allowed)
        }
        "companion_ads_required" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-ads-required-missing-values",
                    "companion_ads_required has no required names; skipped.",
                );
            }
            crate::media::companion_ads_required_misses(xml, &allowed)
        }
        "min_mediafile_file_size" => {
            let Some(min) = req.min else {
                return skip_missing_names(
                    findings,
                    "policy.min-mediafile-file-size-missing-min",
                    "min_mediafile_file_size has no min; skipped.",
                );
            };
            crate::media::mediafile_file_size_below(xml, min)
        }
        "mediafile_media_type" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mediafile-media-type-missing-values",
                    "mediafile_media_type has no mediaType names; skipped.",
                );
            }
            crate::media::mediafile_media_type_misses(xml, &allowed)
        }
        "interactive_creative_file_variable_duration" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.interactive-creative-file-variable-duration-missing-values",
                    "interactive_creative_file_variable_duration has no variableDuration names; skipped.",
                );
            }
            crate::media::interactive_creative_file_variable_duration_misses(xml, &allowed)
        }
        "creative_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.creative-id-missing-values",
                    "creative_id has no id names; skipped.",
                );
            }
            crate::media::creative_id_misses(xml, &allowed)
        }
        "clickthrough_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.clickthrough-id-missing-values",
                    "clickthrough_id has no id names; skipped.",
                );
            }
            crate::media::clickthrough_id_misses(xml, &allowed)
        }
        "tracking_offset" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.tracking-offset-missing-values",
                    "tracking_offset has no offset names; skipped.",
                );
            }
            crate::media::tracking_offset_misses(xml, &allowed)
        }
        "clicktracking_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.clicktracking-id-missing-values",
                    "clicktracking_id has no id names; skipped.",
                );
            }
            crate::media::clicktracking_id_misses(xml, &allowed)
        }
        "custom_click_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.custom-click-id-missing-values",
                    "custom_click_id has no id names; skipped.",
                );
            }
            crate::media::custom_click_id_misses(xml, &allowed)
        }
        "icon_clickthrough_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-clickthrough-id-missing-values",
                    "icon_clickthrough_id has no id names; skipped.",
                );
            }
            crate::media::icon_clickthrough_id_misses(xml, &allowed)
        }
        "companion_clickthrough_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-clickthrough-id-missing-values",
                    "companion_clickthrough_id has no id names; skipped.",
                );
            }
            crate::media::companion_clickthrough_id_misses(xml, &allowed)
        }
        "nonlinear_clickthrough_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.nonlinear-clickthrough-id-missing-values",
                    "nonlinear_clickthrough_id has no id names; skipped.",
                );
            }
            crate::media::nonlinear_clickthrough_id_misses(xml, &allowed)
        }
        "icon_clicktracking_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-clicktracking-id-missing-values",
                    "icon_clicktracking_id has no id names; skipped.",
                );
            }
            crate::media::icon_clicktracking_id_misses(xml, &allowed)
        }
        "companion_clicktracking_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-clicktracking-id-missing-values",
                    "companion_clicktracking_id has no id names; skipped.",
                );
            }
            crate::media::companion_clicktracking_id_misses(xml, &allowed)
        }
        "nonlinear_clicktracking_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.nonlinear-clicktracking-id-missing-values",
                    "nonlinear_clicktracking_id has no id names; skipped.",
                );
            }
            crate::media::nonlinear_clicktracking_id_misses(xml, &allowed)
        }
        "icon_viewtracking_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-viewtracking-id-missing-values",
                    "icon_viewtracking_id has no id names; skipped.",
                );
            }
            crate::media::icon_viewtracking_id_misses(xml, &allowed)
        }
        "tracking_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.tracking-id-missing-values",
                    "tracking_id has no id names; skipped.",
                );
            }
            crate::media::tracking_id_misses(xml, &allowed)
        }
        "error_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.error-id-missing-values",
                    "error_id has no id names; skipped.",
                );
            }
            crate::media::error_id_misses(xml, &allowed)
        }
        "viewable_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.viewable-id-missing-values",
                    "viewable_id has no id names; skipped.",
                );
            }
            crate::media::viewable_id_misses(xml, &allowed)
        }
        "notviewable_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.notviewable-id-missing-values",
                    "notviewable_id has no id names; skipped.",
                );
            }
            crate::media::notviewable_id_misses(xml, &allowed)
        }
        "viewundetermined_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.viewundetermined-id-missing-values",
                    "viewundetermined_id has no id names; skipped.",
                );
            }
            crate::media::viewundetermined_id_misses(xml, &allowed)
        }
        "survey_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.survey-id-missing-values",
                    "survey_id has no id names; skipped.",
                );
            }
            crate::media::survey_id_misses(xml, &allowed)
        }
        "skipoffset" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.skipoffset-missing-values",
                    "skipoffset has no skipoffset names; skipped.",
                );
            }
            crate::media::skipoffset_misses(xml, &allowed)
        }
        "duration" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.duration-missing-values",
                    "duration has no duration names; skipped.",
                );
            }
            crate::media::duration_misses(xml, &allowed)
        }
        "expires" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.expires-missing-values",
                    "expires has no expires names; skipped.",
                );
            }
            crate::media::expires_misses(xml, &allowed)
        }
        "icon_duration" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-duration-missing-values",
                    "icon_duration has no duration names; skipped.",
                );
            }
            crate::media::icon_duration_misses(xml, &allowed)
        }
        "icon_offset" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-offset-missing-values",
                    "icon_offset has no offset names; skipped.",
                );
            }
            crate::media::icon_offset_misses(xml, &allowed)
        }
        "min_suggested_duration" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.min-suggested-duration-missing-values",
                    "min_suggested_duration has no duration names; skipped.",
                );
            }
            crate::media::min_suggested_duration_misses(xml, &allowed)
        }
        "ad_serving_id" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.ad-serving-id-missing-values",
                    "ad_serving_id has no id names; skipped.",
                );
            }
            crate::media::ad_serving_id_misses(xml, &allowed)
        }
        "ad_title" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.ad-title-missing-values",
                    "ad_title has no title names; skipped.",
                );
            }
            crate::media::ad_title_misses(xml, &allowed)
        }
        "description" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.description-missing-values",
                    "description has no description names; skipped.",
                );
            }
            crate::media::description_misses(xml, &allowed)
        }
        "ad_parameters" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.ad-parameters-missing-values",
                    "ad_parameters has no parameter names; skipped.",
                );
            }
            crate::media::ad_parameters_misses(xml, &allowed)
        }
        "static_resource_creative_type" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.static-resource-creative-type-missing-values",
                    "static_resource_creative_type has no type names; skipped.",
                );
            }
            crate::media::static_resource_creative_type_misses(xml, &allowed)
        }
        "flash_resource_creative_type" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.flash-resource-creative-type-missing-values",
                    "flash_resource_creative_type has no type names; skipped.",
                );
            }
            crate::media::flash_resource_creative_type_misses(xml, &allowed)
        }
        "impression" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.impression-missing-values",
                    "impression has no URL names; skipped.",
                );
            }
            crate::media::impression_misses(xml, &allowed)
        }
        "error" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.error-missing-values",
                    "error has no URL names; skipped.",
                );
            }
            crate::media::error_misses(xml, &allowed)
        }
        "clickthrough" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.clickthrough-missing-values",
                    "clickthrough has no URL names; skipped.",
                );
            }
            crate::media::clickthrough_misses(xml, &allowed)
        }
        "survey" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.survey-missing-values",
                    "survey has no URL names; skipped.",
                );
            }
            crate::media::survey_misses(xml, &allowed)
        }
        "tracking" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.tracking-missing-values",
                    "tracking has no URL names; skipped.",
                );
            }
            crate::media::tracking_misses(xml, &allowed)
        }
        "viewable" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.viewable-missing-values",
                    "viewable has no URL names; skipped.",
                );
            }
            crate::media::viewable_misses(xml, &allowed)
        }
        "clicktracking" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.clicktracking-missing-values",
                    "clicktracking has no URL names; skipped.",
                );
            }
            crate::media::clicktracking_misses(xml, &allowed)
        }
        "mezzanine" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mezzanine-missing-values",
                    "mezzanine has no URL names; skipped.",
                );
            }
            crate::media::mezzanine_misses(xml, &allowed)
        }
        "mediafile" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mediafile-missing-values",
                    "mediafile has no URL names; skipped.",
                );
            }
            crate::media::mediafile_misses(xml, &allowed)
        }
        "vast_ad_tag_uri" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.vast-ad-tag-uri-missing-values",
                    "vast_ad_tag_uri has no URL names; skipped.",
                );
            }
            crate::media::vast_ad_tag_uri_misses(xml, &allowed)
        }
        "javascript_resource" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.javascript-resource-missing-values",
                    "javascript_resource has no URL names; skipped.",
                );
            }
            crate::media::javascript_resource_misses(xml, &allowed)
        }
        "closed_caption_file" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.closed-caption-file-missing-values",
                    "closed_caption_file has no URL names; skipped.",
                );
            }
            crate::media::closed_caption_file_misses(xml, &allowed)
        }
        "executable_resource" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.executable-resource-missing-values",
                    "executable_resource has no URL names; skipped.",
                );
            }
            crate::media::executable_resource_misses(xml, &allowed)
        }
        "icon_clickthrough" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-clickthrough-missing-values",
                    "icon_clickthrough has no URL names; skipped.",
                );
            }
            crate::media::icon_clickthrough_misses(xml, &allowed)
        }
        "companion_clickthrough" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-clickthrough-missing-values",
                    "companion_clickthrough has no URL names; skipped.",
                );
            }
            crate::media::companion_clickthrough_misses(xml, &allowed)
        }
        "nonlinear_clickthrough" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.nonlinear-clickthrough-missing-values",
                    "nonlinear_clickthrough has no URL names; skipped.",
                );
            }
            crate::media::nonlinear_clickthrough_misses(xml, &allowed)
        }
        "icon_viewtracking" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-viewtracking-missing-values",
                    "icon_viewtracking has no URL names; skipped.",
                );
            }
            crate::media::icon_viewtracking_misses(xml, &allowed)
        }
        "interactive_creative_file" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.interactive-creative-file-missing-values",
                    "interactive_creative_file has no URL names; skipped.",
                );
            }
            crate::media::interactive_creative_file_misses(xml, &allowed)
        }
        "static_resource" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.static-resource-missing-values",
                    "static_resource has no URL names; skipped.",
                );
            }
            crate::media::static_resource_misses(xml, &allowed)
        }
        "iframe_resource" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.iframe-resource-missing-values",
                    "iframe_resource has no URL names; skipped.",
                );
            }
            crate::media::iframe_resource_misses(xml, &allowed)
        }
        "html_resource" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.html-resource-missing-values",
                    "html_resource has no URL names; skipped.",
                );
            }
            crate::media::html_resource_misses(xml, &allowed)
        }
        "flash_resource" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.flash-resource-missing-values",
                    "flash_resource has no URL names; skipped.",
                );
            }
            crate::media::flash_resource_misses(xml, &allowed)
        }
        "icon_clickfallback" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-clickfallback-missing-values",
                    "icon_clickfallback has no URL names; skipped.",
                );
            }
            crate::media::icon_clickfallback_misses(xml, &allowed)
        }
        "custom_click" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.custom-click-missing-values",
                    "custom_click has no URL names; skipped.",
                );
            }
            crate::media::custom_click_misses(xml, &allowed)
        }
        "icon_clicktracking" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-clicktracking-missing-values",
                    "icon_clicktracking has no URL names; skipped.",
                );
            }
            crate::media::icon_clicktracking_misses(xml, &allowed)
        }
        "companion_clicktracking" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-clicktracking-missing-values",
                    "companion_clicktracking has no URL names; skipped.",
                );
            }
            crate::media::companion_clicktracking_misses(xml, &allowed)
        }
        "nonlinear_clicktracking" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.nonlinear-clicktracking-missing-values",
                    "nonlinear_clicktracking has no URL names; skipped.",
                );
            }
            crate::media::nonlinear_clicktracking_misses(xml, &allowed)
        }
        "notviewable" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.notviewable-missing-values",
                    "notviewable has no URL names; skipped.",
                );
            }
            crate::media::notviewable_misses(xml, &allowed)
        }
        "viewundetermined" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.viewundetermined-missing-values",
                    "viewundetermined has no URL names; skipped.",
                );
            }
            crate::media::viewundetermined_misses(xml, &allowed)
        }
        "extension" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.extension-missing-values",
                    "extension has no URL names; skipped.",
                );
            }
            crate::media::extension_misses(xml, &allowed)
        }
        "verification_parameters" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.verification-parameters-missing-values",
                    "verification_parameters has no URL names; skipped.",
                );
            }
            crate::media::verification_parameters_misses(xml, &allowed)
        }
        "blocked_ad_categories" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.blocked-ad-categories-missing-values",
                    "blocked_ad_categories has no category names; skipped.",
                );
            }
            crate::media::blocked_ad_categories_misses(xml, &allowed)
        }
        "creative_extension" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.creative-extension-missing-values",
                    "creative_extension has no URL names; skipped.",
                );
            }
            crate::media::creative_extension_misses(xml, &allowed)
        }
        "creative_extension_type" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.creative-extension-type-missing-values",
                    "creative_extension_type has no type names; skipped.",
                );
            }
            crate::media::creative_extension_type_misses(xml, &allowed)
        }
        "id_value" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.id-value-missing-values",
                    "id_value has no idValue names; skipped.",
                );
            }
            crate::media::id_value_misses(xml, &allowed)
        }
        "mediafile_codec" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mediafile-codec-missing-values",
                    "mediafile_codec has no codec names; skipped.",
                );
            }
            crate::media::mediafile_codec_misses(xml, &allowed)
        }
        "creative_extension_https" => crate::media::creative_extension_https_misses(xml),
        "notviewable_https" => crate::media::notviewable_https_misses(xml),
        "viewundetermined_https" => crate::media::viewundetermined_https_misses(xml),
        "custom_click_https" => crate::media::custom_click_https_misses(xml),
        "icon_clicktracking_https" => crate::media::icon_clicktracking_https_misses(xml),
        "companion_clicktracking_https" => crate::media::companion_clicktracking_https_misses(xml),
        "nonlinear_clicktracking_https" => crate::media::nonlinear_clicktracking_https_misses(xml),
        "blocked_ad_categories_authority" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.blocked-ad-categories-authority-missing-values",
                    "blocked_ad_categories_authority has no authority names; skipped.",
                );
            }
            crate::media::blocked_ad_categories_authority_misses(xml, &allowed)
        }
        "mediafile_minbitrate" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mediafile-minbitrate-missing-values",
                    "mediafile_minbitrate has no minBitrate names; skipped.",
                );
            }
            crate::media::mediafile_minbitrate_misses(xml, &allowed)
        }
        "mediafile_maxbitrate" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mediafile-maxbitrate-missing-values",
                    "mediafile_maxbitrate has no maxBitrate names; skipped.",
                );
            }
            crate::media::mediafile_maxbitrate_misses(xml, &allowed)
        }
        "mezzanine_minbitrate" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mezzanine-minbitrate-missing-values",
                    "mezzanine_minbitrate has no minBitrate names; skipped.",
                );
            }
            crate::media::mezzanine_minbitrate_misses(xml, &allowed)
        }
        "mezzanine_maxbitrate" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mezzanine-maxbitrate-missing-values",
                    "mezzanine_maxbitrate has no maxBitrate names; skipped.",
                );
            }
            crate::media::mezzanine_maxbitrate_misses(xml, &allowed)
        }
        "mediafile_bitrate" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mediafile-bitrate-missing-values",
                    "mediafile_bitrate has no bitrate names; skipped.",
                );
            }
            crate::media::mediafile_bitrate_misses(xml, &allowed)
        }
        "mezzanine_bitrate" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mezzanine-bitrate-missing-values",
                    "mezzanine_bitrate has no bitrate names; skipped.",
                );
            }
            crate::media::mezzanine_bitrate_misses(xml, &allowed)
        }
        "mediafile_filesize" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mediafile-filesize-missing-values",
                    "mediafile_filesize has no fileSize names; skipped.",
                );
            }
            crate::media::mediafile_filesize_misses(xml, &allowed)
        }
        "mezzanine_filesize" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mezzanine-filesize-missing-values",
                    "mezzanine_filesize has no fileSize names; skipped.",
                );
            }
            crate::media::mezzanine_filesize_misses(xml, &allowed)
        }
        "mediafile_width" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mediafile-width-missing-values",
                    "mediafile_width has no width names; skipped.",
                );
            }
            crate::media::mediafile_width_misses(xml, &allowed)
        }
        "mediafile_height" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mediafile-height-missing-values",
                    "mediafile_height has no height names; skipped.",
                );
            }
            crate::media::mediafile_height_misses(xml, &allowed)
        }
        "mezzanine_width" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mezzanine-width-missing-values",
                    "mezzanine_width has no width names; skipped.",
                );
            }
            crate::media::mezzanine_width_misses(xml, &allowed)
        }
        "mezzanine_height" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mezzanine-height-missing-values",
                    "mezzanine_height has no height names; skipped.",
                );
            }
            crate::media::mezzanine_height_misses(xml, &allowed)
        }
        "mediafile_delivery" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mediafile-delivery-missing-values",
                    "mediafile_delivery has no delivery names; skipped.",
                );
            }
            crate::media::mediafile_delivery_misses(xml, &allowed)
        }
        "icon_width" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-width-missing-values",
                    "icon_width has no width names; skipped.",
                );
            }
            crate::media::icon_width_misses(xml, &allowed)
        }
        "icon_height" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-height-missing-values",
                    "icon_height has no height names; skipped.",
                );
            }
            crate::media::icon_height_misses(xml, &allowed)
        }
        "companion_width" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-width-missing-values",
                    "companion_width has no width names; skipped.",
                );
            }
            crate::media::companion_width_misses(xml, &allowed)
        }
        "companion_height" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-height-missing-values",
                    "companion_height has no height names; skipped.",
                );
            }
            crate::media::companion_height_misses(xml, &allowed)
        }
        "nonlinear_width" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.nonlinear-width-missing-values",
                    "nonlinear_width has no width names; skipped.",
                );
            }
            crate::media::nonlinear_width_misses(xml, &allowed)
        }
        "nonlinear_height" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.nonlinear-height-missing-values",
                    "nonlinear_height has no height names; skipped.",
                );
            }
            crate::media::nonlinear_height_misses(xml, &allowed)
        }
        "companion_expanded_width" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-expanded-width-missing-values",
                    "companion_expanded_width has no expandedWidth names; skipped.",
                );
            }
            crate::media::companion_expanded_width_misses(xml, &allowed)
        }
        "companion_expanded_height" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-expanded-height-missing-values",
                    "companion_expanded_height has no expandedHeight names; skipped.",
                );
            }
            crate::media::companion_expanded_height_misses(xml, &allowed)
        }
        "nonlinear_expanded_width" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.nonlinear-expanded-width-missing-values",
                    "nonlinear_expanded_width has no expandedWidth names; skipped.",
                );
            }
            crate::media::nonlinear_expanded_width_misses(xml, &allowed)
        }
        "nonlinear_expanded_height" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.nonlinear-expanded-height-missing-values",
                    "nonlinear_expanded_height has no expandedHeight names; skipped.",
                );
            }
            crate::media::nonlinear_expanded_height_misses(xml, &allowed)
        }
        "companion_asset_width" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-asset-width-missing-values",
                    "companion_asset_width has no assetWidth names; skipped.",
                );
            }
            crate::media::companion_asset_width_misses(xml, &allowed)
        }
        "companion_asset_height" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.companion-asset-height-missing-values",
                    "companion_asset_height has no assetHeight names; skipped.",
                );
            }
            crate::media::companion_asset_height_misses(xml, &allowed)
        }
        "icon_click_fallback_width" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-click-fallback-width-missing-values",
                    "icon_click_fallback_width has no width names; skipped.",
                );
            }
            crate::media::icon_click_fallback_width_misses(xml, &allowed)
        }
        "icon_click_fallback_height" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.icon-click-fallback-height-missing-values",
                    "icon_click_fallback_height has no height names; skipped.",
                );
            }
            crate::media::icon_click_fallback_height_misses(xml, &allowed)
        }
        "mediafile_type" => {
            let allowed = trimmed_values(req);
            if allowed.is_empty() {
                return skip_missing_names(
                    findings,
                    "policy.mediafile-type-missing-values",
                    "mediafile_type has no type names; skipped.",
                );
            }
            crate::media::mediafile_type_misses(xml, &allowed)
        }
        other => {
            findings.push(Finding {
                requirement_id: format!("policy.unknown-kind.{other}"),
                severity: "warning".into(),
                message: format!("pack requirement kind {other:?} is unknown; skipped."),
                remediable: Some(true),
                dimension: Some("policy".into()),
                ..Default::default()
            });
            false
        }
    }
}

pub fn apply_overlay(overlay: &PolicyOverlay, xml: &str, findings: &mut Vec<Finding>) {
    apply_overlay_for(overlay, xml, None, findings);
}

pub fn apply_overlay_for(
    overlay: &PolicyOverlay,
    xml: &str,
    device_family: Option<&str>,
    findings: &mut Vec<Finding>,
) {
    let start = findings.len();
    let doc = PolicyDocument {
        id: overlay.id.clone(),
        version: "overlay".into(),
        requirements: overlay.requirements.clone(),
        ..Default::default()
    };
    apply_policy_for(&doc, xml, nonempty_str(device_family), findings);
    for f in &mut findings[start..] {
        if f.deal.is_none() {
            f.deal = crate::eligibility::nonempty_owned(overlay.deal_id.clone());
        }
        if f.publisher.is_none() {
            f.publisher = crate::eligibility::nonempty_owned(overlay.publisher_id.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_live_ssai() {
        let p = discover_policy("live-ssai").unwrap();
        assert_eq!(p.id, "live-ssai");
        assert!(p
            .requirements
            .iter()
            .any(|r| r.id.ends_with("mezzanine-required")));
        assert!(p
            .requirements
            .iter()
            .any(|r| r.id.ends_with("universaladid-required")));
        assert_eq!(
            p.transcode_ladder.as_ref().map(|l| l.renditions.len()),
            Some(1)
        );
        assert!(p.proactive_required);
        assert_eq!(p.invalidation_rules.len(), 4);
        crate::schema_catalog::assert_schema(
            "policy-document.json",
            &serde_json::from_str(LIVE_SSAI).unwrap(),
        );
    }

    #[test]
    fn discover_live_sgai_has_no_mezzanine_rule() {
        let p = discover_policy("live-sgai").unwrap();
        assert_eq!(p.architecture.as_deref(), Some("sgai"));
        assert!(!p.requirements.iter().any(|r| r.id.contains("mezzanine")));
        assert_eq!(p.invalidation_rules.len(), 4);
    }

    #[test]
    fn discover_live_csai() {
        let p = discover_policy("live-csai").unwrap();
        assert_eq!(p.architecture.as_deref(), Some("csai"));
        assert!(!p.proactive_required);
        assert!(p.transcode_ladder.is_none());
        assert_eq!(p.invalidation_rules.len(), 4);
    }

    #[test]
    fn discover_live_hybrid() {
        let p = discover_policy("live-hybrid").unwrap();
        assert_eq!(p.architecture.as_deref(), Some("hybrid"));
        assert!(p.proactive_required);
        assert!(p.requirements.iter().any(|r| r.id.contains("mezzanine")));
        assert!(p
            .requirements
            .iter()
            .any(|r| r.id.contains("universaladid")));
        assert_eq!(p.invalidation_rules.len(), 4);
        crate::schema_catalog::assert_schema(
            "policy-document.json",
            &serde_json::from_str(LIVE_HYBRID).unwrap(),
        );
    }

    #[test]
    fn codecs_overlay_requires_declared_audio() {
        let overlay =
            parse_overlay(include_str!("../../../fixtures/overlays/audio-aac.json")).unwrap();
        let mut miss = Vec::new();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/wrapper.xml"),
            &mut miss,
        );
        assert!(miss
            .iter()
            .any(|f| f.requirement_id == "live-audio.aac-required"));
        let mut hit = Vec::new();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/ok-codecs.xml"),
            &mut hit,
        );
        assert!(!hit
            .iter()
            .any(|f| f.requirement_id == "live-audio.aac-required"));
    }

    #[test]
    fn leftover_empty_codec_values_are_dropped() {
        let overlay = PolicyOverlay {
            id: "codec".into(),
            requirements: vec![PolicyRequirement {
                id: "codec.avc".into(),
                kind: "codec".into(),
                message: "need avc".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["avc".into(), "  ".into(), "".into()]),
            }],
            ..Default::default()
        };
        let mut ok = Vec::new();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/ok-codecs.xml"),
            &mut ok,
        );
        assert!(!ok.iter().any(|f| f.requirement_id == "codec.avc"));
        let blank = PolicyOverlay {
            id: "blank".into(),
            requirements: vec![PolicyRequirement {
                id: "blank.codec".into(),
                kind: "codec".into(),
                message: "need codec".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["  ".into()]),
            }],
            ..Default::default()
        };
        let mut warn = Vec::new();
        apply_overlay(
            &blank,
            include_str!("../../../fixtures/ok-codecs.xml"),
            &mut warn,
        );
        assert!(warn
            .iter()
            .any(|f| f.requirement_id == "policy.codec-missing-values" && f.severity == "warning"));
        assert!(!warn.iter().any(|f| f.requirement_id == "blank.codec"));
    }

    #[test]
    fn leftover_empty_media_type_values_are_dropped() {
        let overlay = PolicyOverlay {
            id: "mime".into(),
            requirements: vec![PolicyRequirement {
                id: "mime.mp4".into(),
                kind: "media_type".into(),
                message: "need mp4".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["video/mp4".into(), "  ".into(), "".into()]),
            }],
            ..Default::default()
        };
        let mut ok = Vec::new();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut ok,
        );
        assert!(!ok.iter().any(|f| f.requirement_id == "mime.mp4"));
        let blank = PolicyOverlay {
            id: "blank".into(),
            requirements: vec![PolicyRequirement {
                id: "blank.mime".into(),
                kind: "media_type".into(),
                message: "need mime".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["  ".into(), "".into()]),
            }],
            ..Default::default()
        };
        let mut warn = Vec::new();
        apply_overlay(
            &blank,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut warn,
        );
        assert!(warn.iter().any(|f| {
            f.requirement_id == "policy.media-type-missing-values" && f.severity == "warning"
        }));
        assert!(!warn.iter().any(|f| f.requirement_id == "blank.mime"));
    }

    #[test]
    fn leftover_empty_delivery_values_are_dropped() {
        let overlay = PolicyOverlay {
            id: "del".into(),
            requirements: vec![PolicyRequirement {
                id: "del.prog".into(),
                kind: "delivery".into(),
                message: "need progressive".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["progressive".into(), "  ".into(), "".into()]),
            }],
            ..Default::default()
        };
        let mut ok = Vec::new();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut ok,
        );
        assert!(!ok.iter().any(|f| f.requirement_id == "del.prog"));
        let blank = PolicyOverlay {
            id: "blank".into(),
            requirements: vec![PolicyRequirement {
                id: "blank.del".into(),
                kind: "delivery".into(),
                message: "need delivery".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["  ".into(), "".into()]),
            }],
            ..Default::default()
        };
        let mut warn = Vec::new();
        apply_overlay(
            &blank,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut warn,
        );
        assert!(warn.iter().any(|f| {
            f.requirement_id == "policy.delivery-missing-values" && f.severity == "warning"
        }));
        assert!(!warn.iter().any(|f| f.requirement_id == "blank.del"));
    }

    #[test]
    fn leftover_empty_device_family_values_are_dropped() {
        let overlay = PolicyOverlay {
            id: "device".into(),
            requirements: vec![PolicyRequirement {
                id: "device.family".into(),
                kind: "device_family".into(),
                message: "need ctv".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["ctv".into(), "  ".into(), "".into()]),
            }],
            ..Default::default()
        };
        let mut ok = Vec::new();
        apply_overlay_for(
            &overlay,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            Some("ctv"),
            &mut ok,
        );
        assert!(!ok.iter().any(|f| f.requirement_id == "device.family"));
        let blank = PolicyOverlay {
            id: "blank".into(),
            requirements: vec![PolicyRequirement {
                id: "blank.device".into(),
                kind: "device_family".into(),
                message: "need device".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["  ".into()]),
            }],
            ..Default::default()
        };
        let mut warn = Vec::new();
        apply_overlay_for(
            &blank,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            Some("ctv"),
            &mut warn,
        );
        assert!(warn.iter().any(|f| {
            f.requirement_id == "policy.device-family-missing-values" && f.severity == "warning"
        }));
        assert!(!warn.iter().any(|f| f.requirement_id == "blank.device"));
    }

    #[test]
    fn leftover_empty_caption_language_values_are_dropped() {
        let overlay = PolicyOverlay {
            id: "cap".into(),
            requirements: vec![PolicyRequirement {
                id: "cap.lang".into(),
                kind: "caption_language".into(),
                message: "need english captions".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["en".into(), "  ".into(), "".into()]),
            }],
            ..Default::default()
        };
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</MediaFiles>",
            "<ClosedCaptionFiles><ClosedCaptionFile type=\"text/vtt\" language=\"en\"><![CDATA[https://cdn.example.com/ad.vtt]]></ClosedCaptionFile></ClosedCaptionFiles></MediaFiles>",
        );
        let mut ok = Vec::new();
        apply_overlay(&overlay, &xml, &mut ok);
        assert!(!ok.iter().any(|f| f.requirement_id == "cap.lang"));
        let blank = PolicyOverlay {
            id: "blank".into(),
            requirements: vec![PolicyRequirement {
                id: "blank.cap".into(),
                kind: "caption_language".into(),
                message: "need language".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["  ".into(), "".into()]),
            }],
            ..Default::default()
        };
        let mut warn = Vec::new();
        apply_overlay(&blank, &xml, &mut warn);
        assert!(warn.iter().any(|f| {
            f.requirement_id == "policy.caption-language-missing-values" && f.severity == "warning"
        }));
        assert!(!warn.iter().any(|f| f.requirement_id == "blank.cap"));
    }

    #[test]
    fn leftover_empty_caption_type_and_id_registry_values_are_dropped() {
        let cap_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</MediaFiles>",
            "<ClosedCaptionFiles><ClosedCaptionFile type=\"text/vtt\" language=\"en\" fileStatus=\"onDemand\"><![CDATA[https://cdn.example.com/ad.vtt]]></ClosedCaptionFile></ClosedCaptionFiles></MediaFiles>",
        );
        let icon_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" width=\"20\" height=\"20\"></Icon></Icons></Linear>",
        );
        let sys_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<AdSystem>Test</AdSystem>",
            "<AdSystem version=\"1.0\">Test</AdSystem>",
        );
        let pos_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" xPosition=\"right\" yPosition=\"top\"></Icon></Icons></Linear>",
        );
        let seq_xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("<Creative>", "<Creative sequence=\"1\">");
        let ad_xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("<Ad id=\"1\">", "<Ad id=\"1\" sequence=\"1\">");
        let vendor_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</InLine>",
            "<AdVerifications><Verification vendor=\"iabtechlab.com-omid\"></Verification></AdVerifications></InLine>",
        );
        let pricing_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</AdServingId>",
            "</AdServingId><Pricing model=\"CPM\" currency=\"USD\"><![CDATA[2.50]]></Pricing>",
        );
        let cat_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</AdTitle>",
            "</AdTitle><Category authority=\"IAB\">IAB1-1</Category>",
        );
        let adv_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</AdTitle>",
            "</AdTitle><Advertiser id=\"adv-1\">Acme</Advertiser>",
        );
        let adid_xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("<Creative>", "<Creative AdID=\"ad-1\">");
        let icf_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><InteractiveCreativeFile type=\"application/javascript\"></InteractiveCreativeFile>",
        );
        let view_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Impression>",
            "</Impression><ViewableImpression id=\"view-1\"><Viewable><![CDATA[https://t.example.com/view]]></Viewable></ViewableImpression>",
        );
        let comp_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\" type=\"image/png\"><StaticResource creativeType=\"image/png\"><![CDATA[https://cdn.example.com/i.png]]></StaticResource></Companion></CompanionAds></Creative>",
        );
        let icon_type_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" type=\"image/png\" width=\"20\" height=\"20\"></Icon></Icons></Linear>",
        );
        let mf_id_xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("<MediaFile ", "<MediaFile id=\"mf-1\" ");
        let slot_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\" adSlotId=\"slot-1\"></Companion></CompanionAds></Creative>",
        );
        let alt_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" altText=\"AdChoices\" width=\"20\" height=\"20\"></Icon></Icons></Linear>",
        );
        let alt_elem_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><AltText>Learn more</AltText></Companion></CompanionAds></Creative>",
        );
        let cid_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion id=\"comp-1\" width=\"300\" height=\"250\"></Companion></CompanionAds></Creative>",
        );
        let hover_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" hoverText=\"Learn more\" width=\"20\" height=\"20\"></Icon></Icons></Linear>",
        );
        let nl_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear id=\"nl-1\" width=\"300\" height=\"250\"></NonLinear></NonLinearAds>",
        );
        let render_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\" renderingMode=\"end-card\"></Companion></CompanionAds></Creative>",
        );
        let px_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" pxratio=\"2\" width=\"20\" height=\"20\"></Icon></Icons></Linear>",
        );
        let scal_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear scalable=\"true\" width=\"300\" height=\"250\"></NonLinear></NonLinearAds>",
        );
        let mar_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear maintainAspectRatio=\"true\" width=\"300\" height=\"250\"></NonLinear></NonLinearAds>",
        );
        let xml_enc_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\" xmlEncoded=\"true\"></Companion></CompanionAds></Creative>",
        );
        let icon_xmlenc_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon xmlEncoded=\"true\"></Icon></Icons></Linear>",
        );
        let nl_xmlenc_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear xmlEncoded=\"true\" width=\"300\" height=\"250\"></NonLinear></NonLinearAds>",
        );
        let cpx_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\" pxratio=\"2\"></Companion></CompanionAds></Creative>",
        );
        let wrap_xml = r#"<VAST><Ad><Wrapper followAdditionalWrappers="true" allowMultipleAds="true" fallbackOnNoAd="false"><VASTAdTagURI><![CDATA[https://example.com/vast]]></VASTAdTagURI></Wrapper></Ad></VAST>"#;
        let cond_xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("<Ad id=\"1\">", "<Ad id=\"1\" conditionalAd=\"true\">");
        let mf_scal_xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("<MediaFile ", "<MediaFile scalable=\"true\" ");
        let mf_mar_xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("<MediaFile ", "<MediaFile maintainAspectRatio=\"true\" ");
        let exec_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</InLine>",
            "<AdVerifications><Verification><ExecutableResource type=\"application/javascript\"></ExecutableResource></Verification></AdVerifications></InLine>",
        );
        let js_opt_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</InLine>",
            "<AdVerifications><Verification><JavaScriptResource browserOptional=\"true\"></JavaScriptResource></Verification></AdVerifications></InLine>",
        );
        let ext_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</InLine>",
            "<Extensions><Extension type=\"iab\"></Extension></Extensions></InLine>",
        );
        let mezz_attr_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</MediaFiles>",
            "<Mezzanine type=\"video/mp4\" delivery=\"progressive\"><![CDATA[https://cdn.example.com/ad.mp4]]></Mezzanine></MediaFiles>",
        );
        let params_enc_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<AdParameters xmlEncoded=\"true\"><![CDATA[{}]]></AdParameters></Linear>",
        );
        let mezz_codec_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</MediaFiles>",
            "<Mezzanine id=\"mezz-1\" codec=\"avc1.42E01E\" fileSize=\"1000000\" width=\"1920\" height=\"1080\" bitrate=\"15000\" mediaType=\"video\" scalable=\"true\" maintainAspectRatio=\"true\"><![CDATA[https://cdn.example.com/ad.mp4]]></Mezzanine></MediaFiles>",
        );
        let ad_type_xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("<Ad id=\"1\">", "<Ad id=\"1\" adType=\"video\">");
        let imp_id_xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("<Impression>", "<Impression id=\"imp-1\">");
        let ads_req_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds required=\"all\"><Companion width=\"300\" height=\"250\"></Companion></CompanionAds></Creative>",
        );
        let mf_mt_xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("<MediaFile ", "<MediaFile mediaType=\"2D\" ");
        let icf_var_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><InteractiveCreativeFile variableDuration=\"true\"></InteractiveCreativeFile>",
        );
        let cr_id_xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("<Creative>", "<Creative id=\"cr-1\">");
        let ct_id_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Duration>",
            "</Duration><VideoClicks><ClickThrough id=\"ct-1\"><![CDATA[https://adv.example.com/c]]></ClickThrough></VideoClicks>",
        );
        let trk_off_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<Tracking event=\"midpoint\">",
            "<Tracking event=\"midpoint\" offset=\"00:00:15\">",
        );
        let clk_trk_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Duration>",
            "</Duration><VideoClicks><ClickTracking id=\"clk-1\"><![CDATA[https://t.example.com/click]]></ClickTracking></VideoClicks>",
        );
        let cust_clk_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Duration>",
            "</Duration><VideoClicks><CustomClick id=\"cust-1\"><![CDATA[https://t.example.com/custom]]></CustomClick></VideoClicks>",
        );
        let ict_id_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconClicks><IconClickThrough id=\"ict-1\"><![CDATA[https://adv.example.com/i]]></IconClickThrough></IconClicks></Icon></Icons></Linear>",
        );
        let cct_id_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><CompanionClickThrough id=\"cct-1\"><![CDATA[https://adv.example.com/c]]></CompanionClickThrough></Companion></CompanionAds></Creative>",
        );
        let nlc_id_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\" height=\"250\"><NonLinearClickThrough id=\"nlc-1\"><![CDATA[https://adv.example.com/nl]]></NonLinearClickThrough></NonLinear></NonLinearAds>",
        );
        let ictk_id_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconClicks><IconClickTracking id=\"ictk-1\"><![CDATA[https://t.example.com/i]]></IconClickTracking></IconClicks></Icon></Icons></Linear>",
        );
        let cctk_id_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><CompanionClickTracking id=\"cctk-1\"><![CDATA[https://t.example.com/c]]></CompanionClickTracking></Companion></CompanionAds></Creative>",
        );
        let nlct_id_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\" height=\"250\"><NonLinearClickTracking id=\"nlct-1\"><![CDATA[https://t.example.com/nl]]></NonLinearClickTracking></NonLinear></NonLinearAds>",
        );
        let ivt_id_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconViewTracking id=\"ivt-1\"><![CDATA[https://t.example.com/iv]]></IconViewTracking></Icon></Icons></Linear>",
        );
        let trk_id_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<Tracking event=\"start\">",
            "<Tracking event=\"start\" id=\"trk-1\">",
        );
        let err_id_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</AdTitle>",
            "</AdTitle><Error id=\"err-1\"><![CDATA[https://t.example.com/error]]></Error>",
        );
        let view_elem_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Impression>",
            "</Impression><ViewableImpression><Viewable id=\"v-1\"><![CDATA[https://t.example.com/view]]></Viewable></ViewableImpression>",
        );
        let nv_id_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Impression>",
            "</Impression><ViewableImpression><NotViewable id=\"nv-1\"><![CDATA[https://t.example.com/nv]]></NotViewable></ViewableImpression>",
        );
        let vu_id_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Impression>",
            "</Impression><ViewableImpression><ViewUndetermined id=\"vu-1\"><![CDATA[https://t.example.com/vu]]></ViewUndetermined></ViewableImpression>",
        );
        let surv_id_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</AdTitle>",
            "</AdTitle><Survey id=\"surv-1\"><![CDATA[https://t.example.com/survey]]></Survey>",
        );
        let skip_off_xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("<Linear>", "<Linear skipoffset=\"00:00:05\">");
        let dur_txt_xml = include_str!("../../../fixtures/ok-https-mp4.xml").to_string();
        let exp_txt_xml = include_str!("../../../fixtures/ok-https-mp4.xml").to_string();
        let icon_dur_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" duration=\"00:00:15\"></Icon></Icons></Linear>",
        );
        let icon_off_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" offset=\"00:00:00\"></Icon></Icons></Linear>",
        );
        let msd_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\" height=\"250\" minSuggestedDuration=\"00:00:15\"></NonLinear></NonLinearAds>",
        );
        let asid_txt_xml = include_str!("../../../fixtures/ok-https-mp4.xml").to_string();
        let adtitle_txt_xml = include_str!("../../../fixtures/ok-https-mp4.xml").to_string();
        let desc_txt_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</AdTitle>",
            "</AdTitle><Description>A test ad</Description>",
        );
        let params_txt_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Duration>",
            "</Duration><AdParameters>foo=1</AdParameters>",
        );
        let static_ct_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><StaticResource creativeType=\"image/png\"><![CDATA[https://cdn.example.com/i.png]]></StaticResource></Companion></CompanionAds></Creative>",
        );
        let flash_ct_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><FlashResource creativeType=\"application/x-shockwave-flash\"><![CDATA[https://cdn.example.com/i.swf]]></FlashResource></Companion></CompanionAds></Creative>",
        );
        let imp_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").to_string();
        let err_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</AdTitle>",
            "</AdTitle><Error><![CDATA[https://t.example.com/error]]></Error>",
        );
        let clk_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<VideoClicks><ClickThrough><![CDATA[https://t.example.com/click]]></ClickThrough></VideoClicks></Linear>",
        );
        let surv_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</AdTitle>",
            "</AdTitle><Survey><![CDATA[https://t.example.com/survey]]></Survey>",
        );
        let trk_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").to_string();
        let view_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Impression>",
            "</Impression><ViewableImpression><Viewable><![CDATA[https://t.example.com/view]]></Viewable></ViewableImpression>",
        );
        let clk_trk_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<VideoClicks><ClickTracking><![CDATA[https://t.example.com/clk]]></ClickTracking></VideoClicks></Linear>",
        );
        let mezz_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</MediaFile>",
            "</MediaFile><Mezzanine><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        let mf_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").to_string();
        let tag_uri_xml = r#"<VAST><Ad><Wrapper><VASTAdTagURI><![CDATA[https://example.com/vast]]></VASTAdTagURI></Wrapper></Ad></VAST>"#.to_string();
        let js_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Impression>",
            "</Impression><AdVerifications><Verification vendor=\"omid\"><JavaScriptResource><![CDATA[https://t.example.com/omid.js]]></JavaScriptResource></Verification></AdVerifications>",
        );
        let cc_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<ClosedCaptionFiles><ClosedCaptionFile><![CDATA[https://cdn.example.com/cap.vtt]]></ClosedCaptionFile></ClosedCaptionFiles></Linear>",
        );
        let exec_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Impression>",
            "</Impression><AdVerifications><Verification vendor=\"omid\"><ExecutableResource><![CDATA[https://t.example.com/omid.exe]]></ExecutableResource></Verification></AdVerifications>",
        );
        let ict_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconClicks><IconClickThrough><![CDATA[https://adv.example.com/i]]></IconClickThrough></IconClicks></Icon></Icons></Linear>",
        );
        let cct_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><CompanionClickThrough><![CDATA[https://adv.example.com/c]]></CompanionClickThrough></Companion></CompanionAds></Creative>",
        );
        let nlc_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\" height=\"250\"><NonLinearClickThrough><![CDATA[https://adv.example.com/nl]]></NonLinearClickThrough></NonLinear></NonLinearAds>",
        );
        let ivt_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconViewTracking><![CDATA[https://t.example.com/iv]]></IconViewTracking></Icon></Icons></Linear>",
        );
        let icf_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><InteractiveCreativeFile type=\"application/javascript\"><![CDATA[https://cdn.example.com/v.js]]></InteractiveCreativeFile>",
        );
        let static_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><StaticResource creativeType=\"image/png\"><![CDATA[https://cdn.example.com/i.png]]></StaticResource></Companion></CompanionAds></Creative>",
        );
        let iframe_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><IFrameResource><![CDATA[https://cdn.example.com/frame.html]]></IFrameResource></Companion></CompanionAds></Creative>",
        );
        let html_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><HTMLResource><![CDATA[https://cdn.example.com/ad.html]]></HTMLResource></Companion></CompanionAds></Creative>",
        );
        let flash_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><FlashResource><![CDATA[https://cdn.example.com/ad.swf]]></FlashResource></Companion></CompanionAds></Creative>",
        );
        let icfb_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconClicks><IconClickFallbackImages><IconClickFallbackImage><![CDATA[https://cdn.example.com/icon.png]]></IconClickFallbackImage></IconClickFallbackImages></IconClicks></Icon></Icons></Linear>",
        );
        let custom_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Duration>",
            "</Duration><VideoClicks><CustomClick><![CDATA[https://t.example.com/custom]]></CustomClick></VideoClicks>",
        );
        let ictk_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconClicks><IconClickTracking><![CDATA[https://t.example.com/i]]></IconClickTracking></IconClicks></Icon></Icons></Linear>",
        );
        let cctk_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><CompanionClickTracking><![CDATA[https://t.example.com/c]]></CompanionClickTracking></Companion></CompanionAds></Creative>",
        );
        let nlct_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\" height=\"250\"><NonLinearClickTracking><![CDATA[https://t.example.com/nl]]></NonLinearClickTracking></NonLinear></NonLinearAds>",
        );
        let nv_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Impression>",
            "</Impression><ViewableImpression><NotViewable><![CDATA[https://t.example.com/nv]]></NotViewable></ViewableImpression>",
        );
        let vu_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Impression>",
            "</Impression><ViewableImpression><ViewUndetermined><![CDATA[https://t.example.com/vu]]></ViewUndetermined></ViewableImpression>",
        );
        let ext_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</InLine>",
            "<Extensions><Extension><![CDATA[https://t.example.com/ext]]></Extension></Extensions></InLine>",
        );
        let vp_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</InLine>",
            "<AdVerifications><Verification vendor=\"omid\"><VerificationParameters><![CDATA[https://t.example.com/omid]]></VerificationParameters></Verification></AdVerifications></InLine>",
        );
        let blocked_url_xml = r#"<VAST><Ad><Wrapper><VASTAdTagURI><![CDATA[https://example.com/vast]]></VASTAdTagURI><BlockedAdCategories><![CDATA[IAB1-1]]></BlockedAdCategories></Wrapper></Ad></VAST>"#.to_string();
        let cext_url_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CreativeExtensions><CreativeExtension><![CDATA[https://t.example.com/cext]]></CreativeExtension></CreativeExtensions></Creative>",
        );
        let cext_type_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CreativeExtensions><CreativeExtension type=\"iab\"></CreativeExtension></CreativeExtensions></Creative>",
        );
        let id_value_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<UniversalAdId idRegistry=\"ad-id.org\">ABCD1234567</UniversalAdId>",
            "<UniversalAdId idRegistry=\"ad-id.org\" idValue=\"FROM-ATTR\"></UniversalAdId>",
        );
        let mf_codec_xml = include_str!("../../../fixtures/ok-https-mp4.xml").to_string();
        let blocked_auth_xml = r#"<VAST><Ad><Wrapper><VASTAdTagURI><![CDATA[https://example.com/vast]]></VASTAdTagURI><BlockedAdCategories authority="IAB"><![CDATA[IAB1-1]]></BlockedAdCategories></Wrapper></Ad></VAST>"#.to_string();
        let mf_minbr_xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("bitrate=\"1500\"", "bitrate=\"1500\" minBitrate=\"1000\"");
        let mf_maxbr_xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("bitrate=\"1500\"", "bitrate=\"1500\" maxBitrate=\"5000\"");
        let mezz_minbr_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</MediaFile>",
            "</MediaFile><Mezzanine minBitrate=\"1000\"><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        let mezz_maxbr_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</MediaFile>",
            "</MediaFile><Mezzanine maxBitrate=\"50000\"><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        let mf_br_xml = include_str!("../../../fixtures/ok-https-mp4.xml").to_string();
        let mezz_br_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</MediaFile>",
            "</MediaFile><Mezzanine bitrate=\"8000\"><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        let mf_fs_xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("bitrate=\"1500\"", "bitrate=\"1500\" fileSize=\"1000000\"");
        let mezz_fs_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</MediaFile>",
            "</MediaFile><Mezzanine fileSize=\"5000000\"><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        let mf_w_xml = include_str!("../../../fixtures/ok-https-mp4.xml").to_string();
        let mf_h_xml = include_str!("../../../fixtures/ok-https-mp4.xml").to_string();
        let mezz_w_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</MediaFile>",
            "</MediaFile><Mezzanine width=\"1920\"><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        let mezz_h_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</MediaFile>",
            "</MediaFile><Mezzanine height=\"1080\"><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        let mf_del_xml = include_str!("../../../fixtures/ok-https-mp4.xml").to_string();
        let icon_w_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon width=\"20\"></Icon></Icons></Linear>",
        );
        let icon_h_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon height=\"20\"></Icon></Icons></Linear>",
        );
        let comp_w_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><CompanionAds><Companion width=\"300\"></Companion></CompanionAds>",
        );
        let comp_h_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><CompanionAds><Companion height=\"250\"></Companion></CompanionAds>",
        );
        let nl_w_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\"></NonLinear></NonLinearAds>",
        );
        let nl_h_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear height=\"250\"></NonLinear></NonLinearAds>",
        );
        let cex_w_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><CompanionAds><Companion expandedWidth=\"600\"></Companion></CompanionAds>",
        );
        let cex_h_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><CompanionAds><Companion expandedHeight=\"500\"></Companion></CompanionAds>",
        );
        let nlex_w_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear expandedWidth=\"600\"></NonLinear></NonLinearAds>",
        );
        let nlex_h_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear expandedHeight=\"500\"></NonLinear></NonLinearAds>",
        );
        let cas_w_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><CompanionAds><Companion assetWidth=\"300\"></Companion></CompanionAds>",
        );
        let cas_h_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "</Linear><CompanionAds><Companion assetHeight=\"250\"></Companion></CompanionAds>",
        );
        let icfb_w_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon><IconClicks><IconClickFallbackImages><IconClickFallbackImage width=\"20\"></IconClickFallbackImage></IconClickFallbackImages></IconClicks></Icon></Icons></Linear>",
        );
        let icfb_h_xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon><IconClicks><IconClickFallbackImages><IconClickFallbackImage height=\"20\"></IconClickFallbackImage></IconClickFallbackImages></IconClicks></Icon></Icons></Linear>",
        );
        let mf_type_xml = include_str!("../../../fixtures/ok-https-mp4.xml").to_string();
        let cases = [
            (
                "caption_type",
                "cap.type",
                "policy.caption-type-missing-values",
                cap_xml.as_str(),
                vec!["text/vtt".into()],
            ),
            (
                "caption_file_status",
                "cap.status",
                "policy.caption-file-status-missing-values",
                cap_xml.as_str(),
                vec!["onDemand".into()],
            ),
            (
                "id_registry",
                "id.reg",
                "policy.id-registry-missing-values",
                include_str!("../../../fixtures/ok-https-mp4.xml"),
                vec!["ad-id.org".into()],
            ),
            (
                "icon_program",
                "icon.prog",
                "policy.icon-program-missing-values",
                icon_xml.as_str(),
                vec!["adChoices".into()],
            ),
            (
                "ad_system_version",
                "sys.ver",
                "policy.ad-system-version-missing-values",
                sys_xml.as_str(),
                vec!["1.0".into()],
            ),
            (
                "icon_x_position",
                "icon.x",
                "policy.icon-x-position-missing-values",
                pos_xml.as_str(),
                vec!["right".into()],
            ),
            (
                "icon_y_position",
                "icon.y",
                "policy.icon-y-position-missing-values",
                pos_xml.as_str(),
                vec!["top".into()],
            ),
            (
                "vast_version",
                "vast.ver",
                "policy.vast-version-missing-values",
                include_str!("../../../fixtures/ok-https-mp4.xml"),
                vec!["4.2".into()],
            ),
            (
                "creative_sequence",
                "cr.seq",
                "policy.creative-sequence-missing-values",
                seq_xml.as_str(),
                vec!["1".into()],
            ),
            (
                "ad_sequence",
                "ad.seq",
                "policy.ad-sequence-missing-values",
                ad_xml.as_str(),
                vec!["1".into()],
            ),
            (
                "verification_vendor",
                "ver.vendor",
                "policy.verification-vendor-missing-values",
                vendor_xml.as_str(),
                vec!["iabtechlab.com-omid".into()],
            ),
            (
                "pricing_model",
                "price.model",
                "policy.pricing-model-missing-values",
                pricing_xml.as_str(),
                vec!["CPM".into()],
            ),
            (
                "pricing_currency",
                "price.ccy",
                "policy.pricing-currency-missing-values",
                pricing_xml.as_str(),
                vec!["USD".into()],
            ),
            (
                "category_authority",
                "cat.auth",
                "policy.category-authority-missing-values",
                cat_xml.as_str(),
                vec!["IAB".into()],
            ),
            (
                "advertiser",
                "adv.name",
                "policy.advertiser-missing-values",
                adv_xml.as_str(),
                vec!["Acme".into()],
            ),
            (
                "advertiser_id",
                "adv.id",
                "policy.advertiser-id-missing-values",
                adv_xml.as_str(),
                vec!["adv-1".into()],
            ),
            (
                "ad_system",
                "sys.name",
                "policy.ad-system-missing-values",
                include_str!("../../../fixtures/ok-https-mp4.xml"),
                vec!["Test".into()],
            ),
            (
                "category",
                "cat.name",
                "policy.category-missing-values",
                cat_xml.as_str(),
                vec!["IAB1-1".into()],
            ),
            (
                "pricing",
                "price.amt",
                "policy.pricing-missing-values",
                pricing_xml.as_str(),
                vec!["2.50".into()],
            ),
            (
                "universal_ad_id",
                "uad.id",
                "policy.universal-ad-id-missing-values",
                include_str!("../../../fixtures/ok-https-mp4.xml"),
                vec!["ABCD1234567".into()],
            ),
            (
                "api_framework",
                "api.name",
                "policy.api-framework-missing-values",
                include_str!("../../../fixtures/ok-https-mp4.xml"),
                vec!["omid".into()],
            ),
            (
                "creative_adid",
                "cr.adid",
                "policy.creative-adid-missing-values",
                adid_xml.as_str(),
                vec!["ad-1".into()],
            ),
            (
                "interactive_creative_file_type",
                "icf.type",
                "policy.interactive-creative-file-type-missing-values",
                icf_xml.as_str(),
                vec!["application/javascript".into()],
            ),
            (
                "viewable_impression_id",
                "view.id",
                "policy.viewable-impression-id-missing-values",
                view_xml.as_str(),
                vec!["view-1".into()],
            ),
            (
                "companion_type",
                "comp.type",
                "policy.companion-type-missing-values",
                comp_xml.as_str(),
                vec!["image/png".into()],
            ),
            (
                "icon_type",
                "icon.type",
                "policy.icon-type-missing-values",
                icon_type_xml.as_str(),
                vec!["image/png".into()],
            ),
            (
                "ad_id",
                "ad.id",
                "policy.ad-id-missing-values",
                include_str!("../../../fixtures/ok-https-mp4.xml"),
                vec!["1".into()],
            ),
            (
                "mediafile_id",
                "mf.id",
                "policy.mediafile-id-missing-values",
                mf_id_xml.as_str(),
                vec!["mf-1".into()],
            ),
            (
                "companion_adslot_id",
                "comp.slot",
                "policy.companion-adslot-id-missing-values",
                slot_xml.as_str(),
                vec!["slot-1".into()],
            ),
            (
                "icon_alt_text",
                "icon.alt",
                "policy.icon-alt-text-missing-values",
                alt_xml.as_str(),
                vec!["AdChoices".into()],
            ),
            (
                "companion_alt_text",
                "comp.alt",
                "policy.companion-alt-text-missing-values",
                alt_elem_xml.as_str(),
                vec!["Learn more".into()],
            ),
            (
                "companion_id",
                "comp.id",
                "policy.companion-id-missing-values",
                cid_xml.as_str(),
                vec!["comp-1".into()],
            ),
            (
                "icon_hover_text",
                "icon.hover",
                "policy.icon-hover-text-missing-values",
                hover_xml.as_str(),
                vec!["Learn more".into()],
            ),
            (
                "nonlinear_id",
                "nl.id",
                "policy.nonlinear-id-missing-values",
                nl_xml.as_str(),
                vec!["nl-1".into()],
            ),
            (
                "companion_rendering_mode",
                "comp.mode",
                "policy.companion-rendering-mode-missing-values",
                render_xml.as_str(),
                vec!["end-card".into()],
            ),
            (
                "icon_pxratio",
                "icon.px",
                "policy.icon-pxratio-missing-values",
                px_xml.as_str(),
                vec!["2".into()],
            ),
            (
                "nonlinear_scalable",
                "nl.scal",
                "policy.nonlinear-scalable-missing-values",
                scal_xml.as_str(),
                vec!["true".into()],
            ),
            (
                "nonlinear_maintain_aspect_ratio",
                "nl.mar",
                "policy.nonlinear-maintain-aspect-ratio-missing-values",
                mar_xml.as_str(),
                vec!["true".into()],
            ),
            (
                "companion_xml_encoded",
                "comp.xml",
                "policy.companion-xml-encoded-missing-values",
                xml_enc_xml.as_str(),
                vec!["true".into()],
            ),
            (
                "icon_xml_encoded",
                "icon.xml",
                "policy.icon-xml-encoded-missing-values",
                icon_xmlenc_xml.as_str(),
                vec!["true".into()],
            ),
            (
                "nonlinear_xml_encoded",
                "nl.xml",
                "policy.nonlinear-xml-encoded-missing-values",
                nl_xmlenc_xml.as_str(),
                vec!["true".into()],
            ),
            (
                "companion_pxratio",
                "comp.px",
                "policy.companion-pxratio-missing-values",
                cpx_xml.as_str(),
                vec!["2".into()],
            ),
            (
                "wrapper_follow_additional_wrappers",
                "wrap.follow",
                "policy.wrapper-follow-additional-wrappers-missing-values",
                wrap_xml,
                vec!["true".into()],
            ),
            (
                "wrapper_allow_multiple_ads",
                "wrap.multi",
                "policy.wrapper-allow-multiple-ads-missing-values",
                wrap_xml,
                vec!["true".into()],
            ),
            (
                "wrapper_fallback_on_no_ad",
                "wrap.fallback",
                "policy.wrapper-fallback-on-no-ad-missing-values",
                wrap_xml,
                vec!["false".into()],
            ),
            (
                "ad_conditional_ad",
                "ad.cond",
                "policy.ad-conditional-ad-missing-values",
                cond_xml.as_str(),
                vec!["true".into()],
            ),
            (
                "mediafile_scalable",
                "mf.scal",
                "policy.mediafile-scalable-missing-values",
                mf_scal_xml.as_str(),
                vec!["true".into()],
            ),
            (
                "mediafile_maintain_aspect_ratio",
                "mf.mar",
                "policy.mediafile-maintain-aspect-ratio-missing-values",
                mf_mar_xml.as_str(),
                vec!["true".into()],
            ),
            (
                "executable_resource_type",
                "exec.type",
                "policy.executable-resource-type-missing-values",
                exec_xml.as_str(),
                vec!["application/javascript".into()],
            ),
            (
                "javascript_resource_browser_optional",
                "js.opt",
                "policy.javascript-resource-browser-optional-missing-values",
                js_opt_xml.as_str(),
                vec!["true".into()],
            ),
            (
                "extension_type",
                "ext.type",
                "policy.extension-type-missing-values",
                ext_xml.as_str(),
                vec!["iab".into()],
            ),
            (
                "mezzanine_type",
                "mezz.type",
                "policy.mezzanine-type-missing-values",
                mezz_attr_xml.as_str(),
                vec!["video/mp4".into()],
            ),
            (
                "mezzanine_delivery",
                "mezz.del",
                "policy.mezzanine-delivery-missing-values",
                mezz_attr_xml.as_str(),
                vec!["progressive".into()],
            ),
            (
                "ad_parameters_xml_encoded",
                "params.xml",
                "policy.ad-parameters-xml-encoded-missing-values",
                params_enc_xml.as_str(),
                vec!["true".into()],
            ),
            (
                "mezzanine_codec",
                "mezz.codec",
                "policy.mezzanine-codec-missing-values",
                mezz_codec_xml.as_str(),
                vec!["avc1.42E01E".into()],
            ),
            (
                "mezzanine_media_type",
                "mezz.mt",
                "policy.mezzanine-media-type-missing-values",
                mezz_codec_xml.as_str(),
                vec!["video".into()],
            ),
            (
                "mezzanine_scalable",
                "mezz.scal",
                "policy.mezzanine-scalable-missing-values",
                mezz_codec_xml.as_str(),
                vec!["true".into()],
            ),
            (
                "mezzanine_maintain_aspect_ratio",
                "mezz.mar",
                "policy.mezzanine-maintain-aspect-ratio-missing-values",
                mezz_codec_xml.as_str(),
                vec!["true".into()],
            ),
            (
                "mezzanine_id",
                "mezz.id",
                "policy.mezzanine-id-missing-values",
                mezz_codec_xml.as_str(),
                vec!["mezz-1".into()],
            ),
            (
                "ad_type",
                "ad.type",
                "policy.ad-type-missing-values",
                ad_type_xml.as_str(),
                vec!["video".into()],
            ),
            (
                "impression_id",
                "imp.id",
                "policy.impression-id-missing-values",
                imp_id_xml.as_str(),
                vec!["imp-1".into()],
            ),
            (
                "companion_ads_required",
                "comp.req",
                "policy.companion-ads-required-missing-values",
                ads_req_xml.as_str(),
                vec!["all".into()],
            ),
            (
                "mediafile_media_type",
                "mf.mt",
                "policy.mediafile-media-type-missing-values",
                mf_mt_xml.as_str(),
                vec!["2D".into()],
            ),
            (
                "interactive_creative_file_variable_duration",
                "icf.var",
                "policy.interactive-creative-file-variable-duration-missing-values",
                icf_var_xml.as_str(),
                vec!["true".into()],
            ),
            (
                "creative_id",
                "cr.id",
                "policy.creative-id-missing-values",
                cr_id_xml.as_str(),
                vec!["cr-1".into()],
            ),
            (
                "clickthrough_id",
                "ct.id",
                "policy.clickthrough-id-missing-values",
                ct_id_xml.as_str(),
                vec!["ct-1".into()],
            ),
            (
                "tracking_offset",
                "trk.off",
                "policy.tracking-offset-missing-values",
                trk_off_xml.as_str(),
                vec!["00:00:15".into()],
            ),
            (
                "clicktracking_id",
                "clk.id",
                "policy.clicktracking-id-missing-values",
                clk_trk_xml.as_str(),
                vec!["clk-1".into()],
            ),
            (
                "custom_click_id",
                "cust.id",
                "policy.custom-click-id-missing-values",
                cust_clk_xml.as_str(),
                vec!["cust-1".into()],
            ),
            (
                "icon_clickthrough_id",
                "ict.id",
                "policy.icon-clickthrough-id-missing-values",
                ict_id_xml.as_str(),
                vec!["ict-1".into()],
            ),
            (
                "companion_clickthrough_id",
                "cct.id",
                "policy.companion-clickthrough-id-missing-values",
                cct_id_xml.as_str(),
                vec!["cct-1".into()],
            ),
            (
                "nonlinear_clickthrough_id",
                "nlc.id",
                "policy.nonlinear-clickthrough-id-missing-values",
                nlc_id_xml.as_str(),
                vec!["nlc-1".into()],
            ),
            (
                "icon_clicktracking_id",
                "ictk.id",
                "policy.icon-clicktracking-id-missing-values",
                ictk_id_xml.as_str(),
                vec!["ictk-1".into()],
            ),
            (
                "companion_clicktracking_id",
                "cctk.id",
                "policy.companion-clicktracking-id-missing-values",
                cctk_id_xml.as_str(),
                vec!["cctk-1".into()],
            ),
            (
                "nonlinear_clicktracking_id",
                "nlct.id",
                "policy.nonlinear-clicktracking-id-missing-values",
                nlct_id_xml.as_str(),
                vec!["nlct-1".into()],
            ),
            (
                "icon_viewtracking_id",
                "ivt.id",
                "policy.icon-viewtracking-id-missing-values",
                ivt_id_xml.as_str(),
                vec!["ivt-1".into()],
            ),
            (
                "tracking_id",
                "trk.id",
                "policy.tracking-id-missing-values",
                trk_id_xml.as_str(),
                vec!["trk-1".into()],
            ),
            (
                "error_id",
                "err.id",
                "policy.error-id-missing-values",
                err_id_xml.as_str(),
                vec!["err-1".into()],
            ),
            (
                "viewable_id",
                "view.elem",
                "policy.viewable-id-missing-values",
                view_elem_xml.as_str(),
                vec!["v-1".into()],
            ),
            (
                "notviewable_id",
                "nv.id",
                "policy.notviewable-id-missing-values",
                nv_id_xml.as_str(),
                vec!["nv-1".into()],
            ),
            (
                "viewundetermined_id",
                "vu.id",
                "policy.viewundetermined-id-missing-values",
                vu_id_xml.as_str(),
                vec!["vu-1".into()],
            ),
            (
                "survey_id",
                "surv.id",
                "policy.survey-id-missing-values",
                surv_id_xml.as_str(),
                vec!["surv-1".into()],
            ),
            (
                "skipoffset",
                "skip.off",
                "policy.skipoffset-missing-values",
                skip_off_xml.as_str(),
                vec!["00:00:05".into()],
            ),
            (
                "duration",
                "dur.txt",
                "policy.duration-missing-values",
                dur_txt_xml.as_str(),
                vec!["00:00:30".into()],
            ),
            (
                "expires",
                "exp.txt",
                "policy.expires-missing-values",
                exp_txt_xml.as_str(),
                vec!["3600".into()],
            ),
            (
                "icon_duration",
                "icon.dur",
                "policy.icon-duration-missing-values",
                icon_dur_xml.as_str(),
                vec!["00:00:15".into()],
            ),
            (
                "icon_offset",
                "icon.off",
                "policy.icon-offset-missing-values",
                icon_off_xml.as_str(),
                vec!["00:00:00".into()],
            ),
            (
                "min_suggested_duration",
                "nl.msd",
                "policy.min-suggested-duration-missing-values",
                msd_xml.as_str(),
                vec!["00:00:15".into()],
            ),
            (
                "ad_serving_id",
                "asid.txt",
                "policy.ad-serving-id-missing-values",
                asid_txt_xml.as_str(),
                vec!["srv-001".into()],
            ),
            (
                "ad_title",
                "title.txt",
                "policy.ad-title-missing-values",
                adtitle_txt_xml.as_str(),
                vec!["Test".into()],
            ),
            (
                "description",
                "desc.txt",
                "policy.description-missing-values",
                desc_txt_xml.as_str(),
                vec!["A test ad".into()],
            ),
            (
                "ad_parameters",
                "params.txt",
                "policy.ad-parameters-missing-values",
                params_txt_xml.as_str(),
                vec!["foo=1".into()],
            ),
            (
                "static_resource_creative_type",
                "static.ct",
                "policy.static-resource-creative-type-missing-values",
                static_ct_xml.as_str(),
                vec!["image/png".into()],
            ),
            (
                "flash_resource_creative_type",
                "flash.ct",
                "policy.flash-resource-creative-type-missing-values",
                flash_ct_xml.as_str(),
                vec!["application/x-shockwave-flash".into()],
            ),
            (
                "impression",
                "imp.url",
                "policy.impression-missing-values",
                imp_url_xml.as_str(),
                vec!["https://t.example.com/imp?cb=[CACHEBUSTING]&ts=[TIMESTAMP]".into()],
            ),
            (
                "error",
                "err.url",
                "policy.error-missing-values",
                err_url_xml.as_str(),
                vec!["https://t.example.com/error".into()],
            ),
            (
                "clickthrough",
                "clk.url",
                "policy.clickthrough-missing-values",
                clk_url_xml.as_str(),
                vec!["https://t.example.com/click".into()],
            ),
            (
                "survey",
                "surv.url",
                "policy.survey-missing-values",
                surv_url_xml.as_str(),
                vec!["https://t.example.com/survey".into()],
            ),
            (
                "tracking",
                "trk.url",
                "policy.tracking-missing-values",
                trk_url_xml.as_str(),
                vec!["https://t.example.com/start".into()],
            ),
            (
                "viewable",
                "view.url",
                "policy.viewable-missing-values",
                view_url_xml.as_str(),
                vec!["https://t.example.com/view".into()],
            ),
            (
                "clicktracking",
                "clk.trk.url",
                "policy.clicktracking-missing-values",
                clk_trk_url_xml.as_str(),
                vec!["https://t.example.com/clk".into()],
            ),
            (
                "mezzanine",
                "mezz.url",
                "policy.mezzanine-missing-values",
                mezz_url_xml.as_str(),
                vec!["https://cdn.example.com/mezz.mp4".into()],
            ),
            (
                "mediafile",
                "mf.url",
                "policy.mediafile-missing-values",
                mf_url_xml.as_str(),
                vec!["https://cdn.example.com/ad.mp4".into()],
            ),
            (
                "vast_ad_tag_uri",
                "tag.uri",
                "policy.vast-ad-tag-uri-missing-values",
                tag_uri_xml.as_str(),
                vec!["https://example.com/vast".into()],
            ),
            (
                "javascript_resource",
                "js.url",
                "policy.javascript-resource-missing-values",
                js_url_xml.as_str(),
                vec!["https://t.example.com/omid.js".into()],
            ),
            (
                "closed_caption_file",
                "cc.url",
                "policy.closed-caption-file-missing-values",
                cc_url_xml.as_str(),
                vec!["https://cdn.example.com/cap.vtt".into()],
            ),
            (
                "executable_resource",
                "exec.url",
                "policy.executable-resource-missing-values",
                exec_url_xml.as_str(),
                vec!["https://t.example.com/omid.exe".into()],
            ),
            (
                "icon_clickthrough",
                "ict.url",
                "policy.icon-clickthrough-missing-values",
                ict_url_xml.as_str(),
                vec!["https://adv.example.com/i".into()],
            ),
            (
                "companion_clickthrough",
                "cct.url",
                "policy.companion-clickthrough-missing-values",
                cct_url_xml.as_str(),
                vec!["https://adv.example.com/c".into()],
            ),
            (
                "nonlinear_clickthrough",
                "nlc.url",
                "policy.nonlinear-clickthrough-missing-values",
                nlc_url_xml.as_str(),
                vec!["https://adv.example.com/nl".into()],
            ),
            (
                "icon_viewtracking",
                "ivt.url",
                "policy.icon-viewtracking-missing-values",
                ivt_url_xml.as_str(),
                vec!["https://t.example.com/iv".into()],
            ),
            (
                "interactive_creative_file",
                "icf.url",
                "policy.interactive-creative-file-missing-values",
                icf_url_xml.as_str(),
                vec!["https://cdn.example.com/v.js".into()],
            ),
            (
                "static_resource",
                "static.url",
                "policy.static-resource-missing-values",
                static_url_xml.as_str(),
                vec!["https://cdn.example.com/i.png".into()],
            ),
            (
                "iframe_resource",
                "iframe.url",
                "policy.iframe-resource-missing-values",
                iframe_url_xml.as_str(),
                vec!["https://cdn.example.com/frame.html".into()],
            ),
            (
                "html_resource",
                "html.url",
                "policy.html-resource-missing-values",
                html_url_xml.as_str(),
                vec!["https://cdn.example.com/ad.html".into()],
            ),
            (
                "flash_resource",
                "flash.url",
                "policy.flash-resource-missing-values",
                flash_url_xml.as_str(),
                vec!["https://cdn.example.com/ad.swf".into()],
            ),
            (
                "icon_clickfallback",
                "icfb.url",
                "policy.icon-clickfallback-missing-values",
                icfb_url_xml.as_str(),
                vec!["https://cdn.example.com/icon.png".into()],
            ),
            (
                "custom_click",
                "custom.url",
                "policy.custom-click-missing-values",
                custom_url_xml.as_str(),
                vec!["https://t.example.com/custom".into()],
            ),
            (
                "icon_clicktracking",
                "ictk.url",
                "policy.icon-clicktracking-missing-values",
                ictk_url_xml.as_str(),
                vec!["https://t.example.com/i".into()],
            ),
            (
                "companion_clicktracking",
                "cctk.url",
                "policy.companion-clicktracking-missing-values",
                cctk_url_xml.as_str(),
                vec!["https://t.example.com/c".into()],
            ),
            (
                "nonlinear_clicktracking",
                "nlct.url",
                "policy.nonlinear-clicktracking-missing-values",
                nlct_url_xml.as_str(),
                vec!["https://t.example.com/nl".into()],
            ),
            (
                "notviewable",
                "nv.url",
                "policy.notviewable-missing-values",
                nv_url_xml.as_str(),
                vec!["https://t.example.com/nv".into()],
            ),
            (
                "viewundetermined",
                "vu.url",
                "policy.viewundetermined-missing-values",
                vu_url_xml.as_str(),
                vec!["https://t.example.com/vu".into()],
            ),
            (
                "extension",
                "ext.url",
                "policy.extension-missing-values",
                ext_url_xml.as_str(),
                vec!["https://t.example.com/ext".into()],
            ),
            (
                "verification_parameters",
                "vp.url",
                "policy.verification-parameters-missing-values",
                vp_url_xml.as_str(),
                vec!["https://t.example.com/omid".into()],
            ),
            (
                "blocked_ad_categories",
                "blocked.cat",
                "policy.blocked-ad-categories-missing-values",
                blocked_url_xml.as_str(),
                vec!["IAB1-1".into()],
            ),
            (
                "creative_extension",
                "cext.url",
                "policy.creative-extension-missing-values",
                cext_url_xml.as_str(),
                vec!["https://t.example.com/cext".into()],
            ),
            (
                "creative_extension_type",
                "cext.type",
                "policy.creative-extension-type-missing-values",
                cext_type_xml.as_str(),
                vec!["iab".into()],
            ),
            (
                "id_value",
                "uad.idvalue",
                "policy.id-value-missing-values",
                id_value_xml.as_str(),
                vec!["FROM-ATTR".into()],
            ),
            (
                "mediafile_codec",
                "mf.codec",
                "policy.mediafile-codec-missing-values",
                mf_codec_xml.as_str(),
                vec!["avc1.42E01E,mp4a.40.2".into()],
            ),
            (
                "blocked_ad_categories_authority",
                "blocked.auth",
                "policy.blocked-ad-categories-authority-missing-values",
                blocked_auth_xml.as_str(),
                vec!["IAB".into()],
            ),
            (
                "mediafile_minbitrate",
                "mf.minbr",
                "policy.mediafile-minbitrate-missing-values",
                mf_minbr_xml.as_str(),
                vec!["1000".into()],
            ),
            (
                "mediafile_maxbitrate",
                "mf.maxbr",
                "policy.mediafile-maxbitrate-missing-values",
                mf_maxbr_xml.as_str(),
                vec!["5000".into()],
            ),
            (
                "mezzanine_minbitrate",
                "mezz.minbr",
                "policy.mezzanine-minbitrate-missing-values",
                mezz_minbr_xml.as_str(),
                vec!["1000".into()],
            ),
            (
                "mezzanine_maxbitrate",
                "mezz.maxbr",
                "policy.mezzanine-maxbitrate-missing-values",
                mezz_maxbr_xml.as_str(),
                vec!["50000".into()],
            ),
            (
                "mediafile_bitrate",
                "mf.br",
                "policy.mediafile-bitrate-missing-values",
                mf_br_xml.as_str(),
                vec!["1500".into()],
            ),
            (
                "mezzanine_bitrate",
                "mezz.br",
                "policy.mezzanine-bitrate-missing-values",
                mezz_br_xml.as_str(),
                vec!["8000".into()],
            ),
            (
                "mediafile_filesize",
                "mf.fs",
                "policy.mediafile-filesize-missing-values",
                mf_fs_xml.as_str(),
                vec!["1000000".into()],
            ),
            (
                "mezzanine_filesize",
                "mezz.fs",
                "policy.mezzanine-filesize-missing-values",
                mezz_fs_xml.as_str(),
                vec!["5000000".into()],
            ),
            (
                "mediafile_width",
                "mf.w",
                "policy.mediafile-width-missing-values",
                mf_w_xml.as_str(),
                vec!["640".into()],
            ),
            (
                "mediafile_height",
                "mf.h",
                "policy.mediafile-height-missing-values",
                mf_h_xml.as_str(),
                vec!["360".into()],
            ),
            (
                "mezzanine_width",
                "mezz.w",
                "policy.mezzanine-width-missing-values",
                mezz_w_xml.as_str(),
                vec!["1920".into()],
            ),
            (
                "mezzanine_height",
                "mezz.h",
                "policy.mezzanine-height-missing-values",
                mezz_h_xml.as_str(),
                vec!["1080".into()],
            ),
            (
                "mediafile_delivery",
                "mf.del",
                "policy.mediafile-delivery-missing-values",
                mf_del_xml.as_str(),
                vec!["progressive".into()],
            ),
            (
                "icon_width",
                "ic.w",
                "policy.icon-width-missing-values",
                icon_w_xml.as_str(),
                vec!["20".into()],
            ),
            (
                "icon_height",
                "ic.h",
                "policy.icon-height-missing-values",
                icon_h_xml.as_str(),
                vec!["20".into()],
            ),
            (
                "companion_width",
                "comp.w",
                "policy.companion-width-missing-values",
                comp_w_xml.as_str(),
                vec!["300".into()],
            ),
            (
                "companion_height",
                "comp.h",
                "policy.companion-height-missing-values",
                comp_h_xml.as_str(),
                vec!["250".into()],
            ),
            (
                "nonlinear_width",
                "nl.w",
                "policy.nonlinear-width-missing-values",
                nl_w_xml.as_str(),
                vec!["300".into()],
            ),
            (
                "nonlinear_height",
                "nl.h",
                "policy.nonlinear-height-missing-values",
                nl_h_xml.as_str(),
                vec!["250".into()],
            ),
            (
                "companion_expanded_width",
                "comp.ew",
                "policy.companion-expanded-width-missing-values",
                cex_w_xml.as_str(),
                vec!["600".into()],
            ),
            (
                "companion_expanded_height",
                "comp.eh",
                "policy.companion-expanded-height-missing-values",
                cex_h_xml.as_str(),
                vec!["500".into()],
            ),
            (
                "nonlinear_expanded_width",
                "nl.ew",
                "policy.nonlinear-expanded-width-missing-values",
                nlex_w_xml.as_str(),
                vec!["600".into()],
            ),
            (
                "nonlinear_expanded_height",
                "nl.eh",
                "policy.nonlinear-expanded-height-missing-values",
                nlex_h_xml.as_str(),
                vec!["500".into()],
            ),
            (
                "companion_asset_width",
                "comp.aw",
                "policy.companion-asset-width-missing-values",
                cas_w_xml.as_str(),
                vec!["300".into()],
            ),
            (
                "companion_asset_height",
                "comp.ah",
                "policy.companion-asset-height-missing-values",
                cas_h_xml.as_str(),
                vec!["250".into()],
            ),
            (
                "icon_click_fallback_width",
                "icfb.w",
                "policy.icon-click-fallback-width-missing-values",
                icfb_w_xml.as_str(),
                vec!["20".into()],
            ),
            (
                "icon_click_fallback_height",
                "icfb.h",
                "policy.icon-click-fallback-height-missing-values",
                icfb_h_xml.as_str(),
                vec!["20".into()],
            ),
            (
                "mediafile_type",
                "mf.type",
                "policy.mediafile-type-missing-values",
                mf_type_xml.as_str(),
                vec!["video/mp4".into()],
            ),
        ];
        for (kind, id, warn_id, xml, values) in cases {
            let overlay = PolicyOverlay {
                id: "ok".into(),
                requirements: vec![PolicyRequirement {
                    id: id.into(),
                    kind: kind.into(),
                    message: "need attr".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(
                        values
                            .iter()
                            .cloned()
                            .chain(["  ".into(), "".into()])
                            .collect(),
                    ),
                }],
                ..Default::default()
            };
            let mut ok = Vec::new();
            apply_overlay(&overlay, xml, &mut ok);
            assert!(!ok.iter().any(|f| f.requirement_id == id), "{kind}");
            let blank = PolicyOverlay {
                id: "blank".into(),
                requirements: vec![PolicyRequirement {
                    id: format!("blank.{kind}"),
                    kind: kind.into(),
                    message: "need attr".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["  ".into(), "".into()]),
                }],
                ..Default::default()
            };
            let mut warn = Vec::new();
            apply_overlay(&blank, xml, &mut warn);
            assert!(
                warn.iter()
                    .any(|f| f.requirement_id == warn_id && f.severity == "warning"),
                "{kind}"
            );
            assert!(
                !warn
                    .iter()
                    .any(|f| f.requirement_id == format!("blank.{kind}")),
                "{kind}"
            );
        }
    }

    #[test]
    fn leftover_empty_require_macros_values_are_dropped() {
        let overlay = PolicyOverlay {
            id: "macros".into(),
            requirements: vec![PolicyRequirement {
                id: "macros.tokens".into(),
                kind: "require_macros".into(),
                message: "need macros".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["CACHEBUSTING".into(), "  ".into(), "".into()]),
            }],
            ..Default::default()
        };
        let mut ok = Vec::new();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut ok,
        );
        assert!(!ok.iter().any(|f| f.requirement_id == "macros.tokens"));
        let blank = PolicyOverlay {
            id: "blank".into(),
            requirements: vec![PolicyRequirement {
                id: "blank.macros".into(),
                kind: "require_macros".into(),
                message: "need macros".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["  ".into(), "".into()]),
            }],
            ..Default::default()
        };
        let mut warn = Vec::new();
        apply_overlay(
            &blank,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut warn,
        );
        assert!(warn.iter().any(|f| {
            f.requirement_id == "policy.require-macros-missing-values" && f.severity == "warning"
        }));
        assert!(!warn.iter().any(|f| f.requirement_id == "blank.macros"));
    }

    #[test]
    fn leftover_empty_aspect_ratio_is_dropped() {
        let overlay = PolicyOverlay {
            id: "aspect".into(),
            requirements: vec![PolicyRequirement {
                id: "aspect.ratio".into(),
                kind: "aspect_ratio".into(),
                message: "need 16:9".into(),
                element: None,
                max: None,
                min: None,
                value: Some("  16:9  ".into()),
                values: None,
            }],
            ..Default::default()
        };
        let mut ok = Vec::new();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut ok,
        );
        assert!(!ok.iter().any(|f| f.requirement_id == "aspect.ratio"));
        let blank = PolicyOverlay {
            id: "blank".into(),
            requirements: vec![PolicyRequirement {
                id: "blank.aspect".into(),
                kind: "aspect_ratio".into(),
                message: "need ratio".into(),
                element: None,
                max: None,
                min: None,
                value: Some("  ".into()),
                values: None,
            }],
            ..Default::default()
        };
        let mut warn = Vec::new();
        apply_overlay(
            &blank,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut warn,
        );
        assert!(warn.iter().any(|f| {
            f.requirement_id == "policy.aspect-ratio-missing-value" && f.severity == "warning"
        }));
        assert!(!warn.iter().any(|f| f.requirement_id == "blank.aspect"));
    }

    #[test]
    fn leftover_empty_min_max_bounds_are_dropped() {
        let cases = [
            (
                "max_duration_seconds",
                "blank.max-dur",
                "policy.max-duration-missing-max",
            ),
            (
                "min_duration_seconds",
                "blank.min-dur",
                "policy.min-duration-missing-min",
            ),
            (
                "max_bitrate",
                "blank.max-br",
                "policy.max-bitrate-missing-max",
            ),
            (
                "min_bitrate",
                "blank.min-br",
                "policy.min-bitrate-missing-min",
            ),
            ("min_width", "blank.min-w", "policy.min-width-missing-min"),
            ("min_height", "blank.min-h", "policy.min-height-missing-min"),
            ("max_width", "blank.max-w", "policy.max-width-missing-max"),
            ("max_height", "blank.max-h", "policy.max-height-missing-max"),
            (
                "max_skipoffset_seconds",
                "blank.skip",
                "policy.max-skipoffset-missing-max",
            ),
            (
                "min_suggested_duration_seconds",
                "blank.msd",
                "policy.min-suggested-duration-missing-min",
            ),
            (
                "min_icon_duration_seconds",
                "blank.icon-dur",
                "policy.min-icon-duration-missing-min",
            ),
            (
                "max_icon_offset_seconds",
                "blank.icon-off",
                "policy.max-icon-offset-missing-max",
            ),
            (
                "min_icon_width",
                "blank.icon-w",
                "policy.min-icon-width-missing-min",
            ),
            (
                "min_icon_height",
                "blank.icon-h",
                "policy.min-icon-height-missing-min",
            ),
            (
                "min_icon_click_fallback_width",
                "blank.fb-w",
                "policy.min-icon-click-fallback-width-missing-min",
            ),
            (
                "min_icon_click_fallback_height",
                "blank.fb-h",
                "policy.min-icon-click-fallback-height-missing-min",
            ),
            (
                "min_companion_width",
                "blank.comp-w",
                "policy.min-companion-width-missing-min",
            ),
            (
                "min_companion_height",
                "blank.comp-h",
                "policy.min-companion-height-missing-min",
            ),
            (
                "min_companion_expanded_width",
                "blank.comp-ew",
                "policy.min-companion-expanded-width-missing-min",
            ),
            (
                "min_companion_expanded_height",
                "blank.comp-eh",
                "policy.min-companion-expanded-height-missing-min",
            ),
            (
                "min_companion_asset_width",
                "blank.comp-aw",
                "policy.min-companion-asset-width-missing-min",
            ),
            (
                "min_companion_asset_height",
                "blank.comp-ah",
                "policy.min-companion-asset-height-missing-min",
            ),
            (
                "min_nonlinear_expanded_width",
                "blank.nl-ew",
                "policy.min-nonlinear-expanded-width-missing-min",
            ),
            (
                "min_nonlinear_expanded_height",
                "blank.nl-eh",
                "policy.min-nonlinear-expanded-height-missing-min",
            ),
            (
                "min_nonlinear_width",
                "blank.nl-w",
                "policy.min-nonlinear-width-missing-min",
            ),
            (
                "min_nonlinear_height",
                "blank.nl-h",
                "policy.min-nonlinear-height-missing-min",
            ),
            (
                "min_mezzanine_file_size",
                "blank.mezz-fs",
                "policy.min-mezzanine-file-size-missing-min",
            ),
            (
                "min_mediafile_file_size",
                "blank.mf-fs",
                "policy.min-mediafile-file-size-missing-min",
            ),
            (
                "min_mezzanine_width",
                "blank.mezz-w",
                "policy.min-mezzanine-width-missing-min",
            ),
            (
                "min_mezzanine_height",
                "blank.mezz-h",
                "policy.min-mezzanine-height-missing-min",
            ),
            (
                "min_mezzanine_bitrate",
                "blank.mezz-br",
                "policy.min-mezzanine-bitrate-missing-min",
            ),
            (
                "max_mezzanine_bitrate",
                "blank.mezz-br-max",
                "policy.max-mezzanine-bitrate-missing-max",
            ),
            (
                "min_ad_serving_id_length",
                "blank.mid",
                "policy.min-ad-serving-id-length-missing-min",
            ),
            (
                "max_expires_seconds",
                "blank.max-exp",
                "policy.max-expires-missing-max",
            ),
            (
                "min_expires_seconds",
                "blank.min-exp",
                "policy.min-expires-missing-min",
            ),
            (
                "min_description_length",
                "blank.desc",
                "policy.min-description-length-missing-min",
            ),
            (
                "min_ad_title_length",
                "blank.title",
                "policy.min-ad-title-length-missing-min",
            ),
            (
                "min_ad_parameters_length",
                "blank.params",
                "policy.min-ad-parameters-length-missing-min",
            ),
            (
                "max_mezzanine_width",
                "blank.mezz-max-w",
                "policy.max-mezzanine-width-missing-max",
            ),
            (
                "max_mezzanine_height",
                "blank.mezz-max-h",
                "policy.max-mezzanine-height-missing-max",
            ),
            (
                "max_companion_width",
                "blank.comp-max-w",
                "policy.max-companion-width-missing-max",
            ),
            (
                "max_companion_height",
                "blank.comp-max-h",
                "policy.max-companion-height-missing-max",
            ),
            (
                "max_icon_width",
                "blank.icon-max-w",
                "policy.max-icon-width-missing-max",
            ),
            (
                "max_icon_height",
                "blank.icon-max-h",
                "policy.max-icon-height-missing-max",
            ),
            (
                "max_nonlinear_width",
                "blank.nl-max-w",
                "policy.max-nonlinear-width-missing-max",
            ),
            (
                "max_nonlinear_height",
                "blank.nl-max-h",
                "policy.max-nonlinear-height-missing-max",
            ),
            (
                "max_icon_click_fallback_width",
                "blank.icfb-max-w",
                "policy.max-icon-click-fallback-width-missing-max",
            ),
            (
                "max_icon_click_fallback_height",
                "blank.icfb-max-h",
                "policy.max-icon-click-fallback-height-missing-max",
            ),
            (
                "max_companion_expanded_width",
                "blank.comp-exp-max-w",
                "policy.max-companion-expanded-width-missing-max",
            ),
            (
                "max_companion_expanded_height",
                "blank.comp-exp-max-h",
                "policy.max-companion-expanded-height-missing-max",
            ),
            (
                "max_companion_asset_width",
                "blank.comp-asset-max-w",
                "policy.max-companion-asset-width-missing-max",
            ),
            (
                "max_companion_asset_height",
                "blank.comp-asset-max-h",
                "policy.max-companion-asset-height-missing-max",
            ),
            (
                "max_nonlinear_expanded_width",
                "blank.nl-exp-max-w",
                "policy.max-nonlinear-expanded-width-missing-max",
            ),
            (
                "max_nonlinear_expanded_height",
                "blank.nl-exp-max-h",
                "policy.max-nonlinear-expanded-height-missing-max",
            ),
        ];
        for (kind, id, warn_id) in cases {
            let overlay = PolicyOverlay {
                id: "blank".into(),
                requirements: vec![PolicyRequirement {
                    id: id.into(),
                    kind: kind.into(),
                    message: "need bound".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            };
            let mut warn = Vec::new();
            apply_overlay(
                &overlay,
                include_str!("../../../fixtures/ok-https-mp4.xml"),
                &mut warn,
            );
            assert!(
                warn.iter()
                    .any(|f| f.requirement_id == warn_id && f.severity == "warning"),
                "{kind}"
            );
            assert!(!warn.iter().any(|f| f.requirement_id == id), "{kind}");
        }
    }

    fn skipoffset_overlay(max: u64) -> PolicyOverlay {
        PolicyOverlay {
            id: "skip".into(),
            requirements: vec![PolicyRequirement {
                id: "skip.max".into(),
                kind: "max_skipoffset_seconds".into(),
                message: "skip too late".into(),
                element: None,
                max: Some(max),
                min: None,
                value: None,
                values: None,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn skipoffset_passes_when_under_max_or_absent() {
        let overlay = skipoffset_overlay(5);
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("<Linear>", "<Linear skipoffset=\"00:00:03\">");
        let mut findings = Vec::new();
        apply_overlay(&overlay, &xml, &mut findings);
        assert!(!findings.iter().any(|f| f.requirement_id == "skip.max"));
        findings.clear();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut findings,
        );
        assert!(!findings.iter().any(|f| f.requirement_id == "skip.max"));
    }

    #[test]
    fn skipoffset_fails_when_later_linear_exceeds_or_percent() {
        let overlay = skipoffset_overlay(5);
        let late = r#"<VAST><Ad><InLine><Creatives>
<Creative><Linear skipoffset="00:00:01"><Duration>00:00:15</Duration></Linear></Creative>
<Creative><Linear skipoffset="00:00:10"><Duration>00:00:30</Duration>
<MediaFiles><MediaFile type="video/mp4">https://cdn.example.com/ad.mp4</MediaFile></MediaFiles>
</Linear></Creative>
</Creatives></InLine></Ad></VAST>"#;
        let mut findings = Vec::new();
        apply_overlay(&overlay, late, &mut findings);
        assert!(findings.iter().any(|f| f.requirement_id == "skip.max"));
        let pct = r#"<VAST><Ad><InLine><Creative>
<Linear skipoffset="25%"><Duration>00:00:30</Duration></Linear>
</Creative></InLine></Ad></VAST>"#;
        findings.clear();
        apply_overlay(&overlay, pct, &mut findings);
        assert!(findings.iter().any(|f| f.requirement_id == "skip.max"));
        let spaced = r#"<VAST><Ad><InLine><Creative>
<Linear skipoffset = "00:00:10"><Duration>00:00:30</Duration></Linear>
</Creative></InLine></Ad></VAST>"#;
        findings.clear();
        apply_overlay(&overlay, spaced, &mut findings);
        assert!(findings.iter().any(|f| f.requirement_id == "skip.max"));
        let nonlinear = r#"<VAST><Ad><InLine><Creative>
<Linear><Duration>00:00:30</Duration></Linear>
<NonLinearAds>
<NonLinear width="300" height="250" skipoffset="00:00:10">
<StaticResource>https://cdn.example.com/nl.png</StaticResource>
</NonLinear>
</NonLinearAds>
</Creative></InLine></Ad></VAST>"#;
        findings.clear();
        apply_overlay(&overlay, nonlinear, &mut findings);
        assert!(findings.iter().any(|f| f.requirement_id == "skip.max"));
    }

    fn duration_overlay(kind: &str, bound: u64, id: &str) -> PolicyOverlay {
        let (min, max) = if kind.starts_with("min") {
            (Some(bound), None)
        } else {
            (None, Some(bound))
        };
        PolicyOverlay {
            id: "dur".into(),
            requirements: vec![PolicyRequirement {
                id: id.into(),
                kind: kind.into(),
                message: "duration bound".into(),
                element: None,
                max,
                min,
                value: None,
                values: None,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn duration_bounds_read_later_linear() {
        let max = duration_overlay("max_duration_seconds", 30, "dur.max");
        let min = duration_overlay("min_duration_seconds", 6, "dur.min");
        let late_long = r#"<VAST><Ad><InLine><Creatives>
<Creative><Linear><Duration>00:00:15</Duration></Linear></Creative>
<Creative><Linear><Duration>00:01:00</Duration></Linear></Creative>
</Creatives></InLine></Ad></VAST>"#;
        let mut findings = Vec::new();
        apply_overlay(&max, late_long, &mut findings);
        assert!(findings.iter().any(|f| f.requirement_id == "dur.max"));
        let early_short = r#"<VAST><Ad><InLine><Creatives>
<Creative><Linear><Duration>00:00:05</Duration></Linear></Creative>
<Creative><Linear><Duration>00:00:30</Duration></Linear></Creative>
</Creatives></InLine></Ad></VAST>"#;
        findings.clear();
        apply_overlay(&min, early_short, &mut findings);
        assert!(findings.iter().any(|f| f.requirement_id == "dur.min"));
        let empty_then_ok = r#"<VAST><Ad><InLine><Creatives>
<Creative><Linear><Duration></Duration></Linear></Creative>
<Creative><Linear><Duration>00:00:15</Duration></Linear></Creative>
</Creatives></InLine></Ad></VAST>"#;
        findings.clear();
        apply_overlay(&min, empty_then_ok, &mut findings);
        assert!(!findings.iter().any(|f| f.requirement_id == "dur.min"));
        findings.clear();
        apply_overlay(&max, empty_then_ok, &mut findings);
        assert!(!findings.iter().any(|f| f.requirement_id == "dur.max"));
        let garbage = r#"<VAST><Ad><InLine><Creatives>
<Creative><Linear><Duration>n/a</Duration></Linear></Creative>
<Creative><Linear><Duration>00:00:15</Duration></Linear></Creative>
</Creatives></InLine></Ad></VAST>"#;
        findings.clear();
        apply_overlay(&min, garbage, &mut findings);
        assert!(findings.iter().any(|f| f.requirement_id == "dur.min"));
        findings.clear();
        apply_overlay(&max, garbage, &mut findings);
        assert!(findings.iter().any(|f| f.requirement_id == "dur.max"));
    }

    #[test]
    fn forbid_vpaid_hits_later_icf_and_nonlinear() {
        let overlay = PolicyOverlay {
            id: "vp".into(),
            requirements: vec![PolicyRequirement {
                id: "vp.no".into(),
                kind: "forbid_vpaid".into(),
                message: "no vpaid".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: None,
            }],
            ..Default::default()
        };
        let icf = r#"<VAST><Ad><InLine><Creative>
<Linear><MediaFiles>
<MediaFile type="video/mp4">https://cdn.example.com/ad.mp4</MediaFile>
<InteractiveCreativeFile type="application/javascript">https://cdn.example.com/a.js</InteractiveCreativeFile>
<InteractiveCreativeFile type="application/javascript" apiFramework="VPAID">https://cdn.example.com/v.js</InteractiveCreativeFile>
</MediaFiles></Linear>
</Creative></InLine></Ad></VAST>"#;
        let mut findings = Vec::new();
        apply_overlay(&overlay, icf, &mut findings);
        assert!(findings.iter().any(|f| f.requirement_id == "vp.no"));
        let nonlinear = r#"<VAST><Ad><InLine><Creative>
<Linear><MediaFiles>
<MediaFile type="video/mp4">https://cdn.example.com/ad.mp4</MediaFile>
</MediaFiles></Linear>
<NonLinearAds>
<NonLinear width="300" height="250" apiFramework="VPAID">
<StaticResource>https://cdn.example.com/nl.js</StaticResource>
</NonLinear>
</NonLinearAds>
</Creative></InLine></Ad></VAST>"#;
        findings.clear();
        apply_overlay(&overlay, nonlinear, &mut findings);
        assert!(findings.iter().any(|f| f.requirement_id == "vp.no"));
        findings.clear();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut findings,
        );
        assert!(!findings.iter().any(|f| f.requirement_id == "vp.no"));
    }

    #[test]
    fn media_type_passes_when_a_later_file_matches() {
        let overlay = PolicyOverlay {
            id: "mime".into(),
            requirements: vec![PolicyRequirement {
                id: "mime.mp4".into(),
                kind: "media_type".into(),
                message: "need mp4".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["video/mp4".into()]),
            }],
            ..Default::default()
        };
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/webm\" width=\"640\" height=\"360\"><![CDATA[https://cdn.example.com/a.webm]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
        );
        assert!(xml.contains("video/webm"));
        assert!(xml.contains("video/mp4"));
        let mut findings = Vec::new();
        apply_overlay(&overlay, &xml, &mut findings);
        assert!(!findings.iter().any(|f| f.requirement_id == "mime.mp4"));
    }

    #[test]
    fn container_passes_when_a_later_file_matches() {
        let overlay = PolicyOverlay {
            id: "box".into(),
            requirements: vec![PolicyRequirement {
                id: "box.mp4".into(),
                kind: "container".into(),
                message: "need mp4".into(),
                element: None,
                max: None,
                min: None,
                value: Some("mp4".into()),
                values: None,
            }],
            ..Default::default()
        };
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/webm\" width=\"640\" height=\"360\"><![CDATA[https://cdn.example.com/a.webm]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
        );
        assert!(xml.contains("video/webm"));
        let mut findings = Vec::new();
        apply_overlay(&overlay, &xml, &mut findings);
        assert!(!findings.iter().any(|f| f.requirement_id == "box.mp4"));
    }

    #[test]
    fn delivery_passes_when_a_later_file_matches() {
        let overlay = PolicyOverlay {
            id: "del".into(),
            requirements: vec![PolicyRequirement {
                id: "del.prog".into(),
                kind: "delivery".into(),
                message: "need progressive".into(),
                element: None,
                max: None,
                min: None,
                value: Some("progressive".into()),
                values: None,
            }],
            ..Default::default()
        };
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"streaming\" type=\"video/mp4\" width=\"640\" height=\"360\"><![CDATA[https://cdn.example.com/a.m3u8]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
        );
        assert!(xml.contains("delivery=\"streaming\""));
        let mut findings = Vec::new();
        apply_overlay(&overlay, &xml, &mut findings);
        assert!(!findings.iter().any(|f| f.requirement_id == "del.prog"));
    }

    #[test]
    fn delivery_fails_closed_without_attr() {
        let overlay = PolicyOverlay {
            id: "del".into(),
            requirements: vec![PolicyRequirement {
                id: "del.prog".into(),
                kind: "delivery".into(),
                message: "need progressive".into(),
                element: None,
                max: None,
                min: None,
                value: Some("progressive".into()),
                values: None,
            }],
            ..Default::default()
        };
        let mut miss = Vec::new();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/wrapper.xml"),
            &mut miss,
        );
        assert!(miss.iter().any(|f| f.requirement_id == "del.prog"));
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("delivery=\"progressive\" ", "");
        let mut ok = Vec::new();
        apply_overlay(&overlay, &xml, &mut ok);
        assert!(ok.iter().any(|f| f.requirement_id == "del.prog"));
    }

    #[test]
    fn size_and_aspect_pass_when_a_later_file_matches() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"320\" height=\"240\"><![CDATA[https://cdn.example.com/a.mp4]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
        );
        assert!(xml.contains("width=\"320\""));
        for (kind, id, min, value) in [
            ("min_width", "size.min-w", Some(640), None),
            ("min_height", "size.min-h", Some(360), None),
            ("aspect_ratio", "size.aspect", None, Some("16:9")),
        ] {
            let overlay = PolicyOverlay {
                id: "size".into(),
                requirements: vec![PolicyRequirement {
                    id: id.into(),
                    kind: kind.into(),
                    message: "need size".into(),
                    element: None,
                    max: None,
                    min,
                    value: value.map(str::to_string),
                    values: None,
                }],
                ..Default::default()
            };
            let mut findings = Vec::new();
            apply_overlay(&overlay, &xml, &mut findings);
            assert!(!findings.iter().any(|f| f.requirement_id == id), "{kind}");
        }
    }

    #[test]
    fn bitrate_and_codec_pass_when_a_later_file_matches() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"200\" codecs=\"vp09.00.10.08\"><![CDATA[https://cdn.example.com/a.mp4]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
        );
        assert!(xml.contains("bitrate=\"200\""));
        assert!(xml.contains("vp09"));
        let cases: [(&str, &str, Option<u64>, Option<u64>, Option<&str>); 4] = [
            ("min_bitrate", "br.min", Some(1000), None, None),
            ("max_bitrate", "br.max", None, Some(20000), None),
            ("codec", "br.avc", None, None, Some("avc")),
            ("audio_codec", "br.aac", None, None, Some("mp4a")),
        ];
        for (kind, id, min, max, value) in cases {
            let overlay = PolicyOverlay {
                id: "br".into(),
                requirements: vec![PolicyRequirement {
                    id: id.into(),
                    kind: kind.into(),
                    message: "need later file".into(),
                    element: None,
                    max,
                    min,
                    value: value.map(str::to_string),
                    values: None,
                }],
                ..Default::default()
            };
            let mut findings = Vec::new();
            apply_overlay(&overlay, &xml, &mut findings);
            assert!(!findings.iter().any(|f| f.requirement_id == id), "{kind}");
        }
    }

    #[test]
    fn min_and_max_bitrate_require_one_file_in_range() {
        let overlay = PolicyOverlay {
            id: "br".into(),
            requirements: vec![
                PolicyRequirement {
                    id: "br.min".into(),
                    kind: "min_bitrate".into(),
                    message: "need floor".into(),
                    element: None,
                    max: None,
                    min: Some(1000),
                    value: None,
                    values: None,
                },
                PolicyRequirement {
                    id: "br.max".into(),
                    kind: "max_bitrate".into(),
                    message: "need cap".into(),
                    element: None,
                    max: Some(20000),
                    min: None,
                    value: None,
                    values: None,
                },
            ],
            ..Default::default()
        };
        let split = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"200\"><![CDATA[https://cdn.example.com/a.mp4]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"50000\"><![CDATA[https://cdn.example.com/b.mp4]]></MediaFile>",
        );
        let mut miss = Vec::new();
        apply_overlay(&overlay, &split, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "br.min"));
        assert!(miss.iter().any(|f| f.requirement_id == "br.max"));
        let in_range = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"200\"><![CDATA[https://cdn.example.com/a.mp4]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
        );
        let mut ok = Vec::new();
        apply_overlay(&overlay, &in_range, &mut ok);
        assert!(!ok.iter().any(|f| f.requirement_id == "br.min"));
        assert!(!ok.iter().any(|f| f.requirement_id == "br.max"));
    }

    #[test]
    fn min_width_and_height_require_one_file() {
        let overlay = PolicyOverlay {
            id: "size".into(),
            requirements: vec![
                PolicyRequirement {
                    id: "size.min-w".into(),
                    kind: "min_width".into(),
                    message: "need width".into(),
                    element: None,
                    max: None,
                    min: Some(640),
                    value: None,
                    values: None,
                },
                PolicyRequirement {
                    id: "size.min-h".into(),
                    kind: "min_height".into(),
                    message: "need height".into(),
                    element: None,
                    max: None,
                    min: Some(360),
                    value: None,
                    values: None,
                },
            ],
            ..Default::default()
        };
        let split = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"1280\" height=\"180\"><![CDATA[https://cdn.example.com/a.mp4]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"320\" height=\"720\"><![CDATA[https://cdn.example.com/b.mp4]]></MediaFile>",
        );
        let mut miss = Vec::new();
        apply_overlay(&overlay, &split, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "size.min-w"));
        assert!(miss.iter().any(|f| f.requirement_id == "size.min-h"));
        let together = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"320\" height=\"240\"><![CDATA[https://cdn.example.com/a.mp4]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
        );
        let mut ok = Vec::new();
        apply_overlay(&overlay, &together, &mut ok);
        assert!(!ok.iter().any(|f| f.requirement_id == "size.min-w"));
        assert!(!ok.iter().any(|f| f.requirement_id == "size.min-h"));
    }

    #[test]
    fn max_width_and_height_require_one_file() {
        let overlay = PolicyOverlay {
            id: "size".into(),
            requirements: vec![
                PolicyRequirement {
                    id: "size.max-w".into(),
                    kind: "max_width".into(),
                    message: "need width cap".into(),
                    element: None,
                    max: Some(640),
                    min: None,
                    value: None,
                    values: None,
                },
                PolicyRequirement {
                    id: "size.max-h".into(),
                    kind: "max_height".into(),
                    message: "need height cap".into(),
                    element: None,
                    max: Some(360),
                    min: None,
                    value: None,
                    values: None,
                },
            ],
            ..Default::default()
        };
        let split = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"1280\" height=\"180\"><![CDATA[https://cdn.example.com/a.mp4]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"320\" height=\"720\"><![CDATA[https://cdn.example.com/b.mp4]]></MediaFile>",
        );
        let mut miss = Vec::new();
        apply_overlay(&overlay, &split, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "size.max-w"));
        assert!(miss.iter().any(|f| f.requirement_id == "size.max-h"));
        let together = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"1920\" height=\"1080\"><![CDATA[https://cdn.example.com/a.mp4]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
        );
        let mut ok = Vec::new();
        apply_overlay(&overlay, &together, &mut ok);
        assert!(!ok.iter().any(|f| f.requirement_id == "size.max-w"));
        assert!(!ok.iter().any(|f| f.requirement_id == "size.max-h"));
    }

    #[test]
    fn codec_and_audio_require_one_file() {
        let overlay = PolicyOverlay {
            id: "cod".into(),
            requirements: vec![
                PolicyRequirement {
                    id: "cod.avc".into(),
                    kind: "codec".into(),
                    message: "need avc".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: Some("avc".into()),
                    values: None,
                },
                PolicyRequirement {
                    id: "cod.aac".into(),
                    kind: "audio_codec".into(),
                    message: "need aac".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: Some("mp4a".into()),
                    values: None,
                },
            ],
            ..Default::default()
        };
        let split = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" codecs=\"avc1.42E01E\"><![CDATA[https://cdn.example.com/a.mp4]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" codecs=\"mp4a.40.2\"><![CDATA[https://cdn.example.com/b.mp4]]></MediaFile>",
        );
        let mut miss = Vec::new();
        apply_overlay(&overlay, &split, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "cod.avc"));
        assert!(miss.iter().any(|f| f.requirement_id == "cod.aac"));
        let together = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut ok = Vec::new();
        apply_overlay(&overlay, together, &mut ok);
        assert!(!ok.iter().any(|f| f.requirement_id == "cod.avc"));
        assert!(!ok.iter().any(|f| f.requirement_id == "cod.aac"));
    }

    #[test]
    fn min_bitrate_and_codec_require_one_file() {
        let overlay = PolicyOverlay {
            id: "join".into(),
            requirements: vec![
                PolicyRequirement {
                    id: "join.br".into(),
                    kind: "min_bitrate".into(),
                    message: "need bitrate".into(),
                    element: None,
                    max: None,
                    min: Some(1000),
                    value: None,
                    values: None,
                },
                PolicyRequirement {
                    id: "join.avc".into(),
                    kind: "codec".into(),
                    message: "need avc".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: Some("avc".into()),
                    values: None,
                },
            ],
            ..Default::default()
        };
        let split = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"vp09.00.10.08\"><![CDATA[https://cdn.example.com/a.mp4]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"200\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/b.mp4]]></MediaFile>",
        );
        let mut miss = Vec::new();
        apply_overlay(&overlay, &split, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "join.br"));
        assert!(miss.iter().any(|f| f.requirement_id == "join.avc"));
        let together = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut ok = Vec::new();
        apply_overlay(&overlay, together, &mut ok);
        assert!(!ok.iter().any(|f| f.requirement_id == "join.br"));
        assert!(!ok.iter().any(|f| f.requirement_id == "join.avc"));
    }

    #[test]
    fn min_width_and_min_bitrate_require_one_file() {
        let overlay = PolicyOverlay {
            id: "szbr".into(),
            requirements: vec![
                PolicyRequirement {
                    id: "szbr.w".into(),
                    kind: "min_width".into(),
                    message: "need width".into(),
                    element: None,
                    max: None,
                    min: Some(640),
                    value: None,
                    values: None,
                },
                PolicyRequirement {
                    id: "szbr.br".into(),
                    kind: "min_bitrate".into(),
                    message: "need bitrate".into(),
                    element: None,
                    max: None,
                    min: Some(1000),
                    value: None,
                    values: None,
                },
            ],
            ..Default::default()
        };
        let split = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"1280\" height=\"360\" bitrate=\"200\"><![CDATA[https://cdn.example.com/a.mp4]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"320\" height=\"180\" bitrate=\"1500\"><![CDATA[https://cdn.example.com/b.mp4]]></MediaFile>",
        );
        let mut miss = Vec::new();
        apply_overlay(&overlay, &split, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "szbr.w"));
        assert!(miss.iter().any(|f| f.requirement_id == "szbr.br"));
        let together = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut ok = Vec::new();
        apply_overlay(&overlay, together, &mut ok);
        assert!(!ok.iter().any(|f| f.requirement_id == "szbr.w"));
        assert!(!ok.iter().any(|f| f.requirement_id == "szbr.br"));
    }

    #[test]
    fn media_type_and_codec_require_one_file() {
        let overlay = PolicyOverlay {
            id: "mime".into(),
            requirements: vec![
                PolicyRequirement {
                    id: "mime.mp4".into(),
                    kind: "media_type".into(),
                    message: "need mp4".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: Some("video/mp4".into()),
                    values: None,
                },
                PolicyRequirement {
                    id: "mime.avc".into(),
                    kind: "codec".into(),
                    message: "need avc".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: Some("avc".into()),
                    values: None,
                },
            ],
            ..Default::default()
        };
        let split = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/webm\" width=\"640\" height=\"360\" codecs=\"avc1.42E01E\"><![CDATA[https://cdn.example.com/a.webm]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" codecs=\"vp09.00.10.08\"><![CDATA[https://cdn.example.com/b.mp4]]></MediaFile>",
        );
        let mut miss = Vec::new();
        apply_overlay(&overlay, &split, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "mime.mp4"));
        assert!(miss.iter().any(|f| f.requirement_id == "mime.avc"));
        let together = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut ok = Vec::new();
        apply_overlay(&overlay, together, &mut ok);
        assert!(!ok.iter().any(|f| f.requirement_id == "mime.mp4"));
        assert!(!ok.iter().any(|f| f.requirement_id == "mime.avc"));
    }

    #[test]
    fn aspect_and_min_width_require_one_file() {
        let overlay = PolicyOverlay {
            id: "geo".into(),
            requirements: vec![
                PolicyRequirement {
                    id: "geo.min-w".into(),
                    kind: "min_width".into(),
                    message: "need width".into(),
                    element: None,
                    max: None,
                    min: Some(640),
                    value: None,
                    values: None,
                },
                PolicyRequirement {
                    id: "geo.aspect".into(),
                    kind: "aspect_ratio".into(),
                    message: "need 16:9".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: Some("16:9".into()),
                    values: None,
                },
            ],
            ..Default::default()
        };
        let split = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"480\"><![CDATA[https://cdn.example.com/a.mp4]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"320\" height=\"180\"><![CDATA[https://cdn.example.com/b.mp4]]></MediaFile>",
        );
        let mut miss = Vec::new();
        apply_overlay(&overlay, &split, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "geo.min-w"));
        assert!(miss.iter().any(|f| f.requirement_id == "geo.aspect"));
        let together = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut ok = Vec::new();
        apply_overlay(&overlay, together, &mut ok);
        assert!(!ok.iter().any(|f| f.requirement_id == "geo.min-w"));
        assert!(!ok.iter().any(|f| f.requirement_id == "geo.aspect"));
    }

    #[test]
    fn media_type_and_container_require_one_file() {
        let conflict = PolicyOverlay {
            id: "box".into(),
            requirements: vec![
                PolicyRequirement {
                    id: "box.mime".into(),
                    kind: "media_type".into(),
                    message: "need webm".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: Some("video/webm".into()),
                    values: None,
                },
                PolicyRequirement {
                    id: "box.mp4".into(),
                    kind: "container".into(),
                    message: "need mp4".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: Some("mp4".into()),
                    values: None,
                },
            ],
            ..Default::default()
        };
        let split = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/webm\" width=\"640\" height=\"360\"><![CDATA[https://cdn.example.com/a.webm]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
        );
        let mut miss = Vec::new();
        apply_overlay(&conflict, &split, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "box.mime"));
        assert!(miss.iter().any(|f| f.requirement_id == "box.mp4"));
        let aligned = PolicyOverlay {
            id: "box".into(),
            requirements: vec![
                PolicyRequirement {
                    id: "box.mime".into(),
                    kind: "media_type".into(),
                    message: "need mp4".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: Some("video/mp4".into()),
                    values: None,
                },
                PolicyRequirement {
                    id: "box.mp4".into(),
                    kind: "container".into(),
                    message: "need mp4".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: Some("mp4".into()),
                    values: None,
                },
            ],
            ..Default::default()
        };
        let mut ok = Vec::new();
        apply_overlay(
            &aligned,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut ok,
        );
        assert!(!ok.iter().any(|f| f.requirement_id == "box.mime"));
        assert!(!ok.iter().any(|f| f.requirement_id == "box.mp4"));
    }

    #[test]
    fn media_type_and_delivery_require_one_file() {
        let overlay = PolicyOverlay {
            id: "join".into(),
            requirements: vec![
                PolicyRequirement {
                    id: "join.mp4".into(),
                    kind: "media_type".into(),
                    message: "need mp4".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: Some("video/mp4".into()),
                    values: None,
                },
                PolicyRequirement {
                    id: "join.prog".into(),
                    kind: "delivery".into(),
                    message: "need progressive".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: Some("progressive".into()),
                    values: None,
                },
            ],
            ..Default::default()
        };
        let split = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"streaming\" type=\"video/mp4\" width=\"640\" height=\"360\"><![CDATA[https://cdn.example.com/a.m3u8]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/webm\" width=\"640\" height=\"360\"><![CDATA[https://cdn.example.com/b.webm]]></MediaFile>",
        );
        let mut miss = Vec::new();
        apply_overlay(&overlay, &split, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "join.mp4"));
        assert!(miss.iter().any(|f| f.requirement_id == "join.prog"));
        let together = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut ok = Vec::new();
        apply_overlay(&overlay, together, &mut ok);
        assert!(!ok.iter().any(|f| f.requirement_id == "join.mp4"));
        assert!(!ok.iter().any(|f| f.requirement_id == "join.prog"));
    }

    #[test]
    fn privacy_macros_overlay_fails_without_timestamp() {
        let overlay = parse_overlay(include_str!(
            "../../../fixtures/overlays/privacy-macros.json"
        ))
        .unwrap();
        let miss =
            include_str!("../../../fixtures/ok-https-mp4.xml").replace("&ts=[TIMESTAMP]", "");
        let mut findings = Vec::new();
        apply_overlay(&overlay, &miss, &mut findings);
        assert!(findings
            .iter()
            .any(|f| f.requirement_id == "privacy.macros-required"));
        findings.clear();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/privacy-macros.xml"),
            &mut findings,
        );
        assert!(!findings
            .iter()
            .any(|f| f.requirement_id == "privacy.macros-required"));
    }

    #[test]
    fn require_macros_values_are_all_required() {
        let overlay = parse_overlay(include_str!(
            "../../../fixtures/overlays/privacy-macros.json"
        ))
        .unwrap();
        let miss =
            include_str!("../../../fixtures/ok-https-mp4.xml").replace("&ts=[TIMESTAMP]", "");
        let mut findings = Vec::new();
        apply_overlay(&overlay, &miss, &mut findings);
        assert!(findings
            .iter()
            .any(|f| f.requirement_id == "privacy.macros-required"));
    }

    #[test]
    fn news_overlay_blocks_declared_category() {
        let overlay =
            parse_overlay(include_str!("../../../fixtures/overlays/event-news.json")).unwrap();
        let mut findings = Vec::new();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/category-iab25.xml"),
            &mut findings,
        );
        assert!(findings
            .iter()
            .any(|f| f.requirement_id == "event-news.category-forbidden"));
    }

    #[test]
    fn leftover_empty_category_forbidden_values_are_dropped() {
        let overlay = PolicyOverlay {
            id: "news".into(),
            requirements: vec![PolicyRequirement {
                id: "news.category".into(),
                kind: "category_forbidden".into(),
                message: "no IAB25".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["IAB25".into(), "  ".into(), "".into()]),
            }],
            ..Default::default()
        };
        let mut hit = Vec::new();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/category-iab25.xml"),
            &mut hit,
        );
        assert!(hit.iter().any(|f| f.requirement_id == "news.category"));
        let blank = PolicyOverlay {
            id: "blank".into(),
            requirements: vec![PolicyRequirement {
                id: "blank.category".into(),
                kind: "category_forbidden".into(),
                message: "no category".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["  ".into(), "".into()]),
            }],
            ..Default::default()
        };
        let mut warn = Vec::new();
        apply_overlay(
            &blank,
            include_str!("../../../fixtures/category-iab25.xml"),
            &mut warn,
        );
        assert!(warn.iter().any(|f| {
            f.requirement_id == "policy.category-forbidden-missing-values"
                && f.severity == "warning"
        }));
        assert!(!warn.iter().any(|f| f.requirement_id == "blank.category"));
    }

    #[test]
    fn sports_overlay_fails_thirty_second_creative() {
        let overlay =
            parse_overlay(include_str!("../../../fixtures/overlays/event-sports.json")).unwrap();
        let mut findings = Vec::new();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut findings,
        );
        assert!(findings
            .iter()
            .any(|f| f.requirement_id == "event-sports.max-duration"));
    }

    #[test]
    fn unknown_kind_is_warning_not_panic() {
        let doc = PolicyDocument {
            id: "x".into(),
            version: "1".into(),
            requirements: vec![PolicyRequirement {
                id: "x.mystery".into(),
                kind: "not-a-real-kind".into(),
                message: "nope".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: None,
            }],
            ..Default::default()
        };
        let mut findings = Vec::new();
        apply_policy(
            &doc,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut findings,
        );
        assert!(findings
            .iter()
            .any(|f| f.requirement_id.contains("unknown-kind")));
    }

    #[test]
    fn select_overlays_deal_then_event_then_device() {
        let catalog: Vec<PolicyOverlay> =
            serde_json::from_str(include_str!("../../../fixtures/overlays/catalog.json")).unwrap();
        let sports = select_overlays(
            &catalog,
            &crate::Opportunity {
                deal_id: Some("deal-9".into()),
                publisher_id: Some("pub-1".into()),
                event_id: Some("game-1".into()),
                device_family: Some("ctv".into()),
                ..Default::default()
            },
        );
        let ids: Vec<_> = sports.iter().map(|o| o.id.as_str()).collect();
        assert!(ids.contains(&"deal-premium"));
        assert!(ids.contains(&"publisher-strict"));
        assert!(ids.contains(&"event-sports"));
        assert!(ids.contains(&"device-ctv"));
        assert!(!ids.contains(&"event-news"));
        let news = select_overlays(
            &catalog,
            &crate::Opportunity {
                event_id: Some("news-1".into()),
                ..Default::default()
            },
        );
        assert!(news.iter().any(|o| o.id == "event-news"));
        assert!(!news.iter().any(|o| o.id == "event-sports"));
        assert!(!news.iter().any(|o| o.id == "deal-premium"));
    }

    #[test]
    fn leftover_empty_deal_id_is_not_a_deal_key() {
        let leftover = PolicyOverlay {
            id: "leftover".into(),
            deal_id: Some("  ".into()),
            ..Default::default()
        };
        let deal = PolicyOverlay {
            id: "deal-premium".into(),
            deal_id: Some("deal-9".into()),
            ..Default::default()
        };
        let catalog = vec![leftover, deal];
        let none = select_overlays(&catalog, &crate::Opportunity::default());
        assert!(none.iter().any(|o| o.id == "leftover"));
        assert!(!none.iter().any(|o| o.id == "deal-premium"));
        let blank_opp = select_overlays(
            &catalog,
            &crate::Opportunity {
                deal_id: Some("".into()),
                ..Default::default()
            },
        );
        assert!(!blank_opp.iter().any(|o| o.id == "deal-premium"));
        let hit = select_overlays(
            &catalog,
            &crate::Opportunity {
                deal_id: Some("  deal-9 ".into()),
                ..Default::default()
            },
        );
        assert!(hit.iter().any(|o| o.id == "deal-premium"));
    }

    #[test]
    fn leftover_empty_event_id_is_not_an_event_key() {
        let leftover = PolicyOverlay {
            id: "leftover".into(),
            event_id: Some("  ".into()),
            ..Default::default()
        };
        let sports = PolicyOverlay {
            id: "event-sports".into(),
            event_id: Some("game-1".into()),
            ..Default::default()
        };
        let catalog = vec![leftover, sports];
        let none = select_overlays(&catalog, &crate::Opportunity::default());
        assert!(none.iter().any(|o| o.id == "leftover"));
        assert!(!none.iter().any(|o| o.id == "event-sports"));
        let blank_opp = select_overlays(
            &catalog,
            &crate::Opportunity {
                event_id: Some("".into()),
                ..Default::default()
            },
        );
        assert!(!blank_opp.iter().any(|o| o.id == "event-sports"));
        let hit = select_overlays(
            &catalog,
            &crate::Opportunity {
                event_id: Some("  game-1 ".into()),
                ..Default::default()
            },
        );
        let ids: Vec<_> = hit.iter().map(|o| o.id.as_str()).collect();
        assert_eq!(ids[0], "leftover");
        assert!(ids.contains(&"event-sports"));
    }

    #[test]
    fn leftover_empty_device_family_is_not_a_device_key() {
        let leftover = PolicyOverlay {
            id: "leftover".into(),
            device_family: Some("  ".into()),
            ..Default::default()
        };
        let ctv = PolicyOverlay {
            id: "device-ctv".into(),
            device_family: Some("ctv".into()),
            ..Default::default()
        };
        let catalog = vec![leftover, ctv];
        let none = select_overlays(&catalog, &crate::Opportunity::default());
        assert!(none.iter().any(|o| o.id == "leftover"));
        assert!(!none.iter().any(|o| o.id == "device-ctv"));
        let blank_opp = select_overlays(
            &catalog,
            &crate::Opportunity {
                device_family: Some("".into()),
                ..Default::default()
            },
        );
        assert!(!blank_opp.iter().any(|o| o.id == "device-ctv"));
        let hit = select_overlays(
            &catalog,
            &crate::Opportunity {
                device_family: Some("  CTV ".into()),
                ..Default::default()
            },
        );
        let ids: Vec<_> = hit.iter().map(|o| o.id.as_str()).collect();
        assert_eq!(ids[0], "leftover");
        assert!(ids.contains(&"device-ctv"));
    }

    #[test]
    fn leftover_empty_publisher_id_is_not_stamped() {
        let overlay = PolicyOverlay {
            id: "pub".into(),
            publisher_id: Some("  ".into()),
            requirements: vec![PolicyRequirement {
                id: "pub.viewable-required".into(),
                kind: "require_element".into(),
                message: "need viewable".into(),
                element: Some("ViewableImpression".into()),
                max: None,
                min: None,
                value: None,
                values: None,
            }],
            ..Default::default()
        };
        let mut findings = Vec::new();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut findings,
        );
        assert!(findings
            .iter()
            .any(|f| f.requirement_id == "pub.viewable-required"));
        assert!(findings.iter().all(|f| f.publisher.is_none()));
    }

    #[test]
    fn leftover_empty_require_element_name_is_warning() {
        let doc = PolicyDocument {
            id: "x".into(),
            version: "1".into(),
            requirements: vec![PolicyRequirement {
                id: "x.need-elem".into(),
                kind: "require_element".into(),
                message: "need an element".into(),
                element: Some("  ".into()),
                max: None,
                min: None,
                value: None,
                values: None,
            }],
            ..Default::default()
        };
        let mut findings = Vec::new();
        apply_policy(
            &doc,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut findings,
        );
        assert!(findings.iter().any(
            |f| f.requirement_id == "policy.require-element-missing-name"
                && f.severity == "warning"
        ));
        assert!(!findings.iter().any(|f| f.requirement_id == "x.need-elem"));
    }

    #[test]
    fn leftover_empty_tracking_event_names_are_dropped() {
        let overlay = PolicyOverlay {
            id: "events".into(),
            requirements: vec![PolicyRequirement {
                id: "events.quartiles".into(),
                kind: "require_tracking_events".into(),
                message: "need events".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec![
                    "start".into(),
                    "  ".into(),
                    "".into(),
                    "complete".into(),
                ]),
            }],
            ..Default::default()
        };
        let mut ok = Vec::new();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut ok,
        );
        assert!(!ok.iter().any(|f| f.requirement_id == "events.quartiles"));
        let blank = PolicyOverlay {
            id: "blank".into(),
            requirements: vec![PolicyRequirement {
                id: "blank.events".into(),
                kind: "require_tracking_events".into(),
                message: "need events".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["  ".into(), "".into()]),
            }],
            ..Default::default()
        };
        let mut warn = Vec::new();
        apply_overlay(
            &blank,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut warn,
        );
        assert!(warn.iter().any(|f| f.requirement_id
            == "policy.require-tracking-events-missing-names"
            && f.severity == "warning"));
        assert!(!warn.iter().any(|f| f.requirement_id == "blank.events"));
    }

    #[test]
    fn companion_tracking_does_not_satisfy_linear_events() {
        let overlay = PolicyOverlay {
            id: "events".into(),
            requirements: vec![PolicyRequirement {
                id: "events.start".into(),
                kind: "require_tracking_events".into(),
                message: "need start".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec!["start".into(), "complete".into()]),
            }],
            ..Default::default()
        };
        let xml = r#"<VAST version="4.2"><Ad><InLine>
<Impression><![CDATA[https://t.example.com/imp]]></Impression>
<Creatives><Creative>
<Linear>
<Duration>00:00:30</Duration>
<MediaFiles>
<MediaFile delivery="progressive" type="video/mp4" width="640" height="360" bitrate="1500"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>
</MediaFiles>
</Linear>
<CompanionAds>
<Companion width="300" height="250">
<TrackingEvents>
<Tracking event="start"><![CDATA[https://t.example.com/c]]></Tracking>
<Tracking event="complete"><![CDATA[https://t.example.com/c2]]></Tracking>
</TrackingEvents>
</Companion>
</CompanionAds>
</Creative></Creatives>
</InLine></Ad></VAST>"#;
        let mut miss = Vec::new();
        apply_overlay(&overlay, xml, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "events.start"));
        let mut ok = Vec::new();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut ok,
        );
        assert!(!ok.iter().any(|f| f.requirement_id == "events.start"));
    }

    #[test]
    fn closed_caption_overlay_requires_element() {
        let overlay = parse_overlay(include_str!(
            "../../../fixtures/overlays/closed-captions.json"
        ))
        .unwrap();
        let mut miss = Vec::new();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut miss,
        );
        assert!(miss
            .iter()
            .any(|f| f.requirement_id == "captions.closed-caption-file"));
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</MediaFiles>",
            "<ClosedCaptionFiles><ClosedCaptionFile type=\"text/vtt\"><![CDATA[https://cdn.example.com/ad.vtt]]></ClosedCaptionFile></ClosedCaptionFiles></MediaFiles>",
        );
        let mut ok = Vec::new();
        apply_overlay(&overlay, &xml, &mut ok);
        assert!(!ok
            .iter()
            .any(|f| f.requirement_id == "captions.closed-caption-file"));
    }

    #[test]
    fn caption_language_overlay_requires_matching_language() {
        let overlay = parse_overlay(include_str!(
            "../../../fixtures/overlays/caption-language.json"
        ))
        .unwrap();
        let mut miss = Vec::new();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut miss,
        );
        assert!(miss.iter().any(|f| f.requirement_id == "captions.language"));
        let no_lang = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</MediaFiles>",
            "<ClosedCaptionFiles><ClosedCaptionFile type=\"text/vtt\"><![CDATA[https://cdn.example.com/ad.vtt]]></ClosedCaptionFile></ClosedCaptionFiles></MediaFiles>",
        );
        miss.clear();
        apply_overlay(&overlay, &no_lang, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "captions.language"));
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</MediaFiles>",
            "<ClosedCaptionFiles><ClosedCaptionFile type=\"text/vtt\" language=\"es\"><![CDATA[https://cdn.example.com/es.vtt]]></ClosedCaptionFile><ClosedCaptionFile type=\"text/vtt\" language=\"en\"><![CDATA[https://cdn.example.com/ad.vtt]]></ClosedCaptionFile></ClosedCaptionFiles></MediaFiles>",
        );
        let mut ok = Vec::new();
        apply_overlay(&overlay, &xml, &mut ok);
        assert!(!ok.iter().any(|f| f.requirement_id == "captions.language"));
    }

    #[test]
    fn caption_type_and_id_registry_overlays_match_later_attrs() {
        let cap =
            parse_overlay(include_str!("../../../fixtures/overlays/caption-type.json")).unwrap();
        let mut miss = Vec::new();
        apply_overlay(
            &cap,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut miss,
        );
        assert!(miss.iter().any(|f| f.requirement_id == "captions.type"));
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</MediaFiles>",
            "<ClosedCaptionFiles><ClosedCaptionFile language=\"en\"><![CDATA[https://cdn.example.com/a.vtt]]></ClosedCaptionFile><ClosedCaptionFile type=\"text/vtt\" language=\"en\"><![CDATA[https://cdn.example.com/ad.vtt]]></ClosedCaptionFile></ClosedCaptionFiles></MediaFiles>",
        );
        let mut ok = Vec::new();
        apply_overlay(&cap, &xml, &mut ok);
        assert!(!ok.iter().any(|f| f.requirement_id == "captions.type"));
        let reg =
            parse_overlay(include_str!("../../../fixtures/overlays/id-registry.json")).unwrap();
        ok.clear();
        apply_overlay(
            &reg,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut ok,
        );
        assert!(!ok.iter().any(|f| f.requirement_id == "identity.registry"));
        let no_reg = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace(" idRegistry=\"ad-id.org\"", "");
        miss.clear();
        apply_overlay(&reg, &no_reg, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "identity.registry"));
    }

    #[test]
    fn icon_program_and_min_suggested_duration_overlays() {
        let icon =
            parse_overlay(include_str!("../../../fixtures/overlays/icon-program.json")).unwrap();
        let mut miss = Vec::new();
        apply_overlay(
            &icon,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut miss,
        );
        assert!(miss.iter().any(|f| f.requirement_id == "icons.program"));
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"other\" width=\"20\" height=\"20\"></Icon><Icon program=\"adChoices\" width=\"20\" height=\"20\"></Icon></Icons></Linear>",
        );
        let mut ok = Vec::new();
        apply_overlay(&icon, &xml, &mut ok);
        assert!(!ok.iter().any(|f| f.requirement_id == "icons.program"));
        let overlay = PolicyOverlay {
            id: "msd".into(),
            requirements: vec![PolicyRequirement {
                id: "msd.min".into(),
                kind: "min_suggested_duration_seconds".into(),
                message: "need suggested duration".into(),
                element: None,
                max: None,
                min: Some(6),
                value: None,
                values: None,
            }],
            ..Default::default()
        };
        ok.clear();
        apply_overlay(
            &overlay,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut ok,
        );
        assert!(!ok.iter().any(|f| f.requirement_id == "msd.min"));
        let short = r#"<VAST><Ad><InLine><Creative>
<NonLinearAds>
<NonLinear width="300" height="250" minSuggestedDuration="00:00:03"></NonLinear>
</NonLinearAds>
</Creative></InLine></Ad></VAST>"#;
        miss.clear();
        apply_overlay(&overlay, short, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "msd.min"));
    }

    #[test]
    fn ad_system_icon_timing_and_companion_width_overlays() {
        let sys = parse_overlay(include_str!(
            "../../../fixtures/overlays/ad-system-version.json"
        ))
        .unwrap();
        let mut miss = Vec::new();
        apply_overlay(
            &sys,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut miss,
        );
        assert!(miss.iter().any(|f| f.requirement_id == "adsystem.version"));
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<AdSystem>Test</AdSystem>",
            "<AdSystem version=\"1.0\">Test</AdSystem>",
        );
        let mut ok = Vec::new();
        apply_overlay(&sys, &xml, &mut ok);
        assert!(!ok.iter().any(|f| f.requirement_id == "adsystem.version"));
        let dur = PolicyOverlay {
            id: "icon".into(),
            requirements: vec![PolicyRequirement {
                id: "icon.dur".into(),
                kind: "min_icon_duration_seconds".into(),
                message: "need icon duration".into(),
                element: None,
                max: None,
                min: Some(6),
                value: None,
                values: None,
            }],
            ..Default::default()
        };
        ok.clear();
        apply_overlay(
            &dur,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut ok,
        );
        assert!(!ok.iter().any(|f| f.requirement_id == "icon.dur"));
        let short = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" duration=\"00:00:03\"></Icon></Icons></Linear>",
        );
        miss.clear();
        apply_overlay(&dur, &short, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "icon.dur"));
        let off = PolicyOverlay {
            id: "icon".into(),
            requirements: vec![PolicyRequirement {
                id: "icon.off".into(),
                kind: "max_icon_offset_seconds".into(),
                message: "icon offset too late".into(),
                element: None,
                max: Some(5),
                min: None,
                value: None,
                values: None,
            }],
            ..Default::default()
        };
        let late = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" offset=\"00:00:10\"></Icon></Icons></Linear>",
        );
        miss.clear();
        apply_overlay(&off, &late, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "icon.off"));
        let comp = parse_overlay(include_str!(
            "../../../fixtures/overlays/companion-width.json"
        ))
        .unwrap();
        miss.clear();
        apply_overlay(
            &comp,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut miss,
        );
        assert!(miss.iter().any(|f| f.requirement_id == "companion.width"));
        let wide = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"1\" height=\"1\"></Companion><Companion width=\"300\" height=\"250\"><StaticResource>https://cdn.example.com/c.png</StaticResource></Companion></CompanionAds></Creative>",
        );
        ok.clear();
        apply_overlay(&comp, &wide, &mut ok);
        assert!(!ok.iter().any(|f| f.requirement_id == "companion.width"));
        let join = PolicyOverlay {
            id: "comp".into(),
            requirements: vec![
                PolicyRequirement {
                    id: "companion.width".into(),
                    kind: "min_companion_width".into(),
                    message: "need companion width".into(),
                    element: None,
                    max: None,
                    min: Some(300),
                    value: None,
                    values: None,
                },
                PolicyRequirement {
                    id: "companion.height".into(),
                    kind: "min_companion_height".into(),
                    message: "need companion height".into(),
                    element: None,
                    max: None,
                    min: Some(250),
                    value: None,
                    values: None,
                },
            ],
            ..Default::default()
        };
        ok.clear();
        apply_overlay(&join, &wide, &mut ok);
        assert!(!ok
            .iter()
            .any(|f| f.requirement_id.starts_with("companion.")));
        let split = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"1\"></Companion><Companion width=\"1\" height=\"250\"></Companion></CompanionAds></Creative>",
        );
        miss.clear();
        apply_overlay(&join, &split, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "companion.width"));
        let pos = parse_overlay(include_str!(
            "../../../fixtures/overlays/icon-position.json"
        ))
        .unwrap();
        miss.clear();
        apply_overlay(
            &pos,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut miss,
        );
        assert!(miss.iter().any(|f| f.requirement_id == "icons.x"));
        let placed = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" xPosition=\"right\" yPosition=\"top\"></Icon></Icons></Linear>",
        );
        ok.clear();
        apply_overlay(&pos, &placed, &mut ok);
        assert!(!ok.iter().any(|f| f.requirement_id == "icons.x"));
        assert!(!ok.iter().any(|f| f.requirement_id == "icons.y"));
        let mid = parse_overlay(include_str!(
            "../../../fixtures/overlays/ad-serving-id-length.json"
        ))
        .unwrap();
        ok.clear();
        apply_overlay(
            &mid,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut ok,
        );
        assert!(!ok.iter().any(|f| f.requirement_id == "identity.mid-len"));
        let short_mid = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace(">srv-001</AdServingId>", ">ab</AdServingId>");
        miss.clear();
        apply_overlay(&mid, &short_mid, &mut miss);
        assert!(miss.iter().any(|f| f.requirement_id == "identity.mid-len"));
    }

    #[test]
    fn require_macros_trims_values_and_inner_whitespace() {
        let overlay = PolicyOverlay {
            id: "privacy".into(),
            requirements: vec![PolicyRequirement {
                id: "privacy.macros-required".into(),
                kind: "require_macros".into(),
                message: "need macros".into(),
                element: None,
                max: None,
                min: None,
                value: None,
                values: Some(vec![
                    " CACHEBUSTING ".into(),
                    "   ".into(),
                    "TIMESTAMP".into(),
                ]),
            }],
            ..Default::default()
        };
        let padded = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("[CACHEBUSTING]", "[ CACHEBUSTING ]")
            .replace("[TIMESTAMP]", "[ TIMESTAMP ]");
        let mut ok = Vec::new();
        apply_overlay(&overlay, &padded, &mut ok);
        assert!(!ok
            .iter()
            .any(|f| f.requirement_id == "privacy.macros-required"));
        let title_only = include_str!("../../../fixtures/ok-https-mp4.xml")
            .replace("[CACHEBUSTING]", "")
            .replace("[TIMESTAMP]", "")
            .replace(
                "<AdTitle>Test</AdTitle>",
                "<AdTitle>[CACHEBUSTING] [TIMESTAMP]</AdTitle>",
            );
        let mut miss = Vec::new();
        apply_overlay(&overlay, &title_only, &mut miss);
        assert!(miss
            .iter()
            .any(|f| f.requirement_id == "privacy.macros-required"));
    }

    #[test]
    fn parse_overlays_accepts_catalog_or_object() {
        let catalog =
            parse_overlays(include_str!("../../../fixtures/overlays/catalog.json")).unwrap();
        assert!(catalog.iter().any(|o| o.id == "event-sports"));
        let one =
            parse_overlays(include_str!("../../../fixtures/overlays/event-sports.json")).unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].id, "event-sports");
    }

    #[test]
    fn device_family_fails_closed_without_device() {
        let overlay =
            parse_overlay(include_str!("../../../fixtures/overlays/device-ctv.json")).unwrap();
        let mut miss = Vec::new();
        apply_overlay_for(
            &overlay,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            None,
            &mut miss,
        );
        assert!(miss.iter().any(|f| f.requirement_id == "device-ctv.family"));
        let mut hit = Vec::new();
        apply_overlay_for(
            &overlay,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            Some("ctv"),
            &mut hit,
        );
        assert!(!hit.iter().any(|f| f.requirement_id == "device-ctv.family"));
    }

    #[test]
    fn min_bitrate_fails_closed_without_bitrate() {
        let doc = PolicyDocument {
            id: "x".into(),
            version: "1".into(),
            requirements: vec![PolicyRequirement {
                id: "x.min-bitrate".into(),
                kind: "min_bitrate".into(),
                message: "need bitrate".into(),
                element: None,
                max: None,
                min: Some(1000),
                value: None,
                values: None,
            }],
            ..Default::default()
        };
        let mut findings = Vec::new();
        apply_policy(
            &doc,
            include_str!("../../../fixtures/wrapper.xml"),
            &mut findings,
        );
        assert!(findings.iter().any(|f| f.requirement_id == "x.min-bitrate"));
    }

    #[test]
    fn min_bitrate_reads_spaced_attr_via_inspect() {
        let overlay = PolicyOverlay {
            id: "br".into(),
            requirements: vec![PolicyRequirement {
                id: "br.min".into(),
                kind: "min_bitrate".into(),
                message: "need bitrate".into(),
                element: None,
                max: None,
                min: Some(1000),
                value: None,
                values: None,
            }],
            ..Default::default()
        };
        let xml = r#"<VAST version="4.2"><Ad><InLine>
<Creative><Linear>
<Duration>00:00:15</Duration>
<MediaFiles>
<MediaFile delivery="progressive" type="video/mp4" width="640" height="360" bitrate = "4000"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>
</MediaFiles>
</Linear></Creative>
</InLine></Ad></VAST>"#;
        let mut findings = Vec::new();
        apply_overlay(&overlay, xml, &mut findings);
        assert!(!findings.iter().any(|f| f.requirement_id == "br.min"));
    }

    #[test]
    fn live_packs_w10_media_floor_passes_happy_xml() {
        for pack in ["live-ssai", "live-sgai", "live-csai", "live-hybrid"] {
            let doc = discover_policy(pack).unwrap();
            for kind in [
                "min_bitrate",
                "max_bitrate",
                "min_width",
                "min_height",
                "aspect_ratio",
                "min_duration_seconds",
            ] {
                assert!(
                    doc.requirements.iter().any(|r| r.kind == kind),
                    "{pack} missing {kind}"
                );
            }
            assert!(
                doc.requirements.iter().any(|r| r.kind == "require_element"
                    && r.element.as_deref() == Some("UniversalAdId")),
                "{pack} missing UniversalAdId"
            );
            assert!(
                doc.requirements
                    .iter()
                    .any(|r| r.kind == "require_element"
                        && r.element.as_deref() == Some("AdServingId")),
                "{pack} missing AdServingId"
            );
            assert_eq!(
                doc.unknown_creative_treatment.as_deref(),
                Some("informational"),
                "{pack}"
            );
            assert!(
                doc.requirements.iter().any(|r| r.kind == "require_macros"
                    && r.values
                        .as_ref()
                        .is_some_and(|v| v.iter().any(|m| m == "TIMESTAMP"))),
                "{pack} missing TIMESTAMP"
            );
            assert_eq!(doc.invalidation_rules.len(), 4, "{pack}");
            assert!(
                doc.requirements
                    .iter()
                    .any(|r| r.kind == "min_duration_seconds" && r.min == Some(6)),
                "{pack}"
            );
            assert!(
                doc.requirements
                    .iter()
                    .any(|r| r.kind == "min_bitrate" && r.min == Some(1000)),
                "{pack}"
            );
            assert!(
                doc.requirements
                    .iter()
                    .any(|r| r.kind == "max_bitrate" && r.max == Some(20000)),
                "{pack}"
            );
            let xml = if pack == "live-ssai" || pack == "live-hybrid" {
                include_str!("../../../fixtures/ok-mezzanine.xml")
            } else {
                include_str!("../../../fixtures/ok-https-mp4.xml")
            };
            let mut findings = Vec::new();
            apply_policy(&doc, xml, &mut findings);
            assert!(
                !findings
                    .iter()
                    .any(|f| f.requirement_id.ends_with(".min-bitrate")
                        || f.requirement_id.ends_with(".max-bitrate")
                        || f.requirement_id.ends_with(".min-width")
                        || f.requirement_id.ends_with(".min-height")
                        || f.requirement_id.ends_with(".aspect")
                        || f.requirement_id.ends_with(".min-duration")),
                "{pack} {findings:?}"
            );
        }
    }

    #[test]
    fn max_bitrate_fails_closed_without_bitrate() {
        let doc = PolicyDocument {
            id: "x".into(),
            version: "1".into(),
            requirements: vec![PolicyRequirement {
                id: "x.max-bitrate".into(),
                kind: "max_bitrate".into(),
                message: "cap bitrate".into(),
                element: None,
                max: Some(20000),
                min: None,
                value: None,
                values: None,
            }],
            ..Default::default()
        };
        let mut findings = Vec::new();
        apply_policy(
            &doc,
            include_str!("../../../fixtures/wrapper.xml"),
            &mut findings,
        );
        assert!(findings.iter().any(|f| f.requirement_id == "x.max-bitrate"));
    }

    #[test]
    fn max_bitrate_rejects_over_cap() {
        let doc = PolicyDocument {
            id: "x".into(),
            version: "1".into(),
            requirements: vec![PolicyRequirement {
                id: "x.max-bitrate".into(),
                kind: "max_bitrate".into(),
                message: "cap bitrate".into(),
                element: None,
                max: Some(1000),
                min: None,
                value: None,
                values: None,
            }],
            ..Default::default()
        };
        let mut findings = Vec::new();
        apply_policy(
            &doc,
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &mut findings,
        );
        assert!(findings.iter().any(|f| f.requirement_id == "x.max-bitrate"));
    }

    #[test]
    fn min_duration_fails_closed_without_duration() {
        let doc = PolicyDocument {
            id: "x".into(),
            version: "1".into(),
            requirements: vec![PolicyRequirement {
                id: "x.min-duration".into(),
                kind: "min_duration_seconds".into(),
                message: "need duration".into(),
                element: None,
                max: None,
                min: Some(6),
                value: None,
                values: None,
            }],
            ..Default::default()
        };
        let mut findings = Vec::new();
        apply_policy(
            &doc,
            include_str!("../../../fixtures/wrapper.xml"),
            &mut findings,
        );
        assert!(findings
            .iter()
            .any(|f| f.requirement_id == "x.min-duration"));
    }

    #[test]
    fn min_duration_rejects_five_seconds() {
        let doc = discover_policy("live-sgai").unwrap();
        let xml =
            include_str!("../../../fixtures/ok-https-mp4.xml").replace("00:00:30", "00:00:05");
        let mut findings = Vec::new();
        apply_policy(&doc, &xml, &mut findings);
        assert!(findings
            .iter()
            .any(|f| f.requirement_id == "live-sgai.min-duration"));
    }

    #[test]
    fn expired_pack_is_expired() {
        let p = parse_policy(include_str!("../../../fixtures/expired-pack.json")).unwrap();
        let now = time::OffsetDateTime::parse(
            "2026-09-20T00:00:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap();
        assert!(pack_expired(&p, now));
    }

    #[test]
    fn unparseable_expires_at_fails_closed() {
        let mut p = parse_policy(include_str!("../../../fixtures/expired-pack.json")).unwrap();
        p.expires_at = Some("not-a-date".into());
        let now = time::OffsetDateTime::now_utc();
        assert!(pack_expired(&p, now));
    }

    #[test]
    fn media_https_fails_closed_without_mediafile() {
        let doc = discover_policy("live-csai").unwrap();
        let mut findings = Vec::new();
        apply_policy(
            &doc,
            include_str!("../../../fixtures/wrapper.xml"),
            &mut findings,
        );
        assert!(findings
            .iter()
            .any(|f| f.requirement_id == "live-csai.media-https"));
    }

    #[test]
    fn media_https_rejects_ftp_and_protocol_relative() {
        let doc = discover_policy("live-csai").unwrap();
        for url in ["ftp://cdn.example.com/ad.mp4", "//cdn.example.com/ad.mp4"] {
            let xml = include_str!("../../../fixtures/ok-https-mp4.xml")
                .replace("https://cdn.example.com/ad.mp4", url);
            let mut findings = Vec::new();
            apply_policy(&doc, &xml, &mut findings);
            assert!(
                findings
                    .iter()
                    .any(|f| f.requirement_id == "live-csai.media-https"),
                "{url}"
            );
        }
    }

    #[test]
    fn media_https_passes_when_a_later_file_is_https() {
        let doc = discover_policy("live-csai").unwrap();
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml").replace(
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
            "<MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\"><![CDATA[http://cdn.example.com/a.mp4]]></MediaFile><MediaFile delivery=\"progressive\" type=\"video/mp4\" width=\"640\" height=\"360\" bitrate=\"1500\" codecs=\"avc1.42E01E,mp4a.40.2\"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>",
        );
        assert!(xml.contains("http://cdn.example.com/a.mp4"));
        assert!(xml.contains("https://cdn.example.com/ad.mp4"));
        let mut findings = Vec::new();
        apply_policy(&doc, &xml, &mut findings);
        assert!(
            !findings
                .iter()
                .any(|f| f.requirement_id == "live-csai.media-https"),
            "{findings:?}"
        );
    }

    #[test]
    fn media_https_passes_when_a_later_mezzanine_is_https() {
        let doc = discover_policy("live-ssai").unwrap();
        let xml = include_str!("../../../fixtures/ok-mezzanine.xml").replace(
            "<Mezzanine delivery=\"progressive\" type=\"video/mp4\" width=\"1920\" height=\"1080\"><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
            "<Mezzanine delivery=\"progressive\" type=\"video/mp4\" width=\"1920\" height=\"1080\"><![CDATA[http://cdn.example.com/a.mp4]]></Mezzanine><Mezzanine delivery=\"progressive\" type=\"video/mp4\" width=\"1920\" height=\"1080\"><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        assert!(xml.contains("http://cdn.example.com/a.mp4"));
        assert!(xml.contains("https://cdn.example.com/mezz.mp4"));
        let mut findings = Vec::new();
        apply_policy(&doc, &xml, &mut findings);
        assert!(
            !findings
                .iter()
                .any(|f| f.requirement_id == "live-ssai.media-https"),
            "{findings:?}"
        );
    }

    #[test]
    fn vast_version_sequence_and_expires() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "vast.ver".into(),
                        kind: "vast_version".into(),
                        message: "need 4.2".into(),
                        element: None,
                        max: None,
                        min: None,
                        value: None,
                        values: Some(vec!["4.2".into()]),
                    },
                    PolicyRequirement {
                        id: "exp.max".into(),
                        kind: "max_expires_seconds".into(),
                        message: "cache bound".into(),
                        element: None,
                        max: Some(7200),
                        min: None,
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "exp.min".into(),
                        kind: "min_expires_seconds".into(),
                        message: "cache floor".into(),
                        element: None,
                        max: None,
                        min: Some(1800),
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            xml,
            &mut ok,
        );
        assert!(!ok.iter().any(|f| f.requirement_id == "vast.ver"));
        assert!(!ok.iter().any(|f| f.requirement_id == "exp.max"));
        assert!(!ok.iter().any(|f| f.requirement_id == "exp.min"));

        let mut miss = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cr.seq".into(),
                    kind: "creative_sequence".into(),
                    message: "need seq".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut miss,
        );
        assert!(miss.iter().any(|f| f.requirement_id == "cr.seq"));

        let seq = xml.replace("<Creative>", "<Creative sequence=\"1\">");
        let mut seq_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cr.seq".into(),
                    kind: "creative_sequence".into(),
                    message: "need seq".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1".into()]),
                }],
                ..Default::default()
            },
            &seq,
            &mut seq_ok,
        );
        assert!(!seq_ok.iter().any(|f| f.requirement_id == "cr.seq"));

        let late = xml.replace("</Expires>", "</Expires><Expires>86400</Expires>");
        let mut late_f = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "exp.max".into(),
                    kind: "max_expires_seconds".into(),
                    message: "cache bound".into(),
                    element: None,
                    max: Some(7200),
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &late,
            &mut late_f,
        );
        assert!(late_f.iter().any(|f| f.requirement_id == "exp.max"));
    }

    #[test]
    fn ad_sequence_wrapper_https_vendor_and_pricing() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut inline_https = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "wrap.https".into(),
                    kind: "wrapper_https".into(),
                    message: "need https hop".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut inline_https,
        );
        assert!(!inline_https
            .iter()
            .any(|f| f.requirement_id == "wrap.https"));

        let mut wrap_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "wrap.https".into(),
                    kind: "wrapper_https".into(),
                    message: "need https hop".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            include_str!("../../../fixtures/wrapper.xml"),
            &mut wrap_ok,
        );
        assert!(!wrap_ok.iter().any(|f| f.requirement_id == "wrap.https"));

        let http = include_str!("../../../fixtures/wrapper.xml").replace(
            "https://ad.example.com/vast.xml",
            "http://ad.example.com/vast.xml",
        );
        let mut wrap_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "wrap.https".into(),
                    kind: "wrapper_https".into(),
                    message: "need https hop".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http,
            &mut wrap_fail,
        );
        assert!(wrap_fail.iter().any(|f| f.requirement_id == "wrap.https"));

        let mut ad_miss = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ad.seq".into(),
                    kind: "ad_sequence".into(),
                    message: "need seq".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut ad_miss,
        );
        assert!(ad_miss.iter().any(|f| f.requirement_id == "ad.seq"));

        let ad = xml.replace("<Ad id=\"1\">", "<Ad id=\"1\" sequence=\"1\">");
        let mut ad_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ad.seq".into(),
                    kind: "ad_sequence".into(),
                    message: "need seq".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1".into()]),
                }],
                ..Default::default()
            },
            &ad,
            &mut ad_ok,
        );
        assert!(!ad_ok.iter().any(|f| f.requirement_id == "ad.seq"));

        let vendor = xml.replace(
            "</InLine>",
            "<AdVerifications><Verification vendor=\"iabtechlab.com-omid\"></Verification></AdVerifications></InLine>",
        );
        let mut vendor_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ver.vendor".into(),
                    kind: "verification_vendor".into(),
                    message: "need omid".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["iabtechlab.com-omid".into()]),
                }],
                ..Default::default()
            },
            &vendor,
            &mut vendor_ok,
        );
        assert!(!vendor_ok.iter().any(|f| f.requirement_id == "ver.vendor"));

        let price = xml.replace(
            "</AdServingId>",
            "</AdServingId><Pricing model=\"CPM\" currency=\"USD\"><![CDATA[2.50]]></Pricing>",
        );
        let mut price_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "price.model".into(),
                    kind: "pricing_model".into(),
                    message: "need cpm".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["CPM".into()]),
                }],
                ..Default::default()
            },
            &price,
            &mut price_ok,
        );
        assert!(!price_ok.iter().any(|f| f.requirement_id == "price.model"));
    }

    #[test]
    fn pricing_currency_error_https_and_viewable_https() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let price = xml.replace(
            "</AdServingId>",
            "</AdServingId><Pricing model=\"CPM\" currency=\"USD\"><![CDATA[2.50]]></Pricing>",
        );
        let mut ccy_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "price.ccy".into(),
                    kind: "pricing_currency".into(),
                    message: "need usd".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["USD".into()]),
                }],
                ..Default::default()
            },
            &price,
            &mut ccy_ok,
        );
        assert!(!ccy_ok.iter().any(|f| f.requirement_id == "price.ccy"));

        let mut err_miss = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "err.https".into(),
                    kind: "error_https".into(),
                    message: "need https error".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut err_miss,
        );
        assert!(err_miss.iter().any(|f| f.requirement_id == "err.https"));

        let err = xml.replace(
            "</Impression>",
            "</Impression><Error><![CDATA[https://t.example.com/e]]></Error>",
        );
        let mut err_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "err.https".into(),
                    kind: "error_https".into(),
                    message: "need https error".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &err,
            &mut err_ok,
        );
        assert!(!err_ok.iter().any(|f| f.requirement_id == "err.https"));

        let mut view_miss = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "view.https".into(),
                    kind: "viewable_https".into(),
                    message: "need viewable".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut view_miss,
        );
        assert!(view_miss.iter().any(|f| f.requirement_id == "view.https"));

        let view = xml.replace(
            "</Impression>",
            "</Impression><ViewableImpression><Viewable><![CDATA[https://t.example.com/v]]></Viewable></ViewableImpression>",
        );
        let mut view_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "view.https".into(),
                    kind: "viewable_https".into(),
                    message: "need viewable".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &view,
            &mut view_ok,
        );
        assert!(!view_ok.iter().any(|f| f.requirement_id == "view.https"));
    }

    #[test]
    fn impression_clickthrough_and_survey_https() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut imp_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "imp.https".into(),
                    kind: "impression_https".into(),
                    message: "need https impression".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut imp_ok,
        );
        assert!(!imp_ok.iter().any(|f| f.requirement_id == "imp.https"));

        let http_imp = xml.replace("https://t.example.com/imp", "http://t.example.com/imp");
        let mut imp_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "imp.https".into(),
                    kind: "impression_https".into(),
                    message: "need https impression".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_imp,
            &mut imp_fail,
        );
        assert!(imp_fail.iter().any(|f| f.requirement_id == "imp.https"));

        let mut clk_miss = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "clk.https".into(),
                    kind: "clickthrough_https".into(),
                    message: "need https click".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut clk_miss,
        );
        assert!(clk_miss.iter().any(|f| f.requirement_id == "clk.https"));

        let clk = xml.replace(
            "</Duration>",
            "</Duration><VideoClicks><ClickThrough><![CDATA[https://adv.example.com/l]]></ClickThrough></VideoClicks>",
        );
        let mut clk_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "clk.https".into(),
                    kind: "clickthrough_https".into(),
                    message: "need https click".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &clk,
            &mut clk_ok,
        );
        assert!(!clk_ok.iter().any(|f| f.requirement_id == "clk.https"));

        let mut survey_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "survey.https".into(),
                    kind: "survey_https".into(),
                    message: "survey https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut survey_ok,
        );
        assert!(!survey_ok.iter().any(|f| f.requirement_id == "survey.https"));

        let survey_http = xml.replace(
            "</Impression>",
            "</Impression><Survey><![CDATA[http://t.example.com/s]]></Survey>",
        );
        let mut survey_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "survey.https".into(),
                    kind: "survey_https".into(),
                    message: "survey https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &survey_http,
            &mut survey_fail,
        );
        assert!(survey_fail
            .iter()
            .any(|f| f.requirement_id == "survey.https"));
    }

    #[test]
    fn clicktracking_icon_clickthrough_and_category_authority() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut ct_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ct.https".into(),
                    kind: "clicktracking_https".into(),
                    message: "click tracking https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut ct_ok,
        );
        assert!(!ct_ok.iter().any(|f| f.requirement_id == "ct.https"));

        let http_ct = xml.replace(
            "</Duration>",
            "</Duration><VideoClicks><ClickTracking><![CDATA[http://t.example.com/c]]></ClickTracking></VideoClicks>",
        );
        let mut ct_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ct.https".into(),
                    kind: "clicktracking_https".into(),
                    message: "click tracking https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_ct,
            &mut ct_fail,
        );
        assert!(ct_fail.iter().any(|f| f.requirement_id == "ct.https"));

        let http_cc = xml.replace(
            "</Duration>",
            "</Duration><VideoClicks><CustomClick><![CDATA[http://t.example.com/custom]]></CustomClick></VideoClicks>",
        );
        let mut cc_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ct.https".into(),
                    kind: "clicktracking_https".into(),
                    message: "click tracking https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_cc,
            &mut cc_fail,
        );
        assert!(cc_fail.iter().any(|f| f.requirement_id == "ct.https"));

        let mut icon_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icon.clk".into(),
                    kind: "icon_clickthrough_https".into(),
                    message: "icon click https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut icon_ok,
        );
        assert!(!icon_ok.iter().any(|f| f.requirement_id == "icon.clk"));

        let http_icon = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconClickThrough><![CDATA[http://adv.example.com/icon]]></IconClickThrough></Icon></Icons></Linear>",
        );
        let mut icon_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icon.clk".into(),
                    kind: "icon_clickthrough_https".into(),
                    message: "icon click https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_icon,
            &mut icon_fail,
        );
        assert!(icon_fail.iter().any(|f| f.requirement_id == "icon.clk"));

        let mut cat_miss = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cat.auth".into(),
                    kind: "category_authority".into(),
                    message: "need iab authority".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["IAB".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut cat_miss,
        );
        assert!(cat_miss.iter().any(|f| f.requirement_id == "cat.auth"));

        let cat = xml.replace(
            "</AdTitle>",
            "</AdTitle><Category authority=\"IAB\">IAB1-1</Category>",
        );
        let mut cat_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cat.auth".into(),
                    kind: "category_authority".into(),
                    message: "need iab authority".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["IAB".into()]),
                }],
                ..Default::default()
            },
            &cat,
            &mut cat_ok,
        );
        assert!(!cat_ok.iter().any(|f| f.requirement_id == "cat.auth"));
    }

    #[test]
    fn javascript_resource_companion_clickthrough_and_advertiser() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut js_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "js.https".into(),
                    kind: "javascript_resource_https".into(),
                    message: "js https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut js_ok,
        );
        assert!(!js_ok.iter().any(|f| f.requirement_id == "js.https"));

        let http_js = xml.replace(
            "</InLine>",
            "<AdVerifications><Verification><JavaScriptResource><![CDATA[http://t.example.com/omid.js]]></JavaScriptResource></Verification></AdVerifications></InLine>",
        );
        let mut js_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "js.https".into(),
                    kind: "javascript_resource_https".into(),
                    message: "js https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_js,
            &mut js_fail,
        );
        assert!(js_fail.iter().any(|f| f.requirement_id == "js.https"));

        let mut comp_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.clk".into(),
                    kind: "companion_clickthrough_https".into(),
                    message: "companion click https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut comp_ok,
        );
        assert!(!comp_ok.iter().any(|f| f.requirement_id == "comp.clk"));

        let http_comp = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><CompanionClickThrough><![CDATA[http://adv.example.com/c]]></CompanionClickThrough></Companion></CompanionAds></Creative>",
        );
        let mut comp_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.clk".into(),
                    kind: "companion_clickthrough_https".into(),
                    message: "companion click https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_comp,
            &mut comp_fail,
        );
        assert!(comp_fail.iter().any(|f| f.requirement_id == "comp.clk"));

        let mut adv_miss = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "adv.name".into(),
                    kind: "advertiser".into(),
                    message: "need acme".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["Acme".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut adv_miss,
        );
        assert!(adv_miss.iter().any(|f| f.requirement_id == "adv.name"));

        let adv = xml.replace("</AdTitle>", "</AdTitle><Advertiser>Acme</Advertiser>");
        let mut adv_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "adv.name".into(),
                    kind: "advertiser".into(),
                    message: "need acme".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["Acme".into()]),
                }],
                ..Default::default()
            },
            &adv,
            &mut adv_ok,
        );
        assert!(!adv_ok.iter().any(|f| f.requirement_id == "adv.name"));
    }

    #[test]
    fn executable_resource_nonlinear_clickthrough_and_description() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut ex_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ex.https".into(),
                    kind: "executable_resource_https".into(),
                    message: "exe https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut ex_ok,
        );
        assert!(!ex_ok.iter().any(|f| f.requirement_id == "ex.https"));

        let http_ex = xml.replace(
            "</InLine>",
            "<AdVerifications><Verification><ExecutableResource><![CDATA[http://t.example.com/omid.exe]]></ExecutableResource></Verification></AdVerifications></InLine>",
        );
        let mut ex_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ex.https".into(),
                    kind: "executable_resource_https".into(),
                    message: "exe https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_ex,
            &mut ex_fail,
        );
        assert!(ex_fail.iter().any(|f| f.requirement_id == "ex.https"));

        let mut nl_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.clk".into(),
                    kind: "nonlinear_clickthrough_https".into(),
                    message: "nl click https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut nl_ok,
        );
        assert!(!nl_ok.iter().any(|f| f.requirement_id == "nl.clk"));

        let http_nl = xml.replace(
            "</Creative>",
            "<NonLinearAds><NonLinear width=\"300\" height=\"250\"><NonLinearClickThrough><![CDATA[http://adv.example.com/nl]]></NonLinearClickThrough></NonLinear></NonLinearAds></Creative>",
        );
        let mut nl_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.clk".into(),
                    kind: "nonlinear_clickthrough_https".into(),
                    message: "nl click https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_nl,
            &mut nl_fail,
        );
        assert!(nl_fail.iter().any(|f| f.requirement_id == "nl.clk"));

        let mut desc_miss = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "desc.len".into(),
                    kind: "min_description_length".into(),
                    message: "need description".into(),
                    element: None,
                    max: None,
                    min: Some(8),
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut desc_miss,
        );
        assert!(desc_miss.iter().any(|f| f.requirement_id == "desc.len"));

        let desc = xml.replace(
            "</AdTitle>",
            "</AdTitle><Description>campaign copy here</Description>",
        );
        let mut desc_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "desc.len".into(),
                    kind: "min_description_length".into(),
                    message: "need description".into(),
                    element: None,
                    max: None,
                    min: Some(8),
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &desc,
            &mut desc_ok,
        );
        assert!(!desc_ok.iter().any(|f| f.requirement_id == "desc.len"));
    }

    #[test]
    fn static_iframe_https_and_ad_title_length() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut static_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "static.https".into(),
                    kind: "static_resource_https".into(),
                    message: "static https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut static_ok,
        );
        assert!(!static_ok.iter().any(|f| f.requirement_id == "static.https"));

        let http_static = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><StaticResource creativeType=\"image/png\"><![CDATA[http://cdn.example.com/c.png]]></StaticResource></Companion></CompanionAds></Creative>",
        );
        let mut static_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "static.https".into(),
                    kind: "static_resource_https".into(),
                    message: "static https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_static,
            &mut static_fail,
        );
        assert!(static_fail
            .iter()
            .any(|f| f.requirement_id == "static.https"));

        let mut iframe_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "iframe.https".into(),
                    kind: "iframe_resource_https".into(),
                    message: "iframe https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut iframe_ok,
        );
        assert!(!iframe_ok.iter().any(|f| f.requirement_id == "iframe.https"));

        let http_iframe = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><IFrameResource><![CDATA[http://cdn.example.com/frame.html]]></IFrameResource></Companion></CompanionAds></Creative>",
        );
        let mut iframe_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "iframe.https".into(),
                    kind: "iframe_resource_https".into(),
                    message: "iframe https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_iframe,
            &mut iframe_fail,
        );
        assert!(iframe_fail
            .iter()
            .any(|f| f.requirement_id == "iframe.https"));

        let mut title_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "title.len".into(),
                    kind: "min_ad_title_length".into(),
                    message: "need title".into(),
                    element: None,
                    max: None,
                    min: Some(3),
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut title_ok,
        );
        assert!(!title_ok.iter().any(|f| f.requirement_id == "title.len"));

        let mut title_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "title.len".into(),
                    kind: "min_ad_title_length".into(),
                    message: "need title".into(),
                    element: None,
                    max: None,
                    min: Some(8),
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut title_fail,
        );
        assert!(title_fail.iter().any(|f| f.requirement_id == "title.len"));
    }

    #[test]
    fn html_flash_https_and_ad_parameters_length() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut html_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "html.https".into(),
                    kind: "html_resource_https".into(),
                    message: "html https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut html_ok,
        );
        assert!(!html_ok.iter().any(|f| f.requirement_id == "html.https"));

        let http_html = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><HTMLResource><![CDATA[http://cdn.example.com/ad.html]]></HTMLResource></Companion></CompanionAds></Creative>",
        );
        let mut html_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "html.https".into(),
                    kind: "html_resource_https".into(),
                    message: "html https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_html,
            &mut html_fail,
        );
        assert!(html_fail.iter().any(|f| f.requirement_id == "html.https"));

        let mut flash_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "flash.https".into(),
                    kind: "flash_resource_https".into(),
                    message: "flash https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut flash_ok,
        );
        assert!(!flash_ok.iter().any(|f| f.requirement_id == "flash.https"));

        let http_flash = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><FlashResource><![CDATA[http://cdn.example.com/ad.swf]]></FlashResource></Companion></CompanionAds></Creative>",
        );
        let mut flash_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "flash.https".into(),
                    kind: "flash_resource_https".into(),
                    message: "flash https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_flash,
            &mut flash_fail,
        );
        assert!(flash_fail.iter().any(|f| f.requirement_id == "flash.https"));

        let mut params_miss = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "params.len".into(),
                    kind: "min_ad_parameters_length".into(),
                    message: "need params".into(),
                    element: None,
                    max: None,
                    min: Some(8),
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut params_miss,
        );
        assert!(params_miss.iter().any(|f| f.requirement_id == "params.len"));

        let params = xml.replace(
            "</Duration>",
            "</Duration><AdParameters><![CDATA[{\"url\":\"https://t.example.com/vpaid\"}]]></AdParameters>",
        );
        let mut params_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "params.len".into(),
                    kind: "min_ad_parameters_length".into(),
                    message: "need params".into(),
                    element: None,
                    max: None,
                    min: Some(8),
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &params,
            &mut params_ok,
        );
        assert!(!params_ok.iter().any(|f| f.requirement_id == "params.len"));
    }

    #[test]
    fn closed_caption_icon_viewtracking_https_and_ad_system() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut cc_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cc.https".into(),
                    kind: "closed_caption_https".into(),
                    message: "caption https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut cc_ok,
        );
        assert!(!cc_ok.iter().any(|f| f.requirement_id == "cc.https"));

        let http_cc = xml.replace(
            "</MediaFiles>",
            "<ClosedCaptionFiles><ClosedCaptionFile language=\"en\"><![CDATA[http://cdn.example.com/ad.vtt]]></ClosedCaptionFile></ClosedCaptionFiles></MediaFiles>",
        );
        let mut cc_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cc.https".into(),
                    kind: "closed_caption_https".into(),
                    message: "caption https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_cc,
            &mut cc_fail,
        );
        assert!(cc_fail.iter().any(|f| f.requirement_id == "cc.https"));

        let mut iv_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "iv.https".into(),
                    kind: "icon_viewtracking_https".into(),
                    message: "icon view https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut iv_ok,
        );
        assert!(!iv_ok.iter().any(|f| f.requirement_id == "iv.https"));

        let http_iv = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconViewTracking><![CDATA[http://t.example.com/iv]]></IconViewTracking></Icon></Icons></Linear>",
        );
        let mut iv_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "iv.https".into(),
                    kind: "icon_viewtracking_https".into(),
                    message: "icon view https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_iv,
            &mut iv_fail,
        );
        assert!(iv_fail.iter().any(|f| f.requirement_id == "iv.https"));

        let mut sys_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "sys.name".into(),
                    kind: "ad_system".into(),
                    message: "need test".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["Test".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut sys_ok,
        );
        assert!(!sys_ok.iter().any(|f| f.requirement_id == "sys.name"));

        let mut sys_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "sys.name".into(),
                    kind: "ad_system".into(),
                    message: "need acme".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["Acme DSP".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut sys_fail,
        );
        assert!(sys_fail.iter().any(|f| f.requirement_id == "sys.name"));
    }

    #[test]
    fn icon_clickfallback_mezzanine_https_and_category() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut fb_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "fb.https".into(),
                    kind: "icon_clickfallback_https".into(),
                    message: "fallback https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut fb_ok,
        );
        assert!(!fb_ok.iter().any(|f| f.requirement_id == "fb.https"));

        let http_fb = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconClicks><IconClickFallbackImages><IconClickFallbackImage><![CDATA[http://cdn.example.com/icon.png]]></IconClickFallbackImage></IconClickFallbackImages></IconClicks></Icon></Icons></Linear>",
        );
        let mut fb_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "fb.https".into(),
                    kind: "icon_clickfallback_https".into(),
                    message: "fallback https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_fb,
            &mut fb_fail,
        );
        assert!(fb_fail.iter().any(|f| f.requirement_id == "fb.https"));

        let mut mezz_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.https".into(),
                    kind: "mezzanine_https".into(),
                    message: "mezz https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut mezz_ok,
        );
        assert!(!mezz_ok.iter().any(|f| f.requirement_id == "mezz.https"));

        let http_mezz = xml.replace(
            "</AdServingId>",
            "</AdServingId><Mezzanine><![CDATA[http://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        let mut mezz_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.https".into(),
                    kind: "mezzanine_https".into(),
                    message: "mezz https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_mezz,
            &mut mezz_fail,
        );
        assert!(mezz_fail.iter().any(|f| f.requirement_id == "mezz.https"));

        let cat_xml = xml.replace("</AdTitle>", "</AdTitle><Category>IAB1-1</Category>");
        let mut cat_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cat.name".into(),
                    kind: "category".into(),
                    message: "need iab".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["IAB1-1".into()]),
                }],
                ..Default::default()
            },
            &cat_xml,
            &mut cat_ok,
        );
        assert!(!cat_ok.iter().any(|f| f.requirement_id == "cat.name"));

        let mut cat_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cat.name".into(),
                    kind: "category".into(),
                    message: "need iab2".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["IAB2-1".into()]),
                }],
                ..Default::default()
            },
            &cat_xml,
            &mut cat_fail,
        );
        assert!(cat_fail.iter().any(|f| f.requirement_id == "cat.name"));

        let mut cat_miss = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cat.name".into(),
                    kind: "category".into(),
                    message: "need iab".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["IAB1-1".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut cat_miss,
        );
        assert!(cat_miss.iter().any(|f| f.requirement_id == "cat.name"));
    }

    #[test]
    fn tracking_https_pricing_and_extension_https() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut tr_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "tr.https".into(),
                    kind: "tracking_https".into(),
                    message: "tracking https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut tr_ok,
        );
        assert!(!tr_ok.iter().any(|f| f.requirement_id == "tr.https"));

        let http_tr = xml.replace("https://t.example.com/start", "http://t.example.com/start");
        let mut tr_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "tr.https".into(),
                    kind: "tracking_https".into(),
                    message: "tracking https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_tr,
            &mut tr_fail,
        );
        assert!(tr_fail.iter().any(|f| f.requirement_id == "tr.https"));

        let price_xml = xml.replace(
            "</AdServingId>",
            "</AdServingId><Pricing model=\"CPM\" currency=\"USD\"><![CDATA[2.50]]></Pricing>",
        );
        let mut price_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "price.amt".into(),
                    kind: "pricing".into(),
                    message: "need 2.50".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["2.50".into()]),
                }],
                ..Default::default()
            },
            &price_xml,
            &mut price_ok,
        );
        assert!(!price_ok.iter().any(|f| f.requirement_id == "price.amt"));

        let mut price_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "price.amt".into(),
                    kind: "pricing".into(),
                    message: "need 1.00".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1.00".into()]),
                }],
                ..Default::default()
            },
            &price_xml,
            &mut price_fail,
        );
        assert!(price_fail.iter().any(|f| f.requirement_id == "price.amt"));

        let mut ext_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ext.https".into(),
                    kind: "extension_https".into(),
                    message: "ext https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut ext_ok,
        );
        assert!(!ext_ok.iter().any(|f| f.requirement_id == "ext.https"));

        let http_ext = xml.replace(
            "</InLine>",
            "<Extensions><Extension type=\"geo\"><![CDATA[http://t.example.com/ext]]></Extension></Extensions></InLine>",
        );
        let mut ext_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ext.https".into(),
                    kind: "extension_https".into(),
                    message: "ext https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_ext,
            &mut ext_fail,
        );
        assert!(ext_fail.iter().any(|f| f.requirement_id == "ext.https"));
    }

    #[test]
    fn icf_https_universal_ad_id_and_api_framework() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut icf_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icf.https".into(),
                    kind: "interactive_creative_file_https".into(),
                    message: "icf https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut icf_ok,
        );
        assert!(!icf_ok.iter().any(|f| f.requirement_id == "icf.https"));

        let http_icf = xml.replace(
            "</Linear>",
            "</Linear><InteractiveCreativeFile type=\"application/javascript\" apiFramework=\"omid\"><![CDATA[http://cdn.example.com/v.js]]></InteractiveCreativeFile>",
        );
        let mut icf_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icf.https".into(),
                    kind: "interactive_creative_file_https".into(),
                    message: "icf https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &http_icf,
            &mut icf_fail,
        );
        assert!(icf_fail.iter().any(|f| f.requirement_id == "icf.https"));

        let mut uad_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "uad.id".into(),
                    kind: "universal_ad_id".into(),
                    message: "need abcd".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["ABCD1234567".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut uad_ok,
        );
        assert!(!uad_ok.iter().any(|f| f.requirement_id == "uad.id"));

        let mut uad_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "uad.id".into(),
                    kind: "universal_ad_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["OTHER".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut uad_fail,
        );
        assert!(uad_fail.iter().any(|f| f.requirement_id == "uad.id"));

        let mut api_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "api.name".into(),
                    kind: "api_framework".into(),
                    message: "need omid".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["omid".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut api_ok,
        );
        assert!(!api_ok.iter().any(|f| f.requirement_id == "api.name"));

        let vpaid = xml.replace("<MediaFile ", "<MediaFile apiFramework=\"VPAID\" ");
        let mut api_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "api.name".into(),
                    kind: "api_framework".into(),
                    message: "need omid".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["omid".into()]),
                }],
                ..Default::default()
            },
            &vpaid,
            &mut api_fail,
        );
        assert!(api_fail.iter().any(|f| f.requirement_id == "api.name"));

        let mezz_vpaid = xml.replace(
            "</MediaFiles>",
            "<Mezzanine apiFramework=\"VPAID\"><![CDATA[https://cdn.example.com/ad.mp4]]></Mezzanine></MediaFiles>",
        );
        let mut mezz_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "api.name".into(),
                    kind: "api_framework".into(),
                    message: "need omid".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["omid".into()]),
                }],
                ..Default::default()
            },
            &mezz_vpaid,
            &mut mezz_fail,
        );
        assert!(mezz_fail.iter().any(|f| f.requirement_id == "api.name"));

        let ver_vpaid = xml.replace(
            "</InLine>",
            "<AdVerifications><Verification apiFramework=\"VPAID\"></Verification></AdVerifications></InLine>",
        );
        let mut ver_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "api.name".into(),
                    kind: "api_framework".into(),
                    message: "need omid".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["omid".into()]),
                }],
                ..Default::default()
            },
            &ver_vpaid,
            &mut ver_fail,
        );
        assert!(ver_fail.iter().any(|f| f.requirement_id == "api.name"));
    }

    #[test]
    fn creative_adid_icf_type_and_viewable_impression_id() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let adid_xml = xml.replace("<Creative>", "<Creative AdID=\"ad-1\">");
        let mut adid_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cr.adid".into(),
                    kind: "creative_adid".into(),
                    message: "need ad-1".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["ad-1".into()]),
                }],
                ..Default::default()
            },
            &adid_xml,
            &mut adid_ok,
        );
        assert!(!adid_ok.iter().any(|f| f.requirement_id == "cr.adid"));

        let mut adid_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cr.adid".into(),
                    kind: "creative_adid".into(),
                    message: "need ad-2".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["ad-2".into()]),
                }],
                ..Default::default()
            },
            &adid_xml,
            &mut adid_fail,
        );
        assert!(adid_fail.iter().any(|f| f.requirement_id == "cr.adid"));

        let icf_xml = xml.replace(
            "</Linear>",
            "</Linear><InteractiveCreativeFile type=\"application/javascript\"></InteractiveCreativeFile>",
        );
        let mut icf_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icf.type".into(),
                    kind: "interactive_creative_file_type".into(),
                    message: "need js".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["application/javascript".into()]),
                }],
                ..Default::default()
            },
            &icf_xml,
            &mut icf_ok,
        );
        assert!(!icf_ok.iter().any(|f| f.requirement_id == "icf.type"));

        let mut icf_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icf.type".into(),
                    kind: "interactive_creative_file_type".into(),
                    message: "need flash".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["application/x-shockwave-flash".into()]),
                }],
                ..Default::default()
            },
            &icf_xml,
            &mut icf_fail,
        );
        assert!(icf_fail.iter().any(|f| f.requirement_id == "icf.type"));

        let view_xml = xml.replace(
            "</Impression>",
            "</Impression><ViewableImpression id=\"view-1\"><Viewable><![CDATA[https://t.example.com/view]]></Viewable></ViewableImpression>",
        );
        let mut view_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "view.id".into(),
                    kind: "viewable_impression_id".into(),
                    message: "need view-1".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["view-1".into()]),
                }],
                ..Default::default()
            },
            &view_xml,
            &mut view_ok,
        );
        assert!(!view_ok.iter().any(|f| f.requirement_id == "view.id"));

        let mut view_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "view.id".into(),
                    kind: "viewable_impression_id".into(),
                    message: "need view-2".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["view-2".into()]),
                }],
                ..Default::default()
            },
            &view_xml,
            &mut view_fail,
        );
        assert!(view_fail.iter().any(|f| f.requirement_id == "view.id"));
    }

    #[test]
    fn companion_type_icon_type_and_ad_id() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let comp_xml = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\" type=\"image/png\"><StaticResource creativeType=\"image/png\"><![CDATA[https://cdn.example.com/i.png]]></StaticResource></Companion></CompanionAds></Creative>",
        );
        let mut comp_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.type".into(),
                    kind: "companion_type".into(),
                    message: "need png".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["image/png".into()]),
                }],
                ..Default::default()
            },
            &comp_xml,
            &mut comp_ok,
        );
        assert!(!comp_ok.iter().any(|f| f.requirement_id == "comp.type"));

        let mut comp_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.type".into(),
                    kind: "companion_type".into(),
                    message: "need jpeg".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["image/jpeg".into()]),
                }],
                ..Default::default()
            },
            &comp_xml,
            &mut comp_fail,
        );
        assert!(comp_fail.iter().any(|f| f.requirement_id == "comp.type"));

        let icon_xml = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" type=\"image/png\" width=\"20\" height=\"20\"></Icon></Icons></Linear>",
        );
        let mut icon_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icon.type".into(),
                    kind: "icon_type".into(),
                    message: "need png".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["image/png".into()]),
                }],
                ..Default::default()
            },
            &icon_xml,
            &mut icon_ok,
        );
        assert!(!icon_ok.iter().any(|f| f.requirement_id == "icon.type"));

        let mut icon_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icon.type".into(),
                    kind: "icon_type".into(),
                    message: "need jpeg".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["image/jpeg".into()]),
                }],
                ..Default::default()
            },
            &icon_xml,
            &mut icon_fail,
        );
        assert!(icon_fail.iter().any(|f| f.requirement_id == "icon.type"));

        let mut ad_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ad.id".into(),
                    kind: "ad_id".into(),
                    message: "need 1".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut ad_ok,
        );
        assert!(!ad_ok.iter().any(|f| f.requirement_id == "ad.id"));

        let mut ad_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ad.id".into(),
                    kind: "ad_id".into(),
                    message: "need 2".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["2".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut ad_fail,
        );
        assert!(ad_fail.iter().any(|f| f.requirement_id == "ad.id"));
    }

    #[test]
    fn mediafile_id_companion_adslot_and_icon_alt() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mf_xml = xml.replace("<MediaFile ", "<MediaFile id=\"mf-1\" ");
        let mut mf_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.id".into(),
                    kind: "mediafile_id".into(),
                    message: "need mf-1".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["mf-1".into()]),
                }],
                ..Default::default()
            },
            &mf_xml,
            &mut mf_ok,
        );
        assert!(!mf_ok.iter().any(|f| f.requirement_id == "mf.id"));

        let mut mf_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.id".into(),
                    kind: "mediafile_id".into(),
                    message: "need mf-2".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["mf-2".into()]),
                }],
                ..Default::default()
            },
            &mf_xml,
            &mut mf_fail,
        );
        assert!(mf_fail.iter().any(|f| f.requirement_id == "mf.id"));

        let slot_xml = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\" adSlotId=\"slot-1\"></Companion></CompanionAds></Creative>",
        );
        let mut slot_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.slot".into(),
                    kind: "companion_adslot_id".into(),
                    message: "need slot-1".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["slot-1".into()]),
                }],
                ..Default::default()
            },
            &slot_xml,
            &mut slot_ok,
        );
        assert!(!slot_ok.iter().any(|f| f.requirement_id == "comp.slot"));

        let mut slot_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.slot".into(),
                    kind: "companion_adslot_id".into(),
                    message: "need slot-2".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["slot-2".into()]),
                }],
                ..Default::default()
            },
            &slot_xml,
            &mut slot_fail,
        );
        assert!(slot_fail.iter().any(|f| f.requirement_id == "comp.slot"));

        let alt_xml = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" altText=\"AdChoices\" width=\"20\" height=\"20\"></Icon></Icons></Linear>",
        );
        let mut alt_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icon.alt".into(),
                    kind: "icon_alt_text".into(),
                    message: "need AdChoices".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["AdChoices".into()]),
                }],
                ..Default::default()
            },
            &alt_xml,
            &mut alt_ok,
        );
        assert!(!alt_ok.iter().any(|f| f.requirement_id == "icon.alt"));

        let mut alt_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icon.alt".into(),
                    kind: "icon_alt_text".into(),
                    message: "need Other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["Other".into()]),
                }],
                ..Default::default()
            },
            &alt_xml,
            &mut alt_fail,
        );
        assert!(alt_fail.iter().any(|f| f.requirement_id == "icon.alt"));
    }

    #[test]
    fn companion_id_icon_hover_and_nonlinear_id() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let cid_xml = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion id=\"comp-1\" width=\"300\" height=\"250\"></Companion></CompanionAds></Creative>",
        );
        let mut cid_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.id".into(),
                    kind: "companion_id".into(),
                    message: "need comp-1".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["comp-1".into()]),
                }],
                ..Default::default()
            },
            &cid_xml,
            &mut cid_ok,
        );
        assert!(!cid_ok.iter().any(|f| f.requirement_id == "comp.id"));

        let mut cid_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.id".into(),
                    kind: "companion_id".into(),
                    message: "need comp-2".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["comp-2".into()]),
                }],
                ..Default::default()
            },
            &cid_xml,
            &mut cid_fail,
        );
        assert!(cid_fail.iter().any(|f| f.requirement_id == "comp.id"));

        let hover_xml = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" hoverText=\"Learn more\" width=\"20\" height=\"20\"></Icon></Icons></Linear>",
        );
        let mut hover_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icon.hover".into(),
                    kind: "icon_hover_text".into(),
                    message: "need hover".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["Learn more".into()]),
                }],
                ..Default::default()
            },
            &hover_xml,
            &mut hover_ok,
        );
        assert!(!hover_ok.iter().any(|f| f.requirement_id == "icon.hover"));

        let mut hover_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icon.hover".into(),
                    kind: "icon_hover_text".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["Other".into()]),
                }],
                ..Default::default()
            },
            &hover_xml,
            &mut hover_fail,
        );
        assert!(hover_fail.iter().any(|f| f.requirement_id == "icon.hover"));

        let nl_xml = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear id=\"nl-1\" width=\"300\" height=\"250\"></NonLinear></NonLinearAds>",
        );
        let mut nl_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.id".into(),
                    kind: "nonlinear_id".into(),
                    message: "need nl-1".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["nl-1".into()]),
                }],
                ..Default::default()
            },
            &nl_xml,
            &mut nl_ok,
        );
        assert!(!nl_ok.iter().any(|f| f.requirement_id == "nl.id"));

        let mut nl_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.id".into(),
                    kind: "nonlinear_id".into(),
                    message: "need nl-2".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["nl-2".into()]),
                }],
                ..Default::default()
            },
            &nl_xml,
            &mut nl_fail,
        );
        assert!(nl_fail.iter().any(|f| f.requirement_id == "nl.id"));
    }

    #[test]
    fn companion_rendering_mode_icon_pxratio_and_nonlinear_scalable() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let render_xml = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\" renderingMode=\"end-card\"></Companion></CompanionAds></Creative>",
        );
        let mut mode_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.mode".into(),
                    kind: "companion_rendering_mode".into(),
                    message: "need end-card".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["end-card".into()]),
                }],
                ..Default::default()
            },
            &render_xml,
            &mut mode_ok,
        );
        assert!(!mode_ok.iter().any(|f| f.requirement_id == "comp.mode"));

        let mut mode_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.mode".into(),
                    kind: "companion_rendering_mode".into(),
                    message: "need default".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["default".into()]),
                }],
                ..Default::default()
            },
            &render_xml,
            &mut mode_fail,
        );
        assert!(mode_fail.iter().any(|f| f.requirement_id == "comp.mode"));

        let px_xml = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" pxratio=\"2\" width=\"20\" height=\"20\"></Icon></Icons></Linear>",
        );
        let mut px_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icon.px".into(),
                    kind: "icon_pxratio".into(),
                    message: "need 2".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["2".into()]),
                }],
                ..Default::default()
            },
            &px_xml,
            &mut px_ok,
        );
        assert!(!px_ok.iter().any(|f| f.requirement_id == "icon.px"));

        let mut px_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icon.px".into(),
                    kind: "icon_pxratio".into(),
                    message: "need 1".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1".into()]),
                }],
                ..Default::default()
            },
            &px_xml,
            &mut px_fail,
        );
        assert!(px_fail.iter().any(|f| f.requirement_id == "icon.px"));

        let scal_xml = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear scalable=\"true\" width=\"300\" height=\"250\"></NonLinear></NonLinearAds>",
        );
        let mut scal_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.scal".into(),
                    kind: "nonlinear_scalable".into(),
                    message: "need true".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["true".into()]),
                }],
                ..Default::default()
            },
            &scal_xml,
            &mut scal_ok,
        );
        assert!(!scal_ok.iter().any(|f| f.requirement_id == "nl.scal"));

        let mut scal_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.scal".into(),
                    kind: "nonlinear_scalable".into(),
                    message: "need false".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["false".into()]),
                }],
                ..Default::default()
            },
            &scal_xml,
            &mut scal_fail,
        );
        assert!(scal_fail.iter().any(|f| f.requirement_id == "nl.scal"));
    }

    #[test]
    fn companion_expanded_size_and_nonlinear_maintain_aspect_ratio() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let exp_xml = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\" expandedWidth=\"600\" expandedHeight=\"500\"></Companion></CompanionAds></Creative>",
        );
        let mut exp_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "comp.ew".into(),
                        kind: "min_companion_expanded_width".into(),
                        message: "need expanded width".into(),
                        element: None,
                        max: None,
                        min: Some(600),
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "comp.eh".into(),
                        kind: "min_companion_expanded_height".into(),
                        message: "need expanded height".into(),
                        element: None,
                        max: None,
                        min: Some(500),
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &exp_xml,
            &mut exp_ok,
        );
        assert!(!exp_ok
            .iter()
            .any(|f| f.requirement_id.starts_with("comp.e")));

        let split = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion expandedWidth=\"600\" expandedHeight=\"1\"></Companion><Companion expandedWidth=\"1\" expandedHeight=\"500\"></Companion></CompanionAds></Creative>",
        );
        let mut exp_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "comp.ew".into(),
                        kind: "min_companion_expanded_width".into(),
                        message: "need expanded width".into(),
                        element: None,
                        max: None,
                        min: Some(600),
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "comp.eh".into(),
                        kind: "min_companion_expanded_height".into(),
                        message: "need expanded height".into(),
                        element: None,
                        max: None,
                        min: Some(500),
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &split,
            &mut exp_fail,
        );
        assert!(exp_fail.iter().any(|f| f.requirement_id == "comp.ew"));

        let mar_xml = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear maintainAspectRatio=\"true\" width=\"300\" height=\"250\"></NonLinear></NonLinearAds>",
        );
        let mut mar_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.mar".into(),
                    kind: "nonlinear_maintain_aspect_ratio".into(),
                    message: "need true".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["true".into()]),
                }],
                ..Default::default()
            },
            &mar_xml,
            &mut mar_ok,
        );
        assert!(!mar_ok.iter().any(|f| f.requirement_id == "nl.mar"));

        let mut mar_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.mar".into(),
                    kind: "nonlinear_maintain_aspect_ratio".into(),
                    message: "need false".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["false".into()]),
                }],
                ..Default::default()
            },
            &mar_xml,
            &mut mar_fail,
        );
        assert!(mar_fail.iter().any(|f| f.requirement_id == "nl.mar"));
    }

    #[test]
    fn companion_asset_size_and_nonlinear_expanded_size() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let asset_xml = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\" assetWidth=\"300\" assetHeight=\"250\"></Companion></CompanionAds></Creative>",
        );
        let mut asset_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "comp.aw".into(),
                        kind: "min_companion_asset_width".into(),
                        message: "need asset width".into(),
                        element: None,
                        max: None,
                        min: Some(300),
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "comp.ah".into(),
                        kind: "min_companion_asset_height".into(),
                        message: "need asset height".into(),
                        element: None,
                        max: None,
                        min: Some(250),
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &asset_xml,
            &mut asset_ok,
        );
        assert!(!asset_ok
            .iter()
            .any(|f| f.requirement_id.starts_with("comp.a")));

        let split = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion assetWidth=\"300\" assetHeight=\"1\"></Companion><Companion assetWidth=\"1\" assetHeight=\"250\"></Companion></CompanionAds></Creative>",
        );
        let mut asset_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "comp.aw".into(),
                        kind: "min_companion_asset_width".into(),
                        message: "need asset width".into(),
                        element: None,
                        max: None,
                        min: Some(300),
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "comp.ah".into(),
                        kind: "min_companion_asset_height".into(),
                        message: "need asset height".into(),
                        element: None,
                        max: None,
                        min: Some(250),
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &split,
            &mut asset_fail,
        );
        assert!(asset_fail.iter().any(|f| f.requirement_id == "comp.aw"));

        let nl_xml = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear expandedWidth=\"600\" expandedHeight=\"500\" width=\"300\" height=\"250\"></NonLinear></NonLinearAds>",
        );
        let mut nl_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "nl.ew".into(),
                        kind: "min_nonlinear_expanded_width".into(),
                        message: "need expanded width".into(),
                        element: None,
                        max: None,
                        min: Some(600),
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "nl.eh".into(),
                        kind: "min_nonlinear_expanded_height".into(),
                        message: "need expanded height".into(),
                        element: None,
                        max: None,
                        min: Some(500),
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &nl_xml,
            &mut nl_ok,
        );
        assert!(!nl_ok.iter().any(|f| f.requirement_id.starts_with("nl.e")));

        let split_nl = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear expandedWidth=\"600\" expandedHeight=\"1\"></NonLinear><NonLinear expandedWidth=\"1\" expandedHeight=\"500\"></NonLinear></NonLinearAds>",
        );
        let mut nl_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "nl.ew".into(),
                        kind: "min_nonlinear_expanded_width".into(),
                        message: "need expanded width".into(),
                        element: None,
                        max: None,
                        min: Some(600),
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "nl.eh".into(),
                        kind: "min_nonlinear_expanded_height".into(),
                        message: "need expanded height".into(),
                        element: None,
                        max: None,
                        min: Some(500),
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &split_nl,
            &mut nl_fail,
        );
        assert!(nl_fail.iter().any(|f| f.requirement_id == "nl.ew"));
    }

    #[test]
    fn companion_xml_encoded_pxratio_and_nonlinear_size() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let enc_xml = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\" xmlEncoded=\"true\"></Companion></CompanionAds></Creative>",
        );
        let mut enc_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.xml".into(),
                    kind: "companion_xml_encoded".into(),
                    message: "need true".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["true".into()]),
                }],
                ..Default::default()
            },
            &enc_xml,
            &mut enc_ok,
        );
        assert!(!enc_ok.iter().any(|f| f.requirement_id == "comp.xml"));

        let mut enc_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.xml".into(),
                    kind: "companion_xml_encoded".into(),
                    message: "need false".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["false".into()]),
                }],
                ..Default::default()
            },
            &enc_xml,
            &mut enc_fail,
        );
        assert!(enc_fail.iter().any(|f| f.requirement_id == "comp.xml"));

        let cpx_xml = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\" pxratio=\"2\"></Companion></CompanionAds></Creative>",
        );
        let mut cpx_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.px".into(),
                    kind: "companion_pxratio".into(),
                    message: "need 2".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["2".into()]),
                }],
                ..Default::default()
            },
            &cpx_xml,
            &mut cpx_ok,
        );
        assert!(!cpx_ok.iter().any(|f| f.requirement_id == "comp.px"));

        let mut cpx_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.px".into(),
                    kind: "companion_pxratio".into(),
                    message: "need 1".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1".into()]),
                }],
                ..Default::default()
            },
            &cpx_xml,
            &mut cpx_fail,
        );
        assert!(cpx_fail.iter().any(|f| f.requirement_id == "comp.px"));

        let nl_xml = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\" height=\"250\"></NonLinear></NonLinearAds>",
        );
        let mut nl_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "nl.w".into(),
                        kind: "min_nonlinear_width".into(),
                        message: "need width".into(),
                        element: None,
                        max: None,
                        min: Some(300),
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "nl.h".into(),
                        kind: "min_nonlinear_height".into(),
                        message: "need height".into(),
                        element: None,
                        max: None,
                        min: Some(250),
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &nl_xml,
            &mut nl_ok,
        );
        assert!(!nl_ok.iter().any(|f| f.requirement_id.starts_with("nl.")));

        let split = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\" height=\"1\"></NonLinear><NonLinear width=\"1\" height=\"250\"></NonLinear></NonLinearAds>",
        );
        let mut nl_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "nl.w".into(),
                        kind: "min_nonlinear_width".into(),
                        message: "need width".into(),
                        element: None,
                        max: None,
                        min: Some(300),
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "nl.h".into(),
                        kind: "min_nonlinear_height".into(),
                        message: "need height".into(),
                        element: None,
                        max: None,
                        min: Some(250),
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &split,
            &mut nl_fail,
        );
        assert!(nl_fail.iter().any(|f| f.requirement_id == "nl.w"));
    }

    #[test]
    fn wrapper_follow_allow_multiple_and_fallback() {
        let wrap_xml = r#"<VAST><Ad><Wrapper followAdditionalWrappers="true" allowMultipleAds="true" fallbackOnNoAd="false"><VASTAdTagURI><![CDATA[https://example.com/vast]]></VASTAdTagURI></Wrapper></Ad></VAST>"#;
        let mut follow_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "wrap.follow".into(),
                    kind: "wrapper_follow_additional_wrappers".into(),
                    message: "need true".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["true".into()]),
                }],
                ..Default::default()
            },
            wrap_xml,
            &mut follow_ok,
        );
        assert!(!follow_ok.iter().any(|f| f.requirement_id == "wrap.follow"));

        let mut follow_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "wrap.follow".into(),
                    kind: "wrapper_follow_additional_wrappers".into(),
                    message: "need false".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["false".into()]),
                }],
                ..Default::default()
            },
            wrap_xml,
            &mut follow_fail,
        );
        assert!(follow_fail
            .iter()
            .any(|f| f.requirement_id == "wrap.follow"));

        let mut multi_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "wrap.multi".into(),
                    kind: "wrapper_allow_multiple_ads".into(),
                    message: "need true".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["true".into()]),
                }],
                ..Default::default()
            },
            wrap_xml,
            &mut multi_ok,
        );
        assert!(!multi_ok.iter().any(|f| f.requirement_id == "wrap.multi"));

        let mut multi_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "wrap.multi".into(),
                    kind: "wrapper_allow_multiple_ads".into(),
                    message: "need false".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["false".into()]),
                }],
                ..Default::default()
            },
            wrap_xml,
            &mut multi_fail,
        );
        assert!(multi_fail.iter().any(|f| f.requirement_id == "wrap.multi"));

        let mut fallback_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "wrap.fallback".into(),
                    kind: "wrapper_fallback_on_no_ad".into(),
                    message: "need false".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["false".into()]),
                }],
                ..Default::default()
            },
            wrap_xml,
            &mut fallback_ok,
        );
        assert!(!fallback_ok
            .iter()
            .any(|f| f.requirement_id == "wrap.fallback"));

        let mut fallback_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "wrap.fallback".into(),
                    kind: "wrapper_fallback_on_no_ad".into(),
                    message: "need true".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["true".into()]),
                }],
                ..Default::default()
            },
            wrap_xml,
            &mut fallback_fail,
        );
        assert!(fallback_fail
            .iter()
            .any(|f| f.requirement_id == "wrap.fallback"));
    }

    #[test]
    fn ad_conditional_ad_and_mediafile_scalable() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let cond_xml = xml.replace("<Ad id=\"1\">", "<Ad id=\"1\" conditionalAd=\"true\">");
        let mut cond_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ad.cond".into(),
                    kind: "ad_conditional_ad".into(),
                    message: "need true".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["true".into()]),
                }],
                ..Default::default()
            },
            &cond_xml,
            &mut cond_ok,
        );
        assert!(!cond_ok.iter().any(|f| f.requirement_id == "ad.cond"));

        let mut cond_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ad.cond".into(),
                    kind: "ad_conditional_ad".into(),
                    message: "need false".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["false".into()]),
                }],
                ..Default::default()
            },
            &cond_xml,
            &mut cond_fail,
        );
        assert!(cond_fail.iter().any(|f| f.requirement_id == "ad.cond"));

        let scal_xml = xml.replace("<MediaFile ", "<MediaFile scalable=\"true\" ");
        let mut scal_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.scal".into(),
                    kind: "mediafile_scalable".into(),
                    message: "need true".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["true".into()]),
                }],
                ..Default::default()
            },
            &scal_xml,
            &mut scal_ok,
        );
        assert!(!scal_ok.iter().any(|f| f.requirement_id == "mf.scal"));

        let mut scal_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.scal".into(),
                    kind: "mediafile_scalable".into(),
                    message: "need false".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["false".into()]),
                }],
                ..Default::default()
            },
            &scal_xml,
            &mut scal_fail,
        );
        assert!(scal_fail.iter().any(|f| f.requirement_id == "mf.scal"));

        let mar_xml = xml.replace("<MediaFile ", "<MediaFile maintainAspectRatio=\"true\" ");
        let mut mar_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.mar".into(),
                    kind: "mediafile_maintain_aspect_ratio".into(),
                    message: "need true".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["true".into()]),
                }],
                ..Default::default()
            },
            &mar_xml,
            &mut mar_ok,
        );
        assert!(!mar_ok.iter().any(|f| f.requirement_id == "mf.mar"));

        let mut mar_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.mar".into(),
                    kind: "mediafile_maintain_aspect_ratio".into(),
                    message: "need false".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["false".into()]),
                }],
                ..Default::default()
            },
            &mar_xml,
            &mut mar_fail,
        );
        assert!(mar_fail.iter().any(|f| f.requirement_id == "mf.mar"));
    }

    #[test]
    fn executable_resource_type_js_browser_optional_and_extension_type() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let exec_xml = xml.replace(
            "</InLine>",
            "<AdVerifications><Verification><ExecutableResource type=\"application/javascript\"></ExecutableResource></Verification></AdVerifications></InLine>",
        );
        let mut exec_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "exec.type".into(),
                    kind: "executable_resource_type".into(),
                    message: "need js".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["application/javascript".into()]),
                }],
                ..Default::default()
            },
            &exec_xml,
            &mut exec_ok,
        );
        assert!(!exec_ok.iter().any(|f| f.requirement_id == "exec.type"));

        let mut exec_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "exec.type".into(),
                    kind: "executable_resource_type".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["application/x-javascript".into()]),
                }],
                ..Default::default()
            },
            &exec_xml,
            &mut exec_fail,
        );
        assert!(exec_fail.iter().any(|f| f.requirement_id == "exec.type"));

        let js_xml = xml.replace(
            "</InLine>",
            "<AdVerifications><Verification><JavaScriptResource browserOptional=\"true\"></JavaScriptResource></Verification></AdVerifications></InLine>",
        );
        let mut js_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "js.opt".into(),
                    kind: "javascript_resource_browser_optional".into(),
                    message: "need true".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["true".into()]),
                }],
                ..Default::default()
            },
            &js_xml,
            &mut js_ok,
        );
        assert!(!js_ok.iter().any(|f| f.requirement_id == "js.opt"));

        let mut js_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "js.opt".into(),
                    kind: "javascript_resource_browser_optional".into(),
                    message: "need false".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["false".into()]),
                }],
                ..Default::default()
            },
            &js_xml,
            &mut js_fail,
        );
        assert!(js_fail.iter().any(|f| f.requirement_id == "js.opt"));

        let ext_xml = xml.replace(
            "</InLine>",
            "<Extensions><Extension type=\"iab\"></Extension></Extensions></InLine>",
        );
        let mut ext_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ext.type".into(),
                    kind: "extension_type".into(),
                    message: "need iab".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["iab".into()]),
                }],
                ..Default::default()
            },
            &ext_xml,
            &mut ext_ok,
        );
        assert!(!ext_ok.iter().any(|f| f.requirement_id == "ext.type"));

        let mut ext_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ext.type".into(),
                    kind: "extension_type".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["other".into()]),
                }],
                ..Default::default()
            },
            &ext_xml,
            &mut ext_fail,
        );
        assert!(ext_fail.iter().any(|f| f.requirement_id == "ext.type"));
    }

    #[test]
    fn mezzanine_type_delivery_and_ad_parameters_xml_encoded() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mezz_xml = xml.replace(
            "</MediaFiles>",
            "<Mezzanine type=\"video/mp4\" delivery=\"progressive\"><![CDATA[https://cdn.example.com/ad.mp4]]></Mezzanine></MediaFiles>",
        );
        let mut type_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.type".into(),
                    kind: "mezzanine_type".into(),
                    message: "need mp4".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["video/mp4".into()]),
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut type_ok,
        );
        assert!(!type_ok.iter().any(|f| f.requirement_id == "mezz.type"));

        let mut type_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.type".into(),
                    kind: "mezzanine_type".into(),
                    message: "need webm".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["video/webm".into()]),
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut type_fail,
        );
        assert!(type_fail.iter().any(|f| f.requirement_id == "mezz.type"));

        let mut del_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.del".into(),
                    kind: "mezzanine_delivery".into(),
                    message: "need progressive".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["progressive".into()]),
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut del_ok,
        );
        assert!(!del_ok.iter().any(|f| f.requirement_id == "mezz.del"));

        let mut del_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.del".into(),
                    kind: "mezzanine_delivery".into(),
                    message: "need streaming".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["streaming".into()]),
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut del_fail,
        );
        assert!(del_fail.iter().any(|f| f.requirement_id == "mezz.del"));

        let params_xml = xml.replace(
            "</Linear>",
            "<AdParameters xmlEncoded=\"true\"><![CDATA[{}]]></AdParameters></Linear>",
        );
        let mut params_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "params.xml".into(),
                    kind: "ad_parameters_xml_encoded".into(),
                    message: "need true".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["true".into()]),
                }],
                ..Default::default()
            },
            &params_xml,
            &mut params_ok,
        );
        assert!(!params_ok.iter().any(|f| f.requirement_id == "params.xml"));

        let mut params_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "params.xml".into(),
                    kind: "ad_parameters_xml_encoded".into(),
                    message: "need false".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["false".into()]),
                }],
                ..Default::default()
            },
            &params_xml,
            &mut params_fail,
        );
        assert!(params_fail.iter().any(|f| f.requirement_id == "params.xml"));
    }

    #[test]
    fn mezzanine_codec_file_size_and_size() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mezz_xml = xml.replace(
            "</MediaFiles>",
            "<Mezzanine codec=\"avc1.42E01E\" fileSize=\"1000000\" width=\"1920\" height=\"1080\"><![CDATA[https://cdn.example.com/ad.mp4]]></Mezzanine></MediaFiles>",
        );
        let mut codec_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.codec".into(),
                    kind: "mezzanine_codec".into(),
                    message: "need avc".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["avc1.42E01E".into()]),
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut codec_ok,
        );
        assert!(!codec_ok.iter().any(|f| f.requirement_id == "mezz.codec"));

        let mut codec_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.codec".into(),
                    kind: "mezzanine_codec".into(),
                    message: "need vp9".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["vp9".into()]),
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut codec_fail,
        );
        assert!(codec_fail.iter().any(|f| f.requirement_id == "mezz.codec"));

        let mut fs_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.fs".into(),
                    kind: "min_mezzanine_file_size".into(),
                    message: "need size".into(),
                    element: None,
                    max: None,
                    min: Some(1000000),
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut fs_ok,
        );
        assert!(!fs_ok.iter().any(|f| f.requirement_id == "mezz.fs"));

        let mut fs_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.fs".into(),
                    kind: "min_mezzanine_file_size".into(),
                    message: "need bigger".into(),
                    element: None,
                    max: None,
                    min: Some(2000000),
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut fs_fail,
        );
        assert!(fs_fail.iter().any(|f| f.requirement_id == "mezz.fs"));

        let mut size_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "mezz.w".into(),
                        kind: "min_mezzanine_width".into(),
                        message: "need width".into(),
                        element: None,
                        max: None,
                        min: Some(1920),
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "mezz.h".into(),
                        kind: "min_mezzanine_height".into(),
                        message: "need height".into(),
                        element: None,
                        max: None,
                        min: Some(1080),
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &mezz_xml,
            &mut size_ok,
        );
        assert!(!size_ok
            .iter()
            .any(|f| f.requirement_id.starts_with("mezz.")));

        let split = xml.replace(
            "</MediaFiles>",
            "<Mezzanine width=\"1920\" height=\"1\"></Mezzanine><Mezzanine width=\"1\" height=\"1080\"></Mezzanine></MediaFiles>",
        );
        let mut size_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "mezz.w".into(),
                        kind: "min_mezzanine_width".into(),
                        message: "need width".into(),
                        element: None,
                        max: None,
                        min: Some(1920),
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "mezz.h".into(),
                        kind: "min_mezzanine_height".into(),
                        message: "need height".into(),
                        element: None,
                        max: None,
                        min: Some(1080),
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &split,
            &mut size_fail,
        );
        assert!(size_fail.iter().any(|f| f.requirement_id == "mezz.w"));
    }

    #[test]
    fn mezzanine_bitrate_and_media_type() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mezz_xml = xml.replace(
            "</MediaFiles>",
            "<Mezzanine bitrate=\"8000\" mediaType=\"video\"><![CDATA[https://cdn.example.com/ad.mp4]]></Mezzanine></MediaFiles>",
        );
        let mut br_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "mezz.br.min".into(),
                        kind: "min_mezzanine_bitrate".into(),
                        message: "need floor".into(),
                        element: None,
                        max: None,
                        min: Some(1000),
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "mezz.br.max".into(),
                        kind: "max_mezzanine_bitrate".into(),
                        message: "need cap".into(),
                        element: None,
                        max: Some(15000),
                        min: None,
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &mezz_xml,
            &mut br_ok,
        );
        assert!(!br_ok
            .iter()
            .any(|f| f.requirement_id.starts_with("mezz.br")));

        let mut br_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.br.min".into(),
                    kind: "min_mezzanine_bitrate".into(),
                    message: "need floor".into(),
                    element: None,
                    max: None,
                    min: Some(9000),
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut br_fail,
        );
        assert!(br_fail.iter().any(|f| f.requirement_id == "mezz.br.min"));

        let maxbr_xml = xml.replace(
            "</MediaFiles>",
            "<Mezzanine maxBitrate=\"8000\"><![CDATA[https://cdn.example.com/ad.mp4]]></Mezzanine></MediaFiles>",
        );
        let mut maxbr_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.br.min".into(),
                    kind: "min_mezzanine_bitrate".into(),
                    message: "need floor".into(),
                    element: None,
                    max: None,
                    min: Some(1000),
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &maxbr_xml,
            &mut maxbr_ok,
        );
        assert!(!maxbr_ok.iter().any(|f| f.requirement_id == "mezz.br.min"));

        let split = xml.replace(
            "</MediaFiles>",
            "<Mezzanine bitrate=\"500\"></Mezzanine><Mezzanine bitrate=\"20000\"></Mezzanine></MediaFiles>",
        );
        let mut split_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "mezz.br.min".into(),
                        kind: "min_mezzanine_bitrate".into(),
                        message: "need floor".into(),
                        element: None,
                        max: None,
                        min: Some(1000),
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "mezz.br.max".into(),
                        kind: "max_mezzanine_bitrate".into(),
                        message: "need cap".into(),
                        element: None,
                        max: Some(15000),
                        min: None,
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &split,
            &mut split_fail,
        );
        assert!(split_fail
            .iter()
            .any(|f| f.requirement_id.starts_with("mezz.br")));

        let mut mt_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.mt".into(),
                    kind: "mezzanine_media_type".into(),
                    message: "need video".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["video".into()]),
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut mt_ok,
        );
        assert!(!mt_ok.iter().any(|f| f.requirement_id == "mezz.mt"));

        let mut mt_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.mt".into(),
                    kind: "mezzanine_media_type".into(),
                    message: "need audio".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["audio".into()]),
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut mt_fail,
        );
        assert!(mt_fail.iter().any(|f| f.requirement_id == "mezz.mt"));
    }

    #[test]
    fn mezzanine_scalable_aspect_and_id() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mezz_xml = xml.replace(
            "</MediaFiles>",
            "<Mezzanine id=\"mezz-1\" scalable=\"true\" maintainAspectRatio=\"true\"><![CDATA[https://cdn.example.com/ad.mp4]]></Mezzanine></MediaFiles>",
        );
        let mut scal_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.scal".into(),
                    kind: "mezzanine_scalable".into(),
                    message: "need true".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["true".into()]),
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut scal_ok,
        );
        assert!(!scal_ok.iter().any(|f| f.requirement_id == "mezz.scal"));

        let mut scal_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.scal".into(),
                    kind: "mezzanine_scalable".into(),
                    message: "need false".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["false".into()]),
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut scal_fail,
        );
        assert!(scal_fail.iter().any(|f| f.requirement_id == "mezz.scal"));

        let mut mar_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.mar".into(),
                    kind: "mezzanine_maintain_aspect_ratio".into(),
                    message: "need true".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["true".into()]),
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut mar_ok,
        );
        assert!(!mar_ok.iter().any(|f| f.requirement_id == "mezz.mar"));

        let mut mar_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.mar".into(),
                    kind: "mezzanine_maintain_aspect_ratio".into(),
                    message: "need false".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["false".into()]),
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut mar_fail,
        );
        assert!(mar_fail.iter().any(|f| f.requirement_id == "mezz.mar"));

        let mut id_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.id".into(),
                    kind: "mezzanine_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["mezz-1".into()]),
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut id_ok,
        );
        assert!(!id_ok.iter().any(|f| f.requirement_id == "mezz.id"));

        let mut id_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.id".into(),
                    kind: "mezzanine_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["mezz-2".into()]),
                }],
                ..Default::default()
            },
            &mezz_xml,
            &mut id_fail,
        );
        assert!(id_fail.iter().any(|f| f.requirement_id == "mezz.id"));
    }

    #[test]
    fn icon_and_nonlinear_xml_encoded() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let icon_xml = xml.replace(
            "</Linear>",
            "<Icons><Icon xmlEncoded=\"true\"></Icon></Icons></Linear>",
        );
        let mut icon_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icon.xml".into(),
                    kind: "icon_xml_encoded".into(),
                    message: "need true".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["true".into()]),
                }],
                ..Default::default()
            },
            &icon_xml,
            &mut icon_ok,
        );
        assert!(!icon_ok.iter().any(|f| f.requirement_id == "icon.xml"));

        let mut icon_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icon.xml".into(),
                    kind: "icon_xml_encoded".into(),
                    message: "need false".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["false".into()]),
                }],
                ..Default::default()
            },
            &icon_xml,
            &mut icon_fail,
        );
        assert!(icon_fail.iter().any(|f| f.requirement_id == "icon.xml"));

        let nl_xml = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear xmlEncoded=\"true\" width=\"300\" height=\"250\"></NonLinear></NonLinearAds>",
        );
        let mut nl_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.xml".into(),
                    kind: "nonlinear_xml_encoded".into(),
                    message: "need true".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["true".into()]),
                }],
                ..Default::default()
            },
            &nl_xml,
            &mut nl_ok,
        );
        assert!(!nl_ok.iter().any(|f| f.requirement_id == "nl.xml"));

        let mut nl_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.xml".into(),
                    kind: "nonlinear_xml_encoded".into(),
                    message: "need false".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["false".into()]),
                }],
                ..Default::default()
            },
            &nl_xml,
            &mut nl_fail,
        );
        assert!(nl_fail.iter().any(|f| f.requirement_id == "nl.xml"));
    }

    #[test]
    fn icon_size_and_caption_file_status() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let icon_xml = xml.replace(
            "</Linear>",
            "<Icons><Icon width=\"20\" height=\"20\"></Icon></Icons></Linear>",
        );
        let mut size_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "icon.w".into(),
                        kind: "min_icon_width".into(),
                        message: "need width".into(),
                        element: None,
                        max: None,
                        min: Some(20),
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "icon.h".into(),
                        kind: "min_icon_height".into(),
                        message: "need height".into(),
                        element: None,
                        max: None,
                        min: Some(20),
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &icon_xml,
            &mut size_ok,
        );
        assert!(!size_ok
            .iter()
            .any(|f| f.requirement_id.starts_with("icon.")));

        let split = xml.replace(
            "</Linear>",
            "<Icons><Icon width=\"20\" height=\"1\"></Icon><Icon width=\"1\" height=\"20\"></Icon></Icons></Linear>",
        );
        let mut size_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "icon.w".into(),
                        kind: "min_icon_width".into(),
                        message: "need width".into(),
                        element: None,
                        max: None,
                        min: Some(20),
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "icon.h".into(),
                        kind: "min_icon_height".into(),
                        message: "need height".into(),
                        element: None,
                        max: None,
                        min: Some(20),
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &split,
            &mut size_fail,
        );
        assert!(size_fail.iter().any(|f| f.requirement_id == "icon.w"));

        let cap_xml = xml.replace(
            "</MediaFiles>",
            "<ClosedCaptionFiles><ClosedCaptionFile fileStatus=\"onDemand\" type=\"text/vtt\" language=\"en\"><![CDATA[https://cdn.example.com/ad.vtt]]></ClosedCaptionFile></ClosedCaptionFiles></MediaFiles>",
        );
        let mut status_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cap.status".into(),
                    kind: "caption_file_status".into(),
                    message: "need onDemand".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["onDemand".into()]),
                }],
                ..Default::default()
            },
            &cap_xml,
            &mut status_ok,
        );
        assert!(!status_ok.iter().any(|f| f.requirement_id == "cap.status"));

        let mut status_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cap.status".into(),
                    kind: "caption_file_status".into(),
                    message: "need live".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["live".into()]),
                }],
                ..Default::default()
            },
            &cap_xml,
            &mut status_fail,
        );
        assert!(status_fail.iter().any(|f| f.requirement_id == "cap.status"));
    }

    #[test]
    fn icon_click_fallback_size() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let fb_xml = xml.replace(
            "</Linear>",
            "<Icons><Icon><IconClicks><IconClickFallbackImages><IconClickFallbackImage width=\"20\" height=\"20\"><![CDATA[https://cdn.example.com/fb.png]]></IconClickFallbackImage></IconClickFallbackImages></IconClicks></Icon></Icons></Linear>",
        );
        let mut size_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "fb.w".into(),
                        kind: "min_icon_click_fallback_width".into(),
                        message: "need width".into(),
                        element: None,
                        max: None,
                        min: Some(20),
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "fb.h".into(),
                        kind: "min_icon_click_fallback_height".into(),
                        message: "need height".into(),
                        element: None,
                        max: None,
                        min: Some(20),
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &fb_xml,
            &mut size_ok,
        );
        assert!(!size_ok.iter().any(|f| f.requirement_id.starts_with("fb.")));

        let split = xml.replace(
            "</Linear>",
            "<Icons><Icon><IconClicks><IconClickFallbackImages><IconClickFallbackImage width=\"20\" height=\"1\"></IconClickFallbackImage><IconClickFallbackImage width=\"1\" height=\"20\"></IconClickFallbackImage></IconClickFallbackImages></IconClicks></Icon></Icons></Linear>",
        );
        let mut size_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "fb.w".into(),
                        kind: "min_icon_click_fallback_width".into(),
                        message: "need width".into(),
                        element: None,
                        max: None,
                        min: Some(20),
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "fb.h".into(),
                        kind: "min_icon_click_fallback_height".into(),
                        message: "need height".into(),
                        element: None,
                        max: None,
                        min: Some(20),
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &split,
            &mut size_fail,
        );
        assert!(size_fail.iter().any(|f| f.requirement_id == "fb.w"));
    }

    #[test]
    fn advertiser_id_companion_alt_text_and_verification_parameters_https() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let adv = xml.replace(
            "</AdTitle>",
            "</AdTitle><Advertiser id=\"adv-1\">Acme</Advertiser>",
        );
        let mut id_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "adv.id".into(),
                    kind: "advertiser_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["adv-1".into()]),
                }],
                ..Default::default()
            },
            &adv,
            &mut id_ok,
        );
        assert!(!id_ok.iter().any(|f| f.requirement_id == "adv.id"));

        let mut id_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "adv.id".into(),
                    kind: "advertiser_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["adv-2".into()]),
                }],
                ..Default::default()
            },
            &adv,
            &mut id_fail,
        );
        assert!(id_fail.iter().any(|f| f.requirement_id == "adv.id"));

        let alt = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><AltText>Learn more</AltText></Companion></CompanionAds></Creative>",
        );
        let mut alt_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.alt".into(),
                    kind: "companion_alt_text".into(),
                    message: "need alt".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["Learn more".into()]),
                }],
                ..Default::default()
            },
            &alt,
            &mut alt_ok,
        );
        assert!(!alt_ok.iter().any(|f| f.requirement_id == "comp.alt"));

        let mut alt_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.alt".into(),
                    kind: "companion_alt_text".into(),
                    message: "need skip".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["Skip".into()]),
                }],
                ..Default::default()
            },
            &alt,
            &mut alt_fail,
        );
        assert!(alt_fail.iter().any(|f| f.requirement_id == "comp.alt"));

        let mut vp_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "vp.https".into(),
                    kind: "verification_parameters_https".into(),
                    message: "vp https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut vp_ok,
        );
        assert!(!vp_ok.iter().any(|f| f.requirement_id == "vp.https"));

        let vp_http = xml.replace(
            "</InLine>",
            "<AdVerifications><Verification><VerificationParameters><![CDATA[{\"ping\":\"http://t.example.com/omid\"}]]></VerificationParameters></Verification></AdVerifications></InLine>",
        );
        let mut vp_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "vp.https".into(),
                    kind: "verification_parameters_https".into(),
                    message: "vp https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &vp_http,
            &mut vp_fail,
        );
        assert!(vp_fail.iter().any(|f| f.requirement_id == "vp.https"));

        let vp_json = xml.replace(
            "</InLine>",
            "<AdVerifications><Verification><VerificationParameters><![CDATA[{\"ping\":\"https://t.example.com/omid\"}]]></VerificationParameters></Verification></AdVerifications></InLine>",
        );
        let mut vp_json_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "vp.https".into(),
                    kind: "verification_parameters_https".into(),
                    message: "vp https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &vp_json,
            &mut vp_json_ok,
        );
        assert!(!vp_json_ok.iter().any(|f| f.requirement_id == "vp.https"));
    }

    #[test]
    fn ad_type_impression_id_and_companion_ads_required() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let typed = xml.replace("<Ad id=\"1\">", "<Ad id=\"1\" adType=\"video\">");
        let mut type_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ad.type".into(),
                    kind: "ad_type".into(),
                    message: "need video".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["video".into()]),
                }],
                ..Default::default()
            },
            &typed,
            &mut type_ok,
        );
        assert!(!type_ok.iter().any(|f| f.requirement_id == "ad.type"));

        let mut type_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ad.type".into(),
                    kind: "ad_type".into(),
                    message: "need audio".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["audio".into()]),
                }],
                ..Default::default()
            },
            &typed,
            &mut type_fail,
        );
        assert!(type_fail.iter().any(|f| f.requirement_id == "ad.type"));

        let imp = xml.replace("<Impression>", "<Impression id=\"imp-1\">");
        let mut imp_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "imp.id".into(),
                    kind: "impression_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["imp-1".into()]),
                }],
                ..Default::default()
            },
            &imp,
            &mut imp_ok,
        );
        assert!(!imp_ok.iter().any(|f| f.requirement_id == "imp.id"));

        let mut imp_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "imp.id".into(),
                    kind: "impression_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["imp-2".into()]),
                }],
                ..Default::default()
            },
            &imp,
            &mut imp_fail,
        );
        assert!(imp_fail.iter().any(|f| f.requirement_id == "imp.id"));

        let ads = xml.replace(
            "</Creative>",
            "<CompanionAds required=\"all\"><Companion width=\"300\" height=\"250\"></Companion></CompanionAds></Creative>",
        );
        let mut ads_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.req".into(),
                    kind: "companion_ads_required".into(),
                    message: "need all".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["all".into()]),
                }],
                ..Default::default()
            },
            &ads,
            &mut ads_ok,
        );
        assert!(!ads_ok.iter().any(|f| f.requirement_id == "comp.req"));

        let mut ads_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.req".into(),
                    kind: "companion_ads_required".into(),
                    message: "need any".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["any".into()]),
                }],
                ..Default::default()
            },
            &ads,
            &mut ads_fail,
        );
        assert!(ads_fail.iter().any(|f| f.requirement_id == "comp.req"));
    }

    #[test]
    fn mediafile_file_size_media_type_and_icf_variable_duration() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let sized = xml.replace(
            "<MediaFile ",
            "<MediaFile fileSize=\"1000000\" mediaType=\"2D\" ",
        );
        let mut fs_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.fs".into(),
                    kind: "min_mediafile_file_size".into(),
                    message: "need size".into(),
                    element: None,
                    max: None,
                    min: Some(1000000),
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &sized,
            &mut fs_ok,
        );
        assert!(!fs_ok.iter().any(|f| f.requirement_id == "mf.fs"));

        let mut fs_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.fs".into(),
                    kind: "min_mediafile_file_size".into(),
                    message: "need bigger".into(),
                    element: None,
                    max: None,
                    min: Some(2000000),
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &sized,
            &mut fs_fail,
        );
        assert!(fs_fail.iter().any(|f| f.requirement_id == "mf.fs"));

        let mut mt_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.mt".into(),
                    kind: "mediafile_media_type".into(),
                    message: "need 2D".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["2D".into()]),
                }],
                ..Default::default()
            },
            &sized,
            &mut mt_ok,
        );
        assert!(!mt_ok.iter().any(|f| f.requirement_id == "mf.mt"));

        let mut mt_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.mt".into(),
                    kind: "mediafile_media_type".into(),
                    message: "need 360".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["360".into()]),
                }],
                ..Default::default()
            },
            &sized,
            &mut mt_fail,
        );
        assert!(mt_fail.iter().any(|f| f.requirement_id == "mf.mt"));

        let icf = xml.replace(
            "</Linear>",
            "</Linear><InteractiveCreativeFile variableDuration=\"true\"></InteractiveCreativeFile>",
        );
        let mut var_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icf.var".into(),
                    kind: "interactive_creative_file_variable_duration".into(),
                    message: "need true".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["true".into()]),
                }],
                ..Default::default()
            },
            &icf,
            &mut var_ok,
        );
        assert!(!var_ok.iter().any(|f| f.requirement_id == "icf.var"));

        let mut var_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icf.var".into(),
                    kind: "interactive_creative_file_variable_duration".into(),
                    message: "need false".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["false".into()]),
                }],
                ..Default::default()
            },
            &icf,
            &mut var_fail,
        );
        assert!(var_fail.iter().any(|f| f.requirement_id == "icf.var"));
    }

    #[test]
    fn creative_id_clickthrough_id_and_tracking_offset() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let cr = xml.replace("<Creative>", "<Creative id=\"cr-1\">");
        let mut cr_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cr.id".into(),
                    kind: "creative_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["cr-1".into()]),
                }],
                ..Default::default()
            },
            &cr,
            &mut cr_ok,
        );
        assert!(!cr_ok.iter().any(|f| f.requirement_id == "cr.id"));

        let mut cr_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cr.id".into(),
                    kind: "creative_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["cr-2".into()]),
                }],
                ..Default::default()
            },
            &cr,
            &mut cr_fail,
        );
        assert!(cr_fail.iter().any(|f| f.requirement_id == "cr.id"));

        let ct = xml.replace(
            "</Duration>",
            "</Duration><VideoClicks><ClickThrough id=\"ct-1\"><![CDATA[https://adv.example.com/c]]></ClickThrough></VideoClicks>",
        );
        let mut ct_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ct.id".into(),
                    kind: "clickthrough_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["ct-1".into()]),
                }],
                ..Default::default()
            },
            &ct,
            &mut ct_ok,
        );
        assert!(!ct_ok.iter().any(|f| f.requirement_id == "ct.id"));

        let mut ct_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ct.id".into(),
                    kind: "clickthrough_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["ct-2".into()]),
                }],
                ..Default::default()
            },
            &ct,
            &mut ct_fail,
        );
        assert!(ct_fail.iter().any(|f| f.requirement_id == "ct.id"));

        let off = xml.replace(
            "<Tracking event=\"midpoint\">",
            "<Tracking event=\"midpoint\" offset=\"00:00:15\">",
        );
        let mut off_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "trk.off".into(),
                    kind: "tracking_offset".into(),
                    message: "need offset".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["00:00:15".into()]),
                }],
                ..Default::default()
            },
            &off,
            &mut off_ok,
        );
        assert!(!off_ok.iter().any(|f| f.requirement_id == "trk.off"));

        let mut off_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "trk.off".into(),
                    kind: "tracking_offset".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["00:00:10".into()]),
                }],
                ..Default::default()
            },
            &off,
            &mut off_fail,
        );
        assert!(off_fail.iter().any(|f| f.requirement_id == "trk.off"));
    }

    #[test]
    fn clicktracking_id_custom_click_id_and_icon_clickthrough_id() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let clk = xml.replace(
            "</Duration>",
            "</Duration><VideoClicks><ClickTracking id=\"clk-1\"><![CDATA[https://t.example.com/click]]></ClickTracking></VideoClicks>",
        );
        let mut clk_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "clk.id".into(),
                    kind: "clicktracking_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["clk-1".into()]),
                }],
                ..Default::default()
            },
            &clk,
            &mut clk_ok,
        );
        assert!(!clk_ok.iter().any(|f| f.requirement_id == "clk.id"));

        let mut clk_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "clk.id".into(),
                    kind: "clicktracking_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["clk-2".into()]),
                }],
                ..Default::default()
            },
            &clk,
            &mut clk_fail,
        );
        assert!(clk_fail.iter().any(|f| f.requirement_id == "clk.id"));

        let cust = xml.replace(
            "</Duration>",
            "</Duration><VideoClicks><CustomClick id=\"cust-1\"><![CDATA[https://t.example.com/custom]]></CustomClick></VideoClicks>",
        );
        let mut cust_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cust.id".into(),
                    kind: "custom_click_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["cust-1".into()]),
                }],
                ..Default::default()
            },
            &cust,
            &mut cust_ok,
        );
        assert!(!cust_ok.iter().any(|f| f.requirement_id == "cust.id"));

        let mut cust_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cust.id".into(),
                    kind: "custom_click_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["cust-2".into()]),
                }],
                ..Default::default()
            },
            &cust,
            &mut cust_fail,
        );
        assert!(cust_fail.iter().any(|f| f.requirement_id == "cust.id"));

        let ict = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconClicks><IconClickThrough id=\"ict-1\"><![CDATA[https://adv.example.com/i]]></IconClickThrough></IconClicks></Icon></Icons></Linear>",
        );
        let mut ict_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ict.id".into(),
                    kind: "icon_clickthrough_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["ict-1".into()]),
                }],
                ..Default::default()
            },
            &ict,
            &mut ict_ok,
        );
        assert!(!ict_ok.iter().any(|f| f.requirement_id == "ict.id"));

        let mut ict_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ict.id".into(),
                    kind: "icon_clickthrough_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["ict-2".into()]),
                }],
                ..Default::default()
            },
            &ict,
            &mut ict_fail,
        );
        assert!(ict_fail.iter().any(|f| f.requirement_id == "ict.id"));
    }

    #[test]
    fn companion_nonlinear_clickthrough_id_and_icon_clicktracking_id() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let cct = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><CompanionClickThrough id=\"cct-1\"><![CDATA[https://adv.example.com/c]]></CompanionClickThrough></Companion></CompanionAds></Creative>",
        );
        let mut cct_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cct.id".into(),
                    kind: "companion_clickthrough_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["cct-1".into()]),
                }],
                ..Default::default()
            },
            &cct,
            &mut cct_ok,
        );
        assert!(!cct_ok.iter().any(|f| f.requirement_id == "cct.id"));

        let mut cct_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cct.id".into(),
                    kind: "companion_clickthrough_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["cct-2".into()]),
                }],
                ..Default::default()
            },
            &cct,
            &mut cct_fail,
        );
        assert!(cct_fail.iter().any(|f| f.requirement_id == "cct.id"));

        let nlc = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\" height=\"250\"><NonLinearClickThrough id=\"nlc-1\"><![CDATA[https://adv.example.com/nl]]></NonLinearClickThrough></NonLinear></NonLinearAds>",
        );
        let mut nlc_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nlc.id".into(),
                    kind: "nonlinear_clickthrough_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["nlc-1".into()]),
                }],
                ..Default::default()
            },
            &nlc,
            &mut nlc_ok,
        );
        assert!(!nlc_ok.iter().any(|f| f.requirement_id == "nlc.id"));

        let mut nlc_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nlc.id".into(),
                    kind: "nonlinear_clickthrough_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["nlc-2".into()]),
                }],
                ..Default::default()
            },
            &nlc,
            &mut nlc_fail,
        );
        assert!(nlc_fail.iter().any(|f| f.requirement_id == "nlc.id"));

        let ictk = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconClicks><IconClickTracking id=\"ictk-1\"><![CDATA[https://t.example.com/i]]></IconClickTracking></IconClicks></Icon></Icons></Linear>",
        );
        let mut ictk_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ictk.id".into(),
                    kind: "icon_clicktracking_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["ictk-1".into()]),
                }],
                ..Default::default()
            },
            &ictk,
            &mut ictk_ok,
        );
        assert!(!ictk_ok.iter().any(|f| f.requirement_id == "ictk.id"));

        let mut ictk_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ictk.id".into(),
                    kind: "icon_clicktracking_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["ictk-2".into()]),
                }],
                ..Default::default()
            },
            &ictk,
            &mut ictk_fail,
        );
        assert!(ictk_fail.iter().any(|f| f.requirement_id == "ictk.id"));
    }

    #[test]
    fn companion_nonlinear_clicktracking_id_and_icon_viewtracking_id() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let cctk = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><CompanionClickTracking id=\"cctk-1\"><![CDATA[https://t.example.com/c]]></CompanionClickTracking></Companion></CompanionAds></Creative>",
        );
        let mut cctk_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cctk.id".into(),
                    kind: "companion_clicktracking_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["cctk-1".into()]),
                }],
                ..Default::default()
            },
            &cctk,
            &mut cctk_ok,
        );
        assert!(!cctk_ok.iter().any(|f| f.requirement_id == "cctk.id"));

        let mut cctk_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cctk.id".into(),
                    kind: "companion_clicktracking_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["cctk-2".into()]),
                }],
                ..Default::default()
            },
            &cctk,
            &mut cctk_fail,
        );
        assert!(cctk_fail.iter().any(|f| f.requirement_id == "cctk.id"));

        let nlct = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\" height=\"250\"><NonLinearClickTracking id=\"nlct-1\"><![CDATA[https://t.example.com/nl]]></NonLinearClickTracking></NonLinear></NonLinearAds>",
        );
        let mut nlct_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nlct.id".into(),
                    kind: "nonlinear_clicktracking_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["nlct-1".into()]),
                }],
                ..Default::default()
            },
            &nlct,
            &mut nlct_ok,
        );
        assert!(!nlct_ok.iter().any(|f| f.requirement_id == "nlct.id"));

        let mut nlct_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nlct.id".into(),
                    kind: "nonlinear_clicktracking_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["nlct-2".into()]),
                }],
                ..Default::default()
            },
            &nlct,
            &mut nlct_fail,
        );
        assert!(nlct_fail.iter().any(|f| f.requirement_id == "nlct.id"));

        let ivt = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconViewTracking id=\"ivt-1\"><![CDATA[https://t.example.com/iv]]></IconViewTracking></Icon></Icons></Linear>",
        );
        let mut ivt_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ivt.id".into(),
                    kind: "icon_viewtracking_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["ivt-1".into()]),
                }],
                ..Default::default()
            },
            &ivt,
            &mut ivt_ok,
        );
        assert!(!ivt_ok.iter().any(|f| f.requirement_id == "ivt.id"));

        let mut ivt_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ivt.id".into(),
                    kind: "icon_viewtracking_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["ivt-2".into()]),
                }],
                ..Default::default()
            },
            &ivt,
            &mut ivt_fail,
        );
        assert!(ivt_fail.iter().any(|f| f.requirement_id == "ivt.id"));
    }

    #[test]
    fn tracking_id_error_id_and_viewable_id() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let trk = xml.replace(
            "<Tracking event=\"start\">",
            "<Tracking event=\"start\" id=\"trk-1\">",
        );
        let mut trk_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "trk.id".into(),
                    kind: "tracking_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["trk-1".into()]),
                }],
                ..Default::default()
            },
            &trk,
            &mut trk_ok,
        );
        assert!(!trk_ok.iter().any(|f| f.requirement_id == "trk.id"));

        let mut trk_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "trk.id".into(),
                    kind: "tracking_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["trk-2".into()]),
                }],
                ..Default::default()
            },
            &trk,
            &mut trk_fail,
        );
        assert!(trk_fail.iter().any(|f| f.requirement_id == "trk.id"));

        let err = xml.replace(
            "</AdTitle>",
            "</AdTitle><Error id=\"err-1\"><![CDATA[https://t.example.com/error]]></Error>",
        );
        let mut err_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "err.id".into(),
                    kind: "error_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["err-1".into()]),
                }],
                ..Default::default()
            },
            &err,
            &mut err_ok,
        );
        assert!(!err_ok.iter().any(|f| f.requirement_id == "err.id"));

        let mut err_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "err.id".into(),
                    kind: "error_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["err-2".into()]),
                }],
                ..Default::default()
            },
            &err,
            &mut err_fail,
        );
        assert!(err_fail.iter().any(|f| f.requirement_id == "err.id"));

        let view = xml.replace(
            "</Impression>",
            "</Impression><ViewableImpression><Viewable id=\"v-1\"><![CDATA[https://t.example.com/view]]></Viewable></ViewableImpression>",
        );
        let mut view_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "view.elem".into(),
                    kind: "viewable_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["v-1".into()]),
                }],
                ..Default::default()
            },
            &view,
            &mut view_ok,
        );
        assert!(!view_ok.iter().any(|f| f.requirement_id == "view.elem"));

        let mut view_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "view.elem".into(),
                    kind: "viewable_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["v-2".into()]),
                }],
                ..Default::default()
            },
            &view,
            &mut view_fail,
        );
        assert!(view_fail.iter().any(|f| f.requirement_id == "view.elem"));
    }

    #[test]
    fn notviewable_id_viewundetermined_id_and_survey_id() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let nv = xml.replace(
            "</Impression>",
            "</Impression><ViewableImpression><NotViewable id=\"nv-1\"><![CDATA[https://t.example.com/nv]]></NotViewable></ViewableImpression>",
        );
        let mut nv_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nv.id".into(),
                    kind: "notviewable_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["nv-1".into()]),
                }],
                ..Default::default()
            },
            &nv,
            &mut nv_ok,
        );
        assert!(!nv_ok.iter().any(|f| f.requirement_id == "nv.id"));

        let mut nv_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nv.id".into(),
                    kind: "notviewable_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["nv-2".into()]),
                }],
                ..Default::default()
            },
            &nv,
            &mut nv_fail,
        );
        assert!(nv_fail.iter().any(|f| f.requirement_id == "nv.id"));

        let vu = xml.replace(
            "</Impression>",
            "</Impression><ViewableImpression><ViewUndetermined id=\"vu-1\"><![CDATA[https://t.example.com/vu]]></ViewUndetermined></ViewableImpression>",
        );
        let mut vu_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "vu.id".into(),
                    kind: "viewundetermined_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["vu-1".into()]),
                }],
                ..Default::default()
            },
            &vu,
            &mut vu_ok,
        );
        assert!(!vu_ok.iter().any(|f| f.requirement_id == "vu.id"));

        let mut vu_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "vu.id".into(),
                    kind: "viewundetermined_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["vu-2".into()]),
                }],
                ..Default::default()
            },
            &vu,
            &mut vu_fail,
        );
        assert!(vu_fail.iter().any(|f| f.requirement_id == "vu.id"));

        let surv = xml.replace(
            "</AdTitle>",
            "</AdTitle><Survey id=\"surv-1\"><![CDATA[https://t.example.com/survey]]></Survey>",
        );
        let mut surv_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "surv.id".into(),
                    kind: "survey_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["surv-1".into()]),
                }],
                ..Default::default()
            },
            &surv,
            &mut surv_ok,
        );
        assert!(!surv_ok.iter().any(|f| f.requirement_id == "surv.id"));

        let mut surv_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "surv.id".into(),
                    kind: "survey_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["surv-2".into()]),
                }],
                ..Default::default()
            },
            &surv,
            &mut surv_fail,
        );
        assert!(surv_fail.iter().any(|f| f.requirement_id == "surv.id"));
    }

    #[test]
    fn skipoffset_duration_and_expires() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let skip = xml.replace("<Linear>", "<Linear skipoffset=\"00:00:05\">");
        let mut skip_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "skip.off".into(),
                    kind: "skipoffset".into(),
                    message: "need skipoffset".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["00:00:05".into()]),
                }],
                ..Default::default()
            },
            &skip,
            &mut skip_ok,
        );
        assert!(!skip_ok.iter().any(|f| f.requirement_id == "skip.off"));

        let mut skip_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "skip.off".into(),
                    kind: "skipoffset".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["00:00:10".into()]),
                }],
                ..Default::default()
            },
            &skip,
            &mut skip_fail,
        );
        assert!(skip_fail.iter().any(|f| f.requirement_id == "skip.off"));

        let mut dur_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "dur.txt".into(),
                    kind: "duration".into(),
                    message: "need duration".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["00:00:30".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut dur_ok,
        );
        assert!(!dur_ok.iter().any(|f| f.requirement_id == "dur.txt"));

        let mut dur_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "dur.txt".into(),
                    kind: "duration".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["00:00:15".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut dur_fail,
        );
        assert!(dur_fail.iter().any(|f| f.requirement_id == "dur.txt"));

        let mut exp_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "exp.txt".into(),
                    kind: "expires".into(),
                    message: "need expires".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["3600".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut exp_ok,
        );
        assert!(!exp_ok.iter().any(|f| f.requirement_id == "exp.txt"));

        let mut exp_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "exp.txt".into(),
                    kind: "expires".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["7200".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut exp_fail,
        );
        assert!(exp_fail.iter().any(|f| f.requirement_id == "exp.txt"));
    }

    #[test]
    fn icon_duration_icon_offset_and_min_suggested_duration() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let icon_dur = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" duration=\"00:00:15\"></Icon></Icons></Linear>",
        );
        let mut icon_dur_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icon.dur".into(),
                    kind: "icon_duration".into(),
                    message: "need duration".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["00:00:15".into()]),
                }],
                ..Default::default()
            },
            &icon_dur,
            &mut icon_dur_ok,
        );
        assert!(!icon_dur_ok.iter().any(|f| f.requirement_id == "icon.dur"));

        let mut icon_dur_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icon.dur".into(),
                    kind: "icon_duration".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["00:00:30".into()]),
                }],
                ..Default::default()
            },
            &icon_dur,
            &mut icon_dur_fail,
        );
        assert!(icon_dur_fail.iter().any(|f| f.requirement_id == "icon.dur"));

        let icon_off = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\" offset=\"00:00:00\"></Icon></Icons></Linear>",
        );
        let mut icon_off_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icon.off".into(),
                    kind: "icon_offset".into(),
                    message: "need offset".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["00:00:00".into()]),
                }],
                ..Default::default()
            },
            &icon_off,
            &mut icon_off_ok,
        );
        assert!(!icon_off_ok.iter().any(|f| f.requirement_id == "icon.off"));

        let mut icon_off_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icon.off".into(),
                    kind: "icon_offset".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["00:00:05".into()]),
                }],
                ..Default::default()
            },
            &icon_off,
            &mut icon_off_fail,
        );
        assert!(icon_off_fail.iter().any(|f| f.requirement_id == "icon.off"));

        let msd = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\" height=\"250\" minSuggestedDuration=\"00:00:15\"></NonLinear></NonLinearAds>",
        );
        let mut msd_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.msd".into(),
                    kind: "min_suggested_duration".into(),
                    message: "need duration".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["00:00:15".into()]),
                }],
                ..Default::default()
            },
            &msd,
            &mut msd_ok,
        );
        assert!(!msd_ok.iter().any(|f| f.requirement_id == "nl.msd"));

        let mut msd_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.msd".into(),
                    kind: "min_suggested_duration".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["00:00:30".into()]),
                }],
                ..Default::default()
            },
            &msd,
            &mut msd_fail,
        );
        assert!(msd_fail.iter().any(|f| f.requirement_id == "nl.msd"));
    }

    #[test]
    fn ad_serving_id_ad_title_and_description() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut asid_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "asid.txt".into(),
                    kind: "ad_serving_id".into(),
                    message: "need id".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["srv-001".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut asid_ok,
        );
        assert!(!asid_ok.iter().any(|f| f.requirement_id == "asid.txt"));

        let mut asid_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "asid.txt".into(),
                    kind: "ad_serving_id".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["srv-002".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut asid_fail,
        );
        assert!(asid_fail.iter().any(|f| f.requirement_id == "asid.txt"));

        let mut title_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "title.txt".into(),
                    kind: "ad_title".into(),
                    message: "need title".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["Test".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut title_ok,
        );
        assert!(!title_ok.iter().any(|f| f.requirement_id == "title.txt"));

        let mut title_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "title.txt".into(),
                    kind: "ad_title".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["Other".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut title_fail,
        );
        assert!(title_fail.iter().any(|f| f.requirement_id == "title.txt"));

        let desc = xml.replace(
            "</AdTitle>",
            "</AdTitle><Description>A test ad</Description>",
        );
        let mut desc_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "desc.txt".into(),
                    kind: "description".into(),
                    message: "need description".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["A test ad".into()]),
                }],
                ..Default::default()
            },
            &desc,
            &mut desc_ok,
        );
        assert!(!desc_ok.iter().any(|f| f.requirement_id == "desc.txt"));

        let mut desc_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "desc.txt".into(),
                    kind: "description".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["Other".into()]),
                }],
                ..Default::default()
            },
            &desc,
            &mut desc_fail,
        );
        assert!(desc_fail.iter().any(|f| f.requirement_id == "desc.txt"));
    }

    #[test]
    fn ad_parameters_static_and_flash_creative_type() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let params = xml.replace(
            "</Duration>",
            "</Duration><AdParameters>foo=1</AdParameters>",
        );
        let mut params_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "params.txt".into(),
                    kind: "ad_parameters".into(),
                    message: "need params".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["foo=1".into()]),
                }],
                ..Default::default()
            },
            &params,
            &mut params_ok,
        );
        assert!(!params_ok.iter().any(|f| f.requirement_id == "params.txt"));

        let mut params_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "params.txt".into(),
                    kind: "ad_parameters".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["bar=2".into()]),
                }],
                ..Default::default()
            },
            &params,
            &mut params_fail,
        );
        assert!(params_fail.iter().any(|f| f.requirement_id == "params.txt"));

        let static_ct = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><StaticResource creativeType=\"image/png\"><![CDATA[https://cdn.example.com/i.png]]></StaticResource></Companion></CompanionAds></Creative>",
        );
        let mut static_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "static.ct".into(),
                    kind: "static_resource_creative_type".into(),
                    message: "need type".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["image/png".into()]),
                }],
                ..Default::default()
            },
            &static_ct,
            &mut static_ok,
        );
        assert!(!static_ok.iter().any(|f| f.requirement_id == "static.ct"));

        let mut static_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "static.ct".into(),
                    kind: "static_resource_creative_type".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["image/jpeg".into()]),
                }],
                ..Default::default()
            },
            &static_ct,
            &mut static_fail,
        );
        assert!(static_fail.iter().any(|f| f.requirement_id == "static.ct"));

        let flash_ct = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><FlashResource creativeType=\"application/x-shockwave-flash\"><![CDATA[https://cdn.example.com/i.swf]]></FlashResource></Companion></CompanionAds></Creative>",
        );
        let mut flash_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "flash.ct".into(),
                    kind: "flash_resource_creative_type".into(),
                    message: "need type".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["application/x-shockwave-flash".into()]),
                }],
                ..Default::default()
            },
            &flash_ct,
            &mut flash_ok,
        );
        assert!(!flash_ok.iter().any(|f| f.requirement_id == "flash.ct"));

        let mut flash_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "flash.ct".into(),
                    kind: "flash_resource_creative_type".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["application/x-shockwave-flash2".into()]),
                }],
                ..Default::default()
            },
            &flash_ct,
            &mut flash_fail,
        );
        assert!(flash_fail.iter().any(|f| f.requirement_id == "flash.ct"));
    }

    #[test]
    fn impression_error_and_clickthrough() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut imp_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "imp.url".into(),
                    kind: "impression".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec![
                        "https://t.example.com/imp?cb=[CACHEBUSTING]&ts=[TIMESTAMP]".into(),
                    ]),
                }],
                ..Default::default()
            },
            xml,
            &mut imp_ok,
        );
        assert!(!imp_ok.iter().any(|f| f.requirement_id == "imp.url"));

        let mut imp_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "imp.url".into(),
                    kind: "impression".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut imp_fail,
        );
        assert!(imp_fail.iter().any(|f| f.requirement_id == "imp.url"));

        let err = xml.replace(
            "</AdTitle>",
            "</AdTitle><Error><![CDATA[https://t.example.com/error]]></Error>",
        );
        let mut err_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "err.url".into(),
                    kind: "error".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/error".into()]),
                }],
                ..Default::default()
            },
            &err,
            &mut err_ok,
        );
        assert!(!err_ok.iter().any(|f| f.requirement_id == "err.url"));

        let mut err_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "err.url".into(),
                    kind: "error".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &err,
            &mut err_fail,
        );
        assert!(err_fail.iter().any(|f| f.requirement_id == "err.url"));

        let clk = xml.replace(
            "</Linear>",
            "<VideoClicks><ClickThrough><![CDATA[https://t.example.com/click]]></ClickThrough></VideoClicks></Linear>",
        );
        let mut clk_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "clk.url".into(),
                    kind: "clickthrough".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/click".into()]),
                }],
                ..Default::default()
            },
            &clk,
            &mut clk_ok,
        );
        assert!(!clk_ok.iter().any(|f| f.requirement_id == "clk.url"));

        let mut clk_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "clk.url".into(),
                    kind: "clickthrough".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &clk,
            &mut clk_fail,
        );
        assert!(clk_fail.iter().any(|f| f.requirement_id == "clk.url"));
    }

    #[test]
    fn survey_tracking_and_viewable() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let surv = xml.replace(
            "</AdTitle>",
            "</AdTitle><Survey><![CDATA[https://t.example.com/survey]]></Survey>",
        );
        let mut surv_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "surv.url".into(),
                    kind: "survey".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/survey".into()]),
                }],
                ..Default::default()
            },
            &surv,
            &mut surv_ok,
        );
        assert!(!surv_ok.iter().any(|f| f.requirement_id == "surv.url"));

        let mut surv_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "surv.url".into(),
                    kind: "survey".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &surv,
            &mut surv_fail,
        );
        assert!(surv_fail.iter().any(|f| f.requirement_id == "surv.url"));

        let mut trk_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "trk.url".into(),
                    kind: "tracking".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/start".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut trk_ok,
        );
        assert!(!trk_ok.iter().any(|f| f.requirement_id == "trk.url"));

        let mut trk_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "trk.url".into(),
                    kind: "tracking".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut trk_fail,
        );
        assert!(trk_fail.iter().any(|f| f.requirement_id == "trk.url"));

        let view = xml.replace(
            "</Impression>",
            "</Impression><ViewableImpression><Viewable><![CDATA[https://t.example.com/view]]></Viewable></ViewableImpression>",
        );
        let mut view_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "view.url".into(),
                    kind: "viewable".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/view".into()]),
                }],
                ..Default::default()
            },
            &view,
            &mut view_ok,
        );
        assert!(!view_ok.iter().any(|f| f.requirement_id == "view.url"));

        let mut view_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "view.url".into(),
                    kind: "viewable".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &view,
            &mut view_fail,
        );
        assert!(view_fail.iter().any(|f| f.requirement_id == "view.url"));
    }

    #[test]
    fn clicktracking_mezzanine_and_mediafile() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let clk = xml.replace(
            "</Linear>",
            "<VideoClicks><ClickTracking><![CDATA[https://t.example.com/clk]]></ClickTracking></VideoClicks></Linear>",
        );
        let mut clk_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "clk.trk.url".into(),
                    kind: "clicktracking".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/clk".into()]),
                }],
                ..Default::default()
            },
            &clk,
            &mut clk_ok,
        );
        assert!(!clk_ok.iter().any(|f| f.requirement_id == "clk.trk.url"));

        let mut clk_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "clk.trk.url".into(),
                    kind: "clicktracking".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &clk,
            &mut clk_fail,
        );
        assert!(clk_fail.iter().any(|f| f.requirement_id == "clk.trk.url"));

        let mezz = xml.replace(
            "</MediaFile>",
            "</MediaFile><Mezzanine><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        let mut mezz_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.url".into(),
                    kind: "mezzanine".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/mezz.mp4".into()]),
                }],
                ..Default::default()
            },
            &mezz,
            &mut mezz_ok,
        );
        assert!(!mezz_ok.iter().any(|f| f.requirement_id == "mezz.url"));

        let mut mezz_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.url".into(),
                    kind: "mezzanine".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/other.mp4".into()]),
                }],
                ..Default::default()
            },
            &mezz,
            &mut mezz_fail,
        );
        assert!(mezz_fail.iter().any(|f| f.requirement_id == "mezz.url"));

        let mut mf_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.url".into(),
                    kind: "mediafile".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/ad.mp4".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut mf_ok,
        );
        assert!(!mf_ok.iter().any(|f| f.requirement_id == "mf.url"));

        let mut mf_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.url".into(),
                    kind: "mediafile".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/other.mp4".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut mf_fail,
        );
        assert!(mf_fail.iter().any(|f| f.requirement_id == "mf.url"));
    }

    #[test]
    fn vast_ad_tag_uri_javascript_resource_and_closed_caption_file() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let tag = r#"<VAST><Ad><Wrapper><VASTAdTagURI><![CDATA[https://example.com/vast]]></VASTAdTagURI></Wrapper></Ad></VAST>"#;
        let mut tag_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "tag.uri".into(),
                    kind: "vast_ad_tag_uri".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://example.com/vast".into()]),
                }],
                ..Default::default()
            },
            tag,
            &mut tag_ok,
        );
        assert!(!tag_ok.iter().any(|f| f.requirement_id == "tag.uri"));

        let mut tag_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "tag.uri".into(),
                    kind: "vast_ad_tag_uri".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://example.com/other".into()]),
                }],
                ..Default::default()
            },
            tag,
            &mut tag_fail,
        );
        assert!(tag_fail.iter().any(|f| f.requirement_id == "tag.uri"));

        let js = xml.replace(
            "</Impression>",
            "</Impression><AdVerifications><Verification vendor=\"omid\"><JavaScriptResource><![CDATA[https://t.example.com/omid.js]]></JavaScriptResource></Verification></AdVerifications>",
        );
        let mut js_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "js.url".into(),
                    kind: "javascript_resource".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/omid.js".into()]),
                }],
                ..Default::default()
            },
            &js,
            &mut js_ok,
        );
        assert!(!js_ok.iter().any(|f| f.requirement_id == "js.url"));

        let mut js_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "js.url".into(),
                    kind: "javascript_resource".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other.js".into()]),
                }],
                ..Default::default()
            },
            &js,
            &mut js_fail,
        );
        assert!(js_fail.iter().any(|f| f.requirement_id == "js.url"));

        let cc = xml.replace(
            "</Linear>",
            "<ClosedCaptionFiles><ClosedCaptionFile><![CDATA[https://cdn.example.com/cap.vtt]]></ClosedCaptionFile></ClosedCaptionFiles></Linear>",
        );
        let mut cc_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cc.url".into(),
                    kind: "closed_caption_file".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/cap.vtt".into()]),
                }],
                ..Default::default()
            },
            &cc,
            &mut cc_ok,
        );
        assert!(!cc_ok.iter().any(|f| f.requirement_id == "cc.url"));

        let mut cc_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cc.url".into(),
                    kind: "closed_caption_file".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/other.vtt".into()]),
                }],
                ..Default::default()
            },
            &cc,
            &mut cc_fail,
        );
        assert!(cc_fail.iter().any(|f| f.requirement_id == "cc.url"));
    }

    #[test]
    fn executable_resource_icon_clickthrough_and_companion_clickthrough() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let exec = xml.replace(
            "</Impression>",
            "</Impression><AdVerifications><Verification vendor=\"omid\"><ExecutableResource><![CDATA[https://t.example.com/omid.exe]]></ExecutableResource></Verification></AdVerifications>",
        );
        let mut exec_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "exec.url".into(),
                    kind: "executable_resource".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/omid.exe".into()]),
                }],
                ..Default::default()
            },
            &exec,
            &mut exec_ok,
        );
        assert!(!exec_ok.iter().any(|f| f.requirement_id == "exec.url"));

        let mut exec_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "exec.url".into(),
                    kind: "executable_resource".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other.exe".into()]),
                }],
                ..Default::default()
            },
            &exec,
            &mut exec_fail,
        );
        assert!(exec_fail.iter().any(|f| f.requirement_id == "exec.url"));

        let ict = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconClicks><IconClickThrough><![CDATA[https://adv.example.com/i]]></IconClickThrough></IconClicks></Icon></Icons></Linear>",
        );
        let mut ict_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ict.url".into(),
                    kind: "icon_clickthrough".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://adv.example.com/i".into()]),
                }],
                ..Default::default()
            },
            &ict,
            &mut ict_ok,
        );
        assert!(!ict_ok.iter().any(|f| f.requirement_id == "ict.url"));

        let mut ict_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ict.url".into(),
                    kind: "icon_clickthrough".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://adv.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &ict,
            &mut ict_fail,
        );
        assert!(ict_fail.iter().any(|f| f.requirement_id == "ict.url"));

        let cct = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><CompanionClickThrough><![CDATA[https://adv.example.com/c]]></CompanionClickThrough></Companion></CompanionAds></Creative>",
        );
        let mut cct_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cct.url".into(),
                    kind: "companion_clickthrough".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://adv.example.com/c".into()]),
                }],
                ..Default::default()
            },
            &cct,
            &mut cct_ok,
        );
        assert!(!cct_ok.iter().any(|f| f.requirement_id == "cct.url"));

        let mut cct_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cct.url".into(),
                    kind: "companion_clickthrough".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://adv.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &cct,
            &mut cct_fail,
        );
        assert!(cct_fail.iter().any(|f| f.requirement_id == "cct.url"));
    }

    #[test]
    fn nonlinear_clickthrough_icon_viewtracking_and_interactive_creative_file() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let nlc = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\" height=\"250\"><NonLinearClickThrough><![CDATA[https://adv.example.com/nl]]></NonLinearClickThrough></NonLinear></NonLinearAds>",
        );
        let mut nlc_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nlc.url".into(),
                    kind: "nonlinear_clickthrough".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://adv.example.com/nl".into()]),
                }],
                ..Default::default()
            },
            &nlc,
            &mut nlc_ok,
        );
        assert!(!nlc_ok.iter().any(|f| f.requirement_id == "nlc.url"));

        let mut nlc_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nlc.url".into(),
                    kind: "nonlinear_clickthrough".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://adv.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &nlc,
            &mut nlc_fail,
        );
        assert!(nlc_fail.iter().any(|f| f.requirement_id == "nlc.url"));

        let ivt = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconViewTracking><![CDATA[https://t.example.com/iv]]></IconViewTracking></Icon></Icons></Linear>",
        );
        let mut ivt_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ivt.url".into(),
                    kind: "icon_viewtracking".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/iv".into()]),
                }],
                ..Default::default()
            },
            &ivt,
            &mut ivt_ok,
        );
        assert!(!ivt_ok.iter().any(|f| f.requirement_id == "ivt.url"));

        let mut ivt_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ivt.url".into(),
                    kind: "icon_viewtracking".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &ivt,
            &mut ivt_fail,
        );
        assert!(ivt_fail.iter().any(|f| f.requirement_id == "ivt.url"));

        let icf = xml.replace(
            "</Linear>",
            "</Linear><InteractiveCreativeFile type=\"application/javascript\"><![CDATA[https://cdn.example.com/v.js]]></InteractiveCreativeFile>",
        );
        let mut icf_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icf.url".into(),
                    kind: "interactive_creative_file".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/v.js".into()]),
                }],
                ..Default::default()
            },
            &icf,
            &mut icf_ok,
        );
        assert!(!icf_ok.iter().any(|f| f.requirement_id == "icf.url"));

        let mut icf_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icf.url".into(),
                    kind: "interactive_creative_file".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/other.js".into()]),
                }],
                ..Default::default()
            },
            &icf,
            &mut icf_fail,
        );
        assert!(icf_fail.iter().any(|f| f.requirement_id == "icf.url"));
    }

    #[test]
    fn static_resource_iframe_resource_and_html_resource() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let st = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><StaticResource creativeType=\"image/png\"><![CDATA[https://cdn.example.com/i.png]]></StaticResource></Companion></CompanionAds></Creative>",
        );
        let mut st_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "static.url".into(),
                    kind: "static_resource".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/i.png".into()]),
                }],
                ..Default::default()
            },
            &st,
            &mut st_ok,
        );
        assert!(!st_ok.iter().any(|f| f.requirement_id == "static.url"));

        let mut st_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "static.url".into(),
                    kind: "static_resource".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/other.png".into()]),
                }],
                ..Default::default()
            },
            &st,
            &mut st_fail,
        );
        assert!(st_fail.iter().any(|f| f.requirement_id == "static.url"));

        let iframe = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><IFrameResource><![CDATA[https://cdn.example.com/frame.html]]></IFrameResource></Companion></CompanionAds></Creative>",
        );
        let mut iframe_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "iframe.url".into(),
                    kind: "iframe_resource".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/frame.html".into()]),
                }],
                ..Default::default()
            },
            &iframe,
            &mut iframe_ok,
        );
        assert!(!iframe_ok.iter().any(|f| f.requirement_id == "iframe.url"));

        let mut iframe_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "iframe.url".into(),
                    kind: "iframe_resource".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/other.html".into()]),
                }],
                ..Default::default()
            },
            &iframe,
            &mut iframe_fail,
        );
        assert!(iframe_fail.iter().any(|f| f.requirement_id == "iframe.url"));

        let html = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><HTMLResource><![CDATA[https://cdn.example.com/ad.html]]></HTMLResource></Companion></CompanionAds></Creative>",
        );
        let mut html_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "html.url".into(),
                    kind: "html_resource".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/ad.html".into()]),
                }],
                ..Default::default()
            },
            &html,
            &mut html_ok,
        );
        assert!(!html_ok.iter().any(|f| f.requirement_id == "html.url"));

        let mut html_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "html.url".into(),
                    kind: "html_resource".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/other.html".into()]),
                }],
                ..Default::default()
            },
            &html,
            &mut html_fail,
        );
        assert!(html_fail.iter().any(|f| f.requirement_id == "html.url"));
    }

    #[test]
    fn flash_resource_icon_clickfallback_and_custom_click() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let flash = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><FlashResource><![CDATA[https://cdn.example.com/ad.swf]]></FlashResource></Companion></CompanionAds></Creative>",
        );
        let mut flash_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "flash.url".into(),
                    kind: "flash_resource".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/ad.swf".into()]),
                }],
                ..Default::default()
            },
            &flash,
            &mut flash_ok,
        );
        assert!(!flash_ok.iter().any(|f| f.requirement_id == "flash.url"));

        let mut flash_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "flash.url".into(),
                    kind: "flash_resource".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/other.swf".into()]),
                }],
                ..Default::default()
            },
            &flash,
            &mut flash_fail,
        );
        assert!(flash_fail.iter().any(|f| f.requirement_id == "flash.url"));

        let icfb = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconClicks><IconClickFallbackImages><IconClickFallbackImage><![CDATA[https://cdn.example.com/icon.png]]></IconClickFallbackImage></IconClickFallbackImages></IconClicks></Icon></Icons></Linear>",
        );
        let mut icfb_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icfb.url".into(),
                    kind: "icon_clickfallback".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/icon.png".into()]),
                }],
                ..Default::default()
            },
            &icfb,
            &mut icfb_ok,
        );
        assert!(!icfb_ok.iter().any(|f| f.requirement_id == "icfb.url"));

        let mut icfb_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icfb.url".into(),
                    kind: "icon_clickfallback".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://cdn.example.com/other.png".into()]),
                }],
                ..Default::default()
            },
            &icfb,
            &mut icfb_fail,
        );
        assert!(icfb_fail.iter().any(|f| f.requirement_id == "icfb.url"));

        let custom = xml.replace(
            "</Duration>",
            "</Duration><VideoClicks><CustomClick><![CDATA[https://t.example.com/custom]]></CustomClick></VideoClicks>",
        );
        let mut custom_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "custom.url".into(),
                    kind: "custom_click".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/custom".into()]),
                }],
                ..Default::default()
            },
            &custom,
            &mut custom_ok,
        );
        assert!(!custom_ok.iter().any(|f| f.requirement_id == "custom.url"));

        let mut custom_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "custom.url".into(),
                    kind: "custom_click".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &custom,
            &mut custom_fail,
        );
        assert!(custom_fail.iter().any(|f| f.requirement_id == "custom.url"));
    }

    #[test]
    fn icon_clicktracking_companion_clicktracking_and_nonlinear_clicktracking() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let ictk = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconClicks><IconClickTracking><![CDATA[https://t.example.com/i]]></IconClickTracking></IconClicks></Icon></Icons></Linear>",
        );
        let mut ictk_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ictk.url".into(),
                    kind: "icon_clicktracking".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/i".into()]),
                }],
                ..Default::default()
            },
            &ictk,
            &mut ictk_ok,
        );
        assert!(!ictk_ok.iter().any(|f| f.requirement_id == "ictk.url"));

        let mut ictk_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ictk.url".into(),
                    kind: "icon_clicktracking".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &ictk,
            &mut ictk_fail,
        );
        assert!(ictk_fail.iter().any(|f| f.requirement_id == "ictk.url"));

        let cctk = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><CompanionClickTracking><![CDATA[https://t.example.com/c]]></CompanionClickTracking></Companion></CompanionAds></Creative>",
        );
        let mut cctk_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cctk.url".into(),
                    kind: "companion_clicktracking".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/c".into()]),
                }],
                ..Default::default()
            },
            &cctk,
            &mut cctk_ok,
        );
        assert!(!cctk_ok.iter().any(|f| f.requirement_id == "cctk.url"));

        let mut cctk_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cctk.url".into(),
                    kind: "companion_clicktracking".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &cctk,
            &mut cctk_fail,
        );
        assert!(cctk_fail.iter().any(|f| f.requirement_id == "cctk.url"));

        let nlct = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\" height=\"250\"><NonLinearClickTracking><![CDATA[https://t.example.com/nl]]></NonLinearClickTracking></NonLinear></NonLinearAds>",
        );
        let mut nlct_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nlct.url".into(),
                    kind: "nonlinear_clicktracking".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/nl".into()]),
                }],
                ..Default::default()
            },
            &nlct,
            &mut nlct_ok,
        );
        assert!(!nlct_ok.iter().any(|f| f.requirement_id == "nlct.url"));

        let mut nlct_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nlct.url".into(),
                    kind: "nonlinear_clicktracking".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &nlct,
            &mut nlct_fail,
        );
        assert!(nlct_fail.iter().any(|f| f.requirement_id == "nlct.url"));
    }

    #[test]
    fn notviewable_viewundetermined_and_extension() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let nv = xml.replace(
            "</Impression>",
            "</Impression><ViewableImpression><NotViewable><![CDATA[https://t.example.com/nv]]></NotViewable></ViewableImpression>",
        );
        let mut nv_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nv.url".into(),
                    kind: "notviewable".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/nv".into()]),
                }],
                ..Default::default()
            },
            &nv,
            &mut nv_ok,
        );
        assert!(!nv_ok.iter().any(|f| f.requirement_id == "nv.url"));

        let mut nv_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nv.url".into(),
                    kind: "notviewable".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &nv,
            &mut nv_fail,
        );
        assert!(nv_fail.iter().any(|f| f.requirement_id == "nv.url"));

        let vu = xml.replace(
            "</Impression>",
            "</Impression><ViewableImpression><ViewUndetermined><![CDATA[https://t.example.com/vu]]></ViewUndetermined></ViewableImpression>",
        );
        let mut vu_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "vu.url".into(),
                    kind: "viewundetermined".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/vu".into()]),
                }],
                ..Default::default()
            },
            &vu,
            &mut vu_ok,
        );
        assert!(!vu_ok.iter().any(|f| f.requirement_id == "vu.url"));

        let mut vu_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "vu.url".into(),
                    kind: "viewundetermined".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &vu,
            &mut vu_fail,
        );
        assert!(vu_fail.iter().any(|f| f.requirement_id == "vu.url"));

        let ext = xml.replace(
            "</InLine>",
            "<Extensions><Extension><![CDATA[https://t.example.com/ext]]></Extension></Extensions></InLine>",
        );
        let mut ext_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ext.url".into(),
                    kind: "extension".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/ext".into()]),
                }],
                ..Default::default()
            },
            &ext,
            &mut ext_ok,
        );
        assert!(!ext_ok.iter().any(|f| f.requirement_id == "ext.url"));

        let mut ext_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ext.url".into(),
                    kind: "extension".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &ext,
            &mut ext_fail,
        );
        assert!(ext_fail.iter().any(|f| f.requirement_id == "ext.url"));
    }

    #[test]
    fn verification_parameters_blocked_ad_categories_and_creative_extension() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let vp = xml.replace(
            "</InLine>",
            "<AdVerifications><Verification vendor=\"omid\"><VerificationParameters><![CDATA[https://t.example.com/omid]]></VerificationParameters></Verification></AdVerifications></InLine>",
        );
        let mut vp_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "vp.url".into(),
                    kind: "verification_parameters".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/omid".into()]),
                }],
                ..Default::default()
            },
            &vp,
            &mut vp_ok,
        );
        assert!(!vp_ok.iter().any(|f| f.requirement_id == "vp.url"));

        let mut vp_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "vp.url".into(),
                    kind: "verification_parameters".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &vp,
            &mut vp_fail,
        );
        assert!(vp_fail.iter().any(|f| f.requirement_id == "vp.url"));

        let blocked = r#"<VAST><Ad><Wrapper><VASTAdTagURI><![CDATA[https://example.com/vast]]></VASTAdTagURI><BlockedAdCategories><![CDATA[IAB1-1]]></BlockedAdCategories></Wrapper></Ad></VAST>"#;
        let mut blocked_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "blocked.cat".into(),
                    kind: "blocked_ad_categories".into(),
                    message: "need cat".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["IAB1-1".into()]),
                }],
                ..Default::default()
            },
            blocked,
            &mut blocked_ok,
        );
        assert!(!blocked_ok.iter().any(|f| f.requirement_id == "blocked.cat"));

        let mut blocked_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "blocked.cat".into(),
                    kind: "blocked_ad_categories".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["IAB2-2".into()]),
                }],
                ..Default::default()
            },
            blocked,
            &mut blocked_fail,
        );
        assert!(blocked_fail
            .iter()
            .any(|f| f.requirement_id == "blocked.cat"));

        let cext = xml.replace(
            "</Creative>",
            "<CreativeExtensions><CreativeExtension><![CDATA[https://t.example.com/cext]]></CreativeExtension></CreativeExtensions></Creative>",
        );
        let mut cext_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cext.url".into(),
                    kind: "creative_extension".into(),
                    message: "need url".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/cext".into()]),
                }],
                ..Default::default()
            },
            &cext,
            &mut cext_ok,
        );
        assert!(!cext_ok.iter().any(|f| f.requirement_id == "cext.url"));

        let mut cext_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cext.url".into(),
                    kind: "creative_extension".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["https://t.example.com/other".into()]),
                }],
                ..Default::default()
            },
            &cext,
            &mut cext_fail,
        );
        assert!(cext_fail.iter().any(|f| f.requirement_id == "cext.url"));
    }

    #[test]
    fn creative_extension_type_id_value_and_mediafile_codec() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let cext = xml.replace(
            "</Creative>",
            "<CreativeExtensions><CreativeExtension type=\"iab\"></CreativeExtension></CreativeExtensions></Creative>",
        );
        let mut cext_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cext.type".into(),
                    kind: "creative_extension_type".into(),
                    message: "need type".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["iab".into()]),
                }],
                ..Default::default()
            },
            &cext,
            &mut cext_ok,
        );
        assert!(!cext_ok.iter().any(|f| f.requirement_id == "cext.type"));

        let mut cext_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cext.type".into(),
                    kind: "creative_extension_type".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["geo".into()]),
                }],
                ..Default::default()
            },
            &cext,
            &mut cext_fail,
        );
        assert!(cext_fail.iter().any(|f| f.requirement_id == "cext.type"));

        let idv = xml.replace(
            "<UniversalAdId idRegistry=\"ad-id.org\">ABCD1234567</UniversalAdId>",
            "<UniversalAdId idRegistry=\"ad-id.org\" idValue=\"FROM-ATTR\"></UniversalAdId>",
        );
        let mut idv_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "uad.idvalue".into(),
                    kind: "id_value".into(),
                    message: "need idValue".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["FROM-ATTR".into()]),
                }],
                ..Default::default()
            },
            &idv,
            &mut idv_ok,
        );
        assert!(!idv_ok.iter().any(|f| f.requirement_id == "uad.idvalue"));

        let mut idv_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "uad.idvalue".into(),
                    kind: "id_value".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["OTHER".into()]),
                }],
                ..Default::default()
            },
            &idv,
            &mut idv_fail,
        );
        assert!(idv_fail.iter().any(|f| f.requirement_id == "uad.idvalue"));

        let mut mf_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.codec".into(),
                    kind: "mediafile_codec".into(),
                    message: "need codec".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["avc1.42E01E,mp4a.40.2".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut mf_ok,
        );
        assert!(!mf_ok.iter().any(|f| f.requirement_id == "mf.codec"));

        let mut mf_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.codec".into(),
                    kind: "mediafile_codec".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["hev1".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut mf_fail,
        );
        assert!(mf_fail.iter().any(|f| f.requirement_id == "mf.codec"));
    }

    #[test]
    fn max_width_and_max_height() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "size.max-w".into(),
                        kind: "max_width".into(),
                        message: "need width cap".into(),
                        element: None,
                        max: Some(640),
                        min: None,
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "size.max-h".into(),
                        kind: "max_height".into(),
                        message: "need height cap".into(),
                        element: None,
                        max: Some(360),
                        min: None,
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            xml,
            &mut ok,
        );
        assert!(!ok.iter().any(|f| f.requirement_id == "size.max-w"));
        assert!(!ok.iter().any(|f| f.requirement_id == "size.max-h"));

        let over = xml.replace(
            "width=\"640\" height=\"360\"",
            "width=\"1920\" height=\"1080\"",
        );
        let mut fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "size.max-w".into(),
                        kind: "max_width".into(),
                        message: "need width cap".into(),
                        element: None,
                        max: Some(640),
                        min: None,
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "size.max-h".into(),
                        kind: "max_height".into(),
                        message: "need height cap".into(),
                        element: None,
                        max: Some(360),
                        min: None,
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &over,
            &mut fail,
        );
        assert!(fail.iter().any(|f| f.requirement_id == "size.max-w"));
        assert!(fail.iter().any(|f| f.requirement_id == "size.max-h"));
    }

    #[test]
    fn creative_extension_notviewable_and_viewundetermined_https() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut missing_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "cext.https".into(),
                        kind: "creative_extension_https".into(),
                        message: "cext https".into(),
                        element: None,
                        max: None,
                        min: None,
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "nv.https".into(),
                        kind: "notviewable_https".into(),
                        message: "nv https".into(),
                        element: None,
                        max: None,
                        min: None,
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "vu.https".into(),
                        kind: "viewundetermined_https".into(),
                        message: "vu https".into(),
                        element: None,
                        max: None,
                        min: None,
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            xml,
            &mut missing_ok,
        );
        assert!(!missing_ok.iter().any(|f| f.requirement_id == "cext.https"));
        assert!(!missing_ok.iter().any(|f| f.requirement_id == "nv.https"));
        assert!(!missing_ok.iter().any(|f| f.requirement_id == "vu.https"));

        let cext = xml.replace(
            "</Creative>",
            "<CreativeExtensions><CreativeExtension><![CDATA[https://t.example.com/cext]]></CreativeExtension></CreativeExtensions></Creative>",
        );
        let mut cext_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cext.https".into(),
                    kind: "creative_extension_https".into(),
                    message: "cext https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &cext,
            &mut cext_ok,
        );
        assert!(!cext_ok.iter().any(|f| f.requirement_id == "cext.https"));

        let cext_http = xml.replace(
            "</Creative>",
            "<CreativeExtensions><CreativeExtension><![CDATA[http://t.example.com/cext]]></CreativeExtension></CreativeExtensions></Creative>",
        );
        let mut cext_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cext.https".into(),
                    kind: "creative_extension_https".into(),
                    message: "cext https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &cext_http,
            &mut cext_fail,
        );
        assert!(cext_fail.iter().any(|f| f.requirement_id == "cext.https"));

        let nv = xml.replace(
            "</Impression>",
            "</Impression><ViewableImpression><NotViewable><![CDATA[https://t.example.com/nv]]></NotViewable></ViewableImpression>",
        );
        let mut nv_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nv.https".into(),
                    kind: "notviewable_https".into(),
                    message: "nv https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &nv,
            &mut nv_ok,
        );
        assert!(!nv_ok.iter().any(|f| f.requirement_id == "nv.https"));

        let nv_http = xml.replace(
            "</Impression>",
            "</Impression><ViewableImpression><NotViewable><![CDATA[http://t.example.com/nv]]></NotViewable></ViewableImpression>",
        );
        let mut nv_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nv.https".into(),
                    kind: "notviewable_https".into(),
                    message: "nv https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &nv_http,
            &mut nv_fail,
        );
        assert!(nv_fail.iter().any(|f| f.requirement_id == "nv.https"));

        let vu = xml.replace(
            "</Impression>",
            "</Impression><ViewableImpression><ViewUndetermined><![CDATA[https://t.example.com/vu]]></ViewUndetermined></ViewableImpression>",
        );
        let mut vu_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "vu.https".into(),
                    kind: "viewundetermined_https".into(),
                    message: "vu https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &vu,
            &mut vu_ok,
        );
        assert!(!vu_ok.iter().any(|f| f.requirement_id == "vu.https"));

        let vu_http = xml.replace(
            "</Impression>",
            "</Impression><ViewableImpression><ViewUndetermined><![CDATA[http://t.example.com/vu]]></ViewUndetermined></ViewableImpression>",
        );
        let mut vu_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "vu.https".into(),
                    kind: "viewundetermined_https".into(),
                    message: "vu https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &vu_http,
            &mut vu_fail,
        );
        assert!(vu_fail.iter().any(|f| f.requirement_id == "vu.https"));
    }

    #[test]
    fn custom_click_icon_clicktracking_and_companion_clicktracking_https() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut missing_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "custom.https".into(),
                        kind: "custom_click_https".into(),
                        message: "custom https".into(),
                        element: None,
                        max: None,
                        min: None,
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "ictk.https".into(),
                        kind: "icon_clicktracking_https".into(),
                        message: "ictk https".into(),
                        element: None,
                        max: None,
                        min: None,
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "cctk.https".into(),
                        kind: "companion_clicktracking_https".into(),
                        message: "cctk https".into(),
                        element: None,
                        max: None,
                        min: None,
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            xml,
            &mut missing_ok,
        );
        assert!(!missing_ok
            .iter()
            .any(|f| f.requirement_id == "custom.https"));
        assert!(!missing_ok.iter().any(|f| f.requirement_id == "ictk.https"));
        assert!(!missing_ok.iter().any(|f| f.requirement_id == "cctk.https"));

        let custom = xml.replace(
            "</Duration>",
            "</Duration><VideoClicks><CustomClick><![CDATA[https://t.example.com/custom]]></CustomClick></VideoClicks>",
        );
        let mut custom_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "custom.https".into(),
                    kind: "custom_click_https".into(),
                    message: "custom https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &custom,
            &mut custom_ok,
        );
        assert!(!custom_ok.iter().any(|f| f.requirement_id == "custom.https"));

        let custom_http = xml.replace(
            "</Duration>",
            "</Duration><VideoClicks><CustomClick><![CDATA[http://t.example.com/custom]]></CustomClick></VideoClicks>",
        );
        let mut custom_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "custom.https".into(),
                    kind: "custom_click_https".into(),
                    message: "custom https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &custom_http,
            &mut custom_fail,
        );
        assert!(custom_fail
            .iter()
            .any(|f| f.requirement_id == "custom.https"));

        let ictk = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconClicks><IconClickTracking><![CDATA[https://t.example.com/i]]></IconClickTracking></IconClicks></Icon></Icons></Linear>",
        );
        let mut ictk_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ictk.https".into(),
                    kind: "icon_clicktracking_https".into(),
                    message: "ictk https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &ictk,
            &mut ictk_ok,
        );
        assert!(!ictk_ok.iter().any(|f| f.requirement_id == "ictk.https"));

        let ictk_http = xml.replace(
            "</Linear>",
            "<Icons><Icon program=\"adChoices\"><IconClicks><IconClickTracking><![CDATA[http://t.example.com/i]]></IconClickTracking></IconClicks></Icon></Icons></Linear>",
        );
        let mut ictk_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ictk.https".into(),
                    kind: "icon_clicktracking_https".into(),
                    message: "ictk https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &ictk_http,
            &mut ictk_fail,
        );
        assert!(ictk_fail.iter().any(|f| f.requirement_id == "ictk.https"));

        let cctk = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><CompanionClickTracking><![CDATA[https://t.example.com/c]]></CompanionClickTracking></Companion></CompanionAds></Creative>",
        );
        let mut cctk_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cctk.https".into(),
                    kind: "companion_clicktracking_https".into(),
                    message: "cctk https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &cctk,
            &mut cctk_ok,
        );
        assert!(!cctk_ok.iter().any(|f| f.requirement_id == "cctk.https"));

        let cctk_http = xml.replace(
            "</Creative>",
            "<CompanionAds><Companion width=\"300\" height=\"250\"><CompanionClickTracking><![CDATA[http://t.example.com/c]]></CompanionClickTracking></Companion></CompanionAds></Creative>",
        );
        let mut cctk_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "cctk.https".into(),
                    kind: "companion_clicktracking_https".into(),
                    message: "cctk https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &cctk_http,
            &mut cctk_fail,
        );
        assert!(cctk_fail.iter().any(|f| f.requirement_id == "cctk.https"));
    }

    #[test]
    fn nonlinear_clicktracking_https_blocked_authority_and_mediafile_minbitrate() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut missing_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nlct.https".into(),
                    kind: "nonlinear_clicktracking_https".into(),
                    message: "nlct https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            xml,
            &mut missing_ok,
        );
        assert!(!missing_ok.iter().any(|f| f.requirement_id == "nlct.https"));

        let nlct = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\" height=\"250\"><NonLinearClickTracking><![CDATA[https://t.example.com/nl]]></NonLinearClickTracking></NonLinear></NonLinearAds>",
        );
        let mut nlct_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nlct.https".into(),
                    kind: "nonlinear_clicktracking_https".into(),
                    message: "nlct https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &nlct,
            &mut nlct_ok,
        );
        assert!(!nlct_ok.iter().any(|f| f.requirement_id == "nlct.https"));

        let nlct_http = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\" height=\"250\"><NonLinearClickTracking><![CDATA[http://t.example.com/nl]]></NonLinearClickTracking></NonLinear></NonLinearAds>",
        );
        let mut nlct_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nlct.https".into(),
                    kind: "nonlinear_clicktracking_https".into(),
                    message: "nlct https".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &nlct_http,
            &mut nlct_fail,
        );
        assert!(nlct_fail.iter().any(|f| f.requirement_id == "nlct.https"));

        let blocked = r#"<VAST><Ad><Wrapper><VASTAdTagURI><![CDATA[https://example.com/vast]]></VASTAdTagURI><BlockedAdCategories authority="IAB"><![CDATA[IAB1-1]]></BlockedAdCategories></Wrapper></Ad></VAST>"#;
        let mut blocked_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "blocked.auth".into(),
                    kind: "blocked_ad_categories_authority".into(),
                    message: "need IAB".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["IAB".into()]),
                }],
                ..Default::default()
            },
            blocked,
            &mut blocked_ok,
        );
        assert!(!blocked_ok
            .iter()
            .any(|f| f.requirement_id == "blocked.auth"));

        let mut blocked_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "blocked.auth".into(),
                    kind: "blocked_ad_categories_authority".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["IABTECHLAB".into()]),
                }],
                ..Default::default()
            },
            blocked,
            &mut blocked_fail,
        );
        assert!(blocked_fail
            .iter()
            .any(|f| f.requirement_id == "blocked.auth"));

        let minbr = xml.replace("bitrate=\"1500\"", "bitrate=\"1500\" minBitrate=\"1000\"");
        let mut minbr_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.minbr".into(),
                    kind: "mediafile_minbitrate".into(),
                    message: "need minBitrate".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1000".into()]),
                }],
                ..Default::default()
            },
            &minbr,
            &mut minbr_ok,
        );
        assert!(!minbr_ok.iter().any(|f| f.requirement_id == "mf.minbr"));

        let mut minbr_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.minbr".into(),
                    kind: "mediafile_minbitrate".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["2000".into()]),
                }],
                ..Default::default()
            },
            &minbr,
            &mut minbr_fail,
        );
        assert!(minbr_fail.iter().any(|f| f.requirement_id == "mf.minbr"));
    }

    #[test]
    fn mediafile_maxbitrate_and_mezzanine_min_maxbitrate() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let maxbr = xml.replace("bitrate=\"1500\"", "bitrate=\"1500\" maxBitrate=\"5000\"");
        let mut maxbr_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.maxbr".into(),
                    kind: "mediafile_maxbitrate".into(),
                    message: "need maxBitrate".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["5000".into()]),
                }],
                ..Default::default()
            },
            &maxbr,
            &mut maxbr_ok,
        );
        assert!(!maxbr_ok.iter().any(|f| f.requirement_id == "mf.maxbr"));

        let mut maxbr_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.maxbr".into(),
                    kind: "mediafile_maxbitrate".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["8000".into()]),
                }],
                ..Default::default()
            },
            &maxbr,
            &mut maxbr_fail,
        );
        assert!(maxbr_fail.iter().any(|f| f.requirement_id == "mf.maxbr"));

        let mezz_min = xml.replace(
            "</MediaFile>",
            "</MediaFile><Mezzanine minBitrate=\"1000\"><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        let mut mezz_min_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.minbr".into(),
                    kind: "mezzanine_minbitrate".into(),
                    message: "need minBitrate".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1000".into()]),
                }],
                ..Default::default()
            },
            &mezz_min,
            &mut mezz_min_ok,
        );
        assert!(!mezz_min_ok.iter().any(|f| f.requirement_id == "mezz.minbr"));

        let mut mezz_min_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.minbr".into(),
                    kind: "mezzanine_minbitrate".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["2000".into()]),
                }],
                ..Default::default()
            },
            &mezz_min,
            &mut mezz_min_fail,
        );
        assert!(mezz_min_fail
            .iter()
            .any(|f| f.requirement_id == "mezz.minbr"));

        let mezz_max = xml.replace(
            "</MediaFile>",
            "</MediaFile><Mezzanine maxBitrate=\"50000\"><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        let mut mezz_max_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.maxbr".into(),
                    kind: "mezzanine_maxbitrate".into(),
                    message: "need maxBitrate".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["50000".into()]),
                }],
                ..Default::default()
            },
            &mezz_max,
            &mut mezz_max_ok,
        );
        assert!(!mezz_max_ok.iter().any(|f| f.requirement_id == "mezz.maxbr"));

        let mut mezz_max_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.maxbr".into(),
                    kind: "mezzanine_maxbitrate".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["80000".into()]),
                }],
                ..Default::default()
            },
            &mezz_max,
            &mut mezz_max_fail,
        );
        assert!(mezz_max_fail
            .iter()
            .any(|f| f.requirement_id == "mezz.maxbr"));
    }

    #[test]
    fn mediafile_bitrate_mezzanine_bitrate_and_mediafile_filesize() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mut br_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.br".into(),
                    kind: "mediafile_bitrate".into(),
                    message: "need bitrate".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1500".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut br_ok,
        );
        assert!(!br_ok.iter().any(|f| f.requirement_id == "mf.br"));

        let mut br_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.br".into(),
                    kind: "mediafile_bitrate".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["4000".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut br_fail,
        );
        assert!(br_fail.iter().any(|f| f.requirement_id == "mf.br"));

        let mezz = xml.replace(
            "</MediaFile>",
            "</MediaFile><Mezzanine bitrate=\"8000\"><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        let mut mezz_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.br".into(),
                    kind: "mezzanine_bitrate".into(),
                    message: "need bitrate".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["8000".into()]),
                }],
                ..Default::default()
            },
            &mezz,
            &mut mezz_ok,
        );
        assert!(!mezz_ok.iter().any(|f| f.requirement_id == "mezz.br"));

        let mut mezz_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.br".into(),
                    kind: "mezzanine_bitrate".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["4000".into()]),
                }],
                ..Default::default()
            },
            &mezz,
            &mut mezz_fail,
        );
        assert!(mezz_fail.iter().any(|f| f.requirement_id == "mezz.br"));

        let fs = xml.replace("bitrate=\"1500\"", "bitrate=\"1500\" fileSize=\"1000000\"");
        let mut fs_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.fs".into(),
                    kind: "mediafile_filesize".into(),
                    message: "need fileSize".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1000000".into()]),
                }],
                ..Default::default()
            },
            &fs,
            &mut fs_ok,
        );
        assert!(!fs_ok.iter().any(|f| f.requirement_id == "mf.fs"));

        let mut fs_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.fs".into(),
                    kind: "mediafile_filesize".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["2000000".into()]),
                }],
                ..Default::default()
            },
            &fs,
            &mut fs_fail,
        );
        assert!(fs_fail.iter().any(|f| f.requirement_id == "mf.fs"));
    }

    #[test]
    fn mezzanine_filesize_and_mediafile_width_height() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mezz = xml.replace(
            "</MediaFile>",
            "</MediaFile><Mezzanine fileSize=\"5000000\"><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        let mut mezz_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.fs".into(),
                    kind: "mezzanine_filesize".into(),
                    message: "need fileSize".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["5000000".into()]),
                }],
                ..Default::default()
            },
            &mezz,
            &mut mezz_ok,
        );
        assert!(!mezz_ok.iter().any(|f| f.requirement_id == "mezz.fs"));

        let mut mezz_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.fs".into(),
                    kind: "mezzanine_filesize".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1000000".into()]),
                }],
                ..Default::default()
            },
            &mezz,
            &mut mezz_fail,
        );
        assert!(mezz_fail.iter().any(|f| f.requirement_id == "mezz.fs"));

        let mut w_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.w".into(),
                    kind: "mediafile_width".into(),
                    message: "need width".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["640".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut w_ok,
        );
        assert!(!w_ok.iter().any(|f| f.requirement_id == "mf.w"));

        let mut w_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.w".into(),
                    kind: "mediafile_width".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1920".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut w_fail,
        );
        assert!(w_fail.iter().any(|f| f.requirement_id == "mf.w"));

        let mut h_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.h".into(),
                    kind: "mediafile_height".into(),
                    message: "need height".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["360".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut h_ok,
        );
        assert!(!h_ok.iter().any(|f| f.requirement_id == "mf.h"));

        let mut h_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.h".into(),
                    kind: "mediafile_height".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1080".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut h_fail,
        );
        assert!(h_fail.iter().any(|f| f.requirement_id == "mf.h"));
    }

    #[test]
    fn mezzanine_width_height_and_mediafile_delivery() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mezz_w = xml.replace(
            "</MediaFile>",
            "</MediaFile><Mezzanine width=\"1920\"><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        let mut w_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.w".into(),
                    kind: "mezzanine_width".into(),
                    message: "need width".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1920".into()]),
                }],
                ..Default::default()
            },
            &mezz_w,
            &mut w_ok,
        );
        assert!(!w_ok.iter().any(|f| f.requirement_id == "mezz.w"));

        let mut w_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.w".into(),
                    kind: "mezzanine_width".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1280".into()]),
                }],
                ..Default::default()
            },
            &mezz_w,
            &mut w_fail,
        );
        assert!(w_fail.iter().any(|f| f.requirement_id == "mezz.w"));

        let mezz_h = xml.replace(
            "</MediaFile>",
            "</MediaFile><Mezzanine height=\"1080\"><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        let mut h_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.h".into(),
                    kind: "mezzanine_height".into(),
                    message: "need height".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["1080".into()]),
                }],
                ..Default::default()
            },
            &mezz_h,
            &mut h_ok,
        );
        assert!(!h_ok.iter().any(|f| f.requirement_id == "mezz.h"));

        let mut h_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mezz.h".into(),
                    kind: "mezzanine_height".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["720".into()]),
                }],
                ..Default::default()
            },
            &mezz_h,
            &mut h_fail,
        );
        assert!(h_fail.iter().any(|f| f.requirement_id == "mezz.h"));

        let mut del_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.del".into(),
                    kind: "mediafile_delivery".into(),
                    message: "need delivery".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["progressive".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut del_ok,
        );
        assert!(!del_ok.iter().any(|f| f.requirement_id == "mf.del"));

        let mut del_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.del".into(),
                    kind: "mediafile_delivery".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["streaming".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut del_fail,
        );
        assert!(del_fail.iter().any(|f| f.requirement_id == "mf.del"));
    }

    #[test]
    fn icon_width_height_and_companion_width() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let icon = xml.replace(
            "</Linear>",
            "<Icons><Icon width=\"20\" height=\"20\"></Icon></Icons></Linear>",
        );
        let mut w_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ic.w".into(),
                    kind: "icon_width".into(),
                    message: "need width".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["20".into()]),
                }],
                ..Default::default()
            },
            &icon,
            &mut w_ok,
        );
        assert!(!w_ok.iter().any(|f| f.requirement_id == "ic.w"));

        let mut w_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ic.w".into(),
                    kind: "icon_width".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["30".into()]),
                }],
                ..Default::default()
            },
            &icon,
            &mut w_fail,
        );
        assert!(w_fail.iter().any(|f| f.requirement_id == "ic.w"));

        let mut h_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ic.h".into(),
                    kind: "icon_height".into(),
                    message: "need height".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["20".into()]),
                }],
                ..Default::default()
            },
            &icon,
            &mut h_ok,
        );
        assert!(!h_ok.iter().any(|f| f.requirement_id == "ic.h"));

        let mut h_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "ic.h".into(),
                    kind: "icon_height".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["30".into()]),
                }],
                ..Default::default()
            },
            &icon,
            &mut h_fail,
        );
        assert!(h_fail.iter().any(|f| f.requirement_id == "ic.h"));

        let comp = xml.replace(
            "</Linear>",
            "</Linear><CompanionAds><Companion width=\"300\" height=\"250\"></Companion></CompanionAds>",
        );
        let mut c_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.w".into(),
                    kind: "companion_width".into(),
                    message: "need width".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["300".into()]),
                }],
                ..Default::default()
            },
            &comp,
            &mut c_ok,
        );
        assert!(!c_ok.iter().any(|f| f.requirement_id == "comp.w"));

        let mut c_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.w".into(),
                    kind: "companion_width".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["728".into()]),
                }],
                ..Default::default()
            },
            &comp,
            &mut c_fail,
        );
        assert!(c_fail.iter().any(|f| f.requirement_id == "comp.w"));
    }

    #[test]
    fn companion_height_and_nonlinear_width_height() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let comp = xml.replace(
            "</Linear>",
            "</Linear><CompanionAds><Companion width=\"300\" height=\"250\"></Companion></CompanionAds>",
        );
        let mut h_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.h".into(),
                    kind: "companion_height".into(),
                    message: "need height".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["250".into()]),
                }],
                ..Default::default()
            },
            &comp,
            &mut h_ok,
        );
        assert!(!h_ok.iter().any(|f| f.requirement_id == "comp.h"));

        let mut h_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.h".into(),
                    kind: "companion_height".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["90".into()]),
                }],
                ..Default::default()
            },
            &comp,
            &mut h_fail,
        );
        assert!(h_fail.iter().any(|f| f.requirement_id == "comp.h"));

        let nl = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\" height=\"250\"></NonLinear></NonLinearAds>",
        );
        let mut w_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.w".into(),
                    kind: "nonlinear_width".into(),
                    message: "need width".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["300".into()]),
                }],
                ..Default::default()
            },
            &nl,
            &mut w_ok,
        );
        assert!(!w_ok.iter().any(|f| f.requirement_id == "nl.w"));

        let mut w_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.w".into(),
                    kind: "nonlinear_width".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["728".into()]),
                }],
                ..Default::default()
            },
            &nl,
            &mut w_fail,
        );
        assert!(w_fail.iter().any(|f| f.requirement_id == "nl.w"));

        let mut nh_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.h".into(),
                    kind: "nonlinear_height".into(),
                    message: "need height".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["250".into()]),
                }],
                ..Default::default()
            },
            &nl,
            &mut nh_ok,
        );
        assert!(!nh_ok.iter().any(|f| f.requirement_id == "nl.h"));

        let mut nh_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.h".into(),
                    kind: "nonlinear_height".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["90".into()]),
                }],
                ..Default::default()
            },
            &nl,
            &mut nh_fail,
        );
        assert!(nh_fail.iter().any(|f| f.requirement_id == "nl.h"));
    }

    #[test]
    fn companion_and_nonlinear_expanded_width_height() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let comp = xml.replace(
            "</Linear>",
            "</Linear><CompanionAds><Companion expandedWidth=\"600\" expandedHeight=\"500\"></Companion></CompanionAds>",
        );
        let mut w_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.ew".into(),
                    kind: "companion_expanded_width".into(),
                    message: "need expandedWidth".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["600".into()]),
                }],
                ..Default::default()
            },
            &comp,
            &mut w_ok,
        );
        assert!(!w_ok.iter().any(|f| f.requirement_id == "comp.ew"));

        let mut w_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.ew".into(),
                    kind: "companion_expanded_width".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["728".into()]),
                }],
                ..Default::default()
            },
            &comp,
            &mut w_fail,
        );
        assert!(w_fail.iter().any(|f| f.requirement_id == "comp.ew"));

        let mut h_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.eh".into(),
                    kind: "companion_expanded_height".into(),
                    message: "need expandedHeight".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["500".into()]),
                }],
                ..Default::default()
            },
            &comp,
            &mut h_ok,
        );
        assert!(!h_ok.iter().any(|f| f.requirement_id == "comp.eh"));

        let mut h_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.eh".into(),
                    kind: "companion_expanded_height".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["90".into()]),
                }],
                ..Default::default()
            },
            &comp,
            &mut h_fail,
        );
        assert!(h_fail.iter().any(|f| f.requirement_id == "comp.eh"));

        let nl = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear expandedWidth=\"600\"></NonLinear></NonLinearAds>",
        );
        let mut nl_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.ew".into(),
                    kind: "nonlinear_expanded_width".into(),
                    message: "need expandedWidth".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["600".into()]),
                }],
                ..Default::default()
            },
            &nl,
            &mut nl_ok,
        );
        assert!(!nl_ok.iter().any(|f| f.requirement_id == "nl.ew"));

        let mut nl_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.ew".into(),
                    kind: "nonlinear_expanded_width".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["728".into()]),
                }],
                ..Default::default()
            },
            &nl,
            &mut nl_fail,
        );
        assert!(nl_fail.iter().any(|f| f.requirement_id == "nl.ew"));
    }

    #[test]
    fn nonlinear_expanded_height_and_companion_asset_size() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let nl = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear expandedHeight=\"500\"></NonLinear></NonLinearAds>",
        );
        let mut h_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.eh".into(),
                    kind: "nonlinear_expanded_height".into(),
                    message: "need expandedHeight".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["500".into()]),
                }],
                ..Default::default()
            },
            &nl,
            &mut h_ok,
        );
        assert!(!h_ok.iter().any(|f| f.requirement_id == "nl.eh"));

        let mut h_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "nl.eh".into(),
                    kind: "nonlinear_expanded_height".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["90".into()]),
                }],
                ..Default::default()
            },
            &nl,
            &mut h_fail,
        );
        assert!(h_fail.iter().any(|f| f.requirement_id == "nl.eh"));

        let comp = xml.replace(
            "</Linear>",
            "</Linear><CompanionAds><Companion assetWidth=\"300\" assetHeight=\"250\"></Companion></CompanionAds>",
        );
        let mut w_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.aw".into(),
                    kind: "companion_asset_width".into(),
                    message: "need assetWidth".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["300".into()]),
                }],
                ..Default::default()
            },
            &comp,
            &mut w_ok,
        );
        assert!(!w_ok.iter().any(|f| f.requirement_id == "comp.aw"));

        let mut w_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.aw".into(),
                    kind: "companion_asset_width".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["728".into()]),
                }],
                ..Default::default()
            },
            &comp,
            &mut w_fail,
        );
        assert!(w_fail.iter().any(|f| f.requirement_id == "comp.aw"));

        let mut ah_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.ah".into(),
                    kind: "companion_asset_height".into(),
                    message: "need assetHeight".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["250".into()]),
                }],
                ..Default::default()
            },
            &comp,
            &mut ah_ok,
        );
        assert!(!ah_ok.iter().any(|f| f.requirement_id == "comp.ah"));

        let mut ah_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.ah".into(),
                    kind: "companion_asset_height".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["90".into()]),
                }],
                ..Default::default()
            },
            &comp,
            &mut ah_fail,
        );
        assert!(ah_fail.iter().any(|f| f.requirement_id == "comp.ah"));
    }

    #[test]
    fn icon_click_fallback_size_and_mediafile_type() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let fb = xml.replace(
            "</Linear>",
            "<Icons><Icon><IconClicks><IconClickFallbackImages><IconClickFallbackImage width=\"20\" height=\"20\"></IconClickFallbackImage></IconClickFallbackImages></IconClicks></Icon></Icons></Linear>",
        );
        let mut w_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icfb.w".into(),
                    kind: "icon_click_fallback_width".into(),
                    message: "need width".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["20".into()]),
                }],
                ..Default::default()
            },
            &fb,
            &mut w_ok,
        );
        assert!(!w_ok.iter().any(|f| f.requirement_id == "icfb.w"));

        let mut w_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icfb.w".into(),
                    kind: "icon_click_fallback_width".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["30".into()]),
                }],
                ..Default::default()
            },
            &fb,
            &mut w_fail,
        );
        assert!(w_fail.iter().any(|f| f.requirement_id == "icfb.w"));

        let mut h_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icfb.h".into(),
                    kind: "icon_click_fallback_height".into(),
                    message: "need height".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["20".into()]),
                }],
                ..Default::default()
            },
            &fb,
            &mut h_ok,
        );
        assert!(!h_ok.iter().any(|f| f.requirement_id == "icfb.h"));

        let mut h_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "icfb.h".into(),
                    kind: "icon_click_fallback_height".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["30".into()]),
                }],
                ..Default::default()
            },
            &fb,
            &mut h_fail,
        );
        assert!(h_fail.iter().any(|f| f.requirement_id == "icfb.h"));

        let mut t_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.type".into(),
                    kind: "mediafile_type".into(),
                    message: "need type".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["video/mp4".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut t_ok,
        );
        assert!(!t_ok.iter().any(|f| f.requirement_id == "mf.type"));

        let mut t_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "mf.type".into(),
                    kind: "mediafile_type".into(),
                    message: "need other".into(),
                    element: None,
                    max: None,
                    min: None,
                    value: None,
                    values: Some(vec!["video/webm".into()]),
                }],
                ..Default::default()
            },
            xml,
            &mut t_fail,
        );
        assert!(t_fail.iter().any(|f| f.requirement_id == "mf.type"));
    }

    #[test]
    fn max_mezzanine_size_and_max_companion_width() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let mezz = xml.replace(
            "</MediaFile>",
            "</MediaFile><Mezzanine width=\"1920\" height=\"1080\"><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>",
        );
        let mut mezz_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "mezz.max-w".into(),
                        kind: "max_mezzanine_width".into(),
                        message: "need width cap".into(),
                        element: None,
                        max: Some(1920),
                        min: None,
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "mezz.max-h".into(),
                        kind: "max_mezzanine_height".into(),
                        message: "need height cap".into(),
                        element: None,
                        max: Some(1080),
                        min: None,
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &mezz,
            &mut mezz_ok,
        );
        assert!(!mezz_ok.iter().any(|f| f.requirement_id == "mezz.max-w"));
        assert!(!mezz_ok.iter().any(|f| f.requirement_id == "mezz.max-h"));

        let mut mezz_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "mezz.max-w".into(),
                        kind: "max_mezzanine_width".into(),
                        message: "need other".into(),
                        element: None,
                        max: Some(1280),
                        min: None,
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "mezz.max-h".into(),
                        kind: "max_mezzanine_height".into(),
                        message: "need other".into(),
                        element: None,
                        max: Some(720),
                        min: None,
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &mezz,
            &mut mezz_fail,
        );
        assert!(mezz_fail.iter().any(|f| f.requirement_id == "mezz.max-w"));
        assert!(mezz_fail.iter().any(|f| f.requirement_id == "mezz.max-h"));

        let comp = xml.replace(
            "</Linear>",
            "</Linear><CompanionAds><Companion width=\"300\" height=\"250\"></Companion></CompanionAds>",
        );
        let mut c_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.max-w".into(),
                    kind: "max_companion_width".into(),
                    message: "need width cap".into(),
                    element: None,
                    max: Some(300),
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &comp,
            &mut c_ok,
        );
        assert!(!c_ok.iter().any(|f| f.requirement_id == "comp.max-w"));

        let mut c_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.max-w".into(),
                    kind: "max_companion_width".into(),
                    message: "need other".into(),
                    element: None,
                    max: Some(200),
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &comp,
            &mut c_fail,
        );
        assert!(c_fail.iter().any(|f| f.requirement_id == "comp.max-w"));
    }

    #[test]
    fn max_companion_height_and_max_icon_size() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let comp = xml.replace(
            "</Linear>",
            "</Linear><CompanionAds><Companion width=\"300\" height=\"250\"></Companion></CompanionAds>",
        );
        let mut c_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.max-h".into(),
                    kind: "max_companion_height".into(),
                    message: "need height cap".into(),
                    element: None,
                    max: Some(250),
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &comp,
            &mut c_ok,
        );
        assert!(!c_ok.iter().any(|f| f.requirement_id == "comp.max-h"));
        let mut c_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "comp.max-h".into(),
                    kind: "max_companion_height".into(),
                    message: "need height cap".into(),
                    element: None,
                    max: Some(200),
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &comp,
            &mut c_fail,
        );
        assert!(c_fail.iter().any(|f| f.requirement_id == "comp.max-h"));
        let icon = xml.replace(
            "</Linear>",
            "</Linear><Icons><Icon width=\"20\" height=\"20\"></Icon></Icons>",
        );
        let mut i_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "icon.max-w".into(),
                        kind: "max_icon_width".into(),
                        message: "need width cap".into(),
                        element: None,
                        max: Some(20),
                        min: None,
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "icon.max-h".into(),
                        kind: "max_icon_height".into(),
                        message: "need height cap".into(),
                        element: None,
                        max: Some(20),
                        min: None,
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &icon,
            &mut i_ok,
        );
        assert!(!i_ok.iter().any(|f| f.requirement_id == "icon.max-w"));
        assert!(!i_ok.iter().any(|f| f.requirement_id == "icon.max-h"));
        let mut i_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "icon.max-w".into(),
                        kind: "max_icon_width".into(),
                        message: "need width cap".into(),
                        element: None,
                        max: Some(10),
                        min: None,
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "icon.max-h".into(),
                        kind: "max_icon_height".into(),
                        message: "need height cap".into(),
                        element: None,
                        max: Some(10),
                        min: None,
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &icon,
            &mut i_fail,
        );
        assert!(i_fail.iter().any(|f| f.requirement_id == "icon.max-w"));
        assert!(i_fail.iter().any(|f| f.requirement_id == "icon.max-h"));
    }

    #[test]
    fn max_nonlinear_size_and_max_icon_click_fallback_width() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let nl = xml.replace(
            "</Linear>",
            "</Linear><NonLinearAds><NonLinear width=\"300\" height=\"250\"></NonLinear></NonLinearAds>",
        );
        let mut nl_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "nl.max-w".into(),
                        kind: "max_nonlinear_width".into(),
                        message: "need width cap".into(),
                        element: None,
                        max: Some(300),
                        min: None,
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "nl.max-h".into(),
                        kind: "max_nonlinear_height".into(),
                        message: "need height cap".into(),
                        element: None,
                        max: Some(250),
                        min: None,
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &nl,
            &mut nl_ok,
        );
        assert!(!nl_ok.iter().any(|f| f.requirement_id == "nl.max-w"));
        assert!(!nl_ok.iter().any(|f| f.requirement_id == "nl.max-h"));
        let mut nl_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "nl.max-w".into(),
                        kind: "max_nonlinear_width".into(),
                        message: "need width cap".into(),
                        element: None,
                        max: Some(200),
                        min: None,
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "nl.max-h".into(),
                        kind: "max_nonlinear_height".into(),
                        message: "need height cap".into(),
                        element: None,
                        max: Some(200),
                        min: None,
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &nl,
            &mut nl_fail,
        );
        assert!(nl_fail.iter().any(|f| f.requirement_id == "nl.max-w"));
        assert!(nl_fail.iter().any(|f| f.requirement_id == "nl.max-h"));
        let fb = xml.replace(
            "</Linear>",
            "</Linear><Icons><Icon><IconClickFallbackImages><IconClickFallbackImage width=\"20\" height=\"20\"></IconClickFallbackImage></IconClickFallbackImages></Icon></Icons>",
        );
        let mut fb_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "fb.max-w".into(),
                    kind: "max_icon_click_fallback_width".into(),
                    message: "need width cap".into(),
                    element: None,
                    max: Some(20),
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &fb,
            &mut fb_ok,
        );
        assert!(!fb_ok.iter().any(|f| f.requirement_id == "fb.max-w"));
        let mut fb_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "fb.max-w".into(),
                    kind: "max_icon_click_fallback_width".into(),
                    message: "need width cap".into(),
                    element: None,
                    max: Some(10),
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &fb,
            &mut fb_fail,
        );
        assert!(fb_fail.iter().any(|f| f.requirement_id == "fb.max-w"));
    }

    #[test]
    fn max_icon_click_fallback_height_and_max_companion_expanded_size() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let fb = xml.replace(
            "</Linear>",
            "</Linear><Icons><Icon><IconClickFallbackImages><IconClickFallbackImage width=\"20\" height=\"20\"></IconClickFallbackImage></IconClickFallbackImages></Icon></Icons>",
        );
        let mut fb_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "fb.max-h".into(),
                    kind: "max_icon_click_fallback_height".into(),
                    message: "need height cap".into(),
                    element: None,
                    max: Some(20),
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &fb,
            &mut fb_ok,
        );
        assert!(!fb_ok.iter().any(|f| f.requirement_id == "fb.max-h"));
        let mut fb_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![PolicyRequirement {
                    id: "fb.max-h".into(),
                    kind: "max_icon_click_fallback_height".into(),
                    message: "need height cap".into(),
                    element: None,
                    max: Some(10),
                    min: None,
                    value: None,
                    values: None,
                }],
                ..Default::default()
            },
            &fb,
            &mut fb_fail,
        );
        assert!(fb_fail.iter().any(|f| f.requirement_id == "fb.max-h"));
        let comp = xml.replace(
            "</Linear>",
            "</Linear><CompanionAds><Companion expandedWidth=\"600\" expandedHeight=\"400\"></Companion></CompanionAds>",
        );
        let mut c_ok = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "comp.max-ew".into(),
                        kind: "max_companion_expanded_width".into(),
                        message: "need width cap".into(),
                        element: None,
                        max: Some(600),
                        min: None,
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "comp.max-eh".into(),
                        kind: "max_companion_expanded_height".into(),
                        message: "need height cap".into(),
                        element: None,
                        max: Some(400),
                        min: None,
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &comp,
            &mut c_ok,
        );
        assert!(!c_ok.iter().any(|f| f.requirement_id == "comp.max-ew"));
        assert!(!c_ok.iter().any(|f| f.requirement_id == "comp.max-eh"));
        let mut c_fail = Vec::new();
        apply_overlay(
            &PolicyOverlay {
                id: "ov".into(),
                requirements: vec![
                    PolicyRequirement {
                        id: "comp.max-ew".into(),
                        kind: "max_companion_expanded_width".into(),
                        message: "need width cap".into(),
                        element: None,
                        max: Some(400),
                        min: None,
                        value: None,
                        values: None,
                    },
                    PolicyRequirement {
                        id: "comp.max-eh".into(),
                        kind: "max_companion_expanded_height".into(),
                        message: "need height cap".into(),
                        element: None,
                        max: Some(200),
                        min: None,
                        value: None,
                        values: None,
                    },
                ],
                ..Default::default()
            },
            &comp,
            &mut c_fail,
        );
        assert!(c_fail.iter().any(|f| f.requirement_id == "comp.max-ew"));
        assert!(c_fail.iter().any(|f| f.requirement_id == "comp.max-eh"));
    }

    #[test]
    fn companion_asset_and_nonlinear_expanded_ranges_use_one_element() {
        let base = include_str!("../../../fixtures/ok-https-mp4.xml");
        for (element, width_attr, height_attr, prefix) in [
            ("Companion", "assetWidth", "assetHeight", "companion_asset"),
            (
                "NonLinear",
                "expandedWidth",
                "expandedHeight",
                "nonlinear_expanded",
            ),
        ] {
            let requirements = [
                (format!("min_{prefix}_width"), Some(200), None),
                (format!("min_{prefix}_height"), Some(100), None),
                (format!("max_{prefix}_width"), None, Some(400)),
                (format!("max_{prefix}_height"), None, Some(300)),
            ]
            .into_iter()
            .map(|(kind, min, max)| PolicyRequirement {
                id: kind.clone(),
                kind,
                message: "size outside range".into(),
                element: None,
                min,
                max,
                value: None,
                values: None,
            })
            .collect();
            let overlay = PolicyOverlay {
                id: "size-range".into(),
                requirements,
                ..Default::default()
            };
            let evaluate = |elements: &str| {
                let xml = base.replace("</Linear>", &format!("</Linear>{elements}"));
                let mut findings = Vec::new();
                apply_overlay(&overlay, &xml, &mut findings);
                findings
                    .into_iter()
                    .filter(|f| f.requirement_id.contains(prefix))
                    .count()
            };
            let one = |w: &str, h: &str| {
                format!("<{element} {width_attr}=\"{w}\" {height_attr}=\"{h}\" />")
            };
            assert_eq!(evaluate(&one("300", "200")), 0, "{prefix} within range");
            assert_eq!(evaluate(&one("500", "200")), 4, "{prefix} over width cap");
            assert_eq!(evaluate(&one("300", "400")), 4, "{prefix} over height cap");
            assert_eq!(
                evaluate(&(one("300", "400") + &one("500", "200"))),
                4,
                "{prefix} cannot combine attributes from different elements"
            );
            assert_eq!(evaluate(&one("", "200")), 4, "{prefix} missing width");
            assert_eq!(evaluate(""), 4, "{prefix} missing element");
        }
    }
}
