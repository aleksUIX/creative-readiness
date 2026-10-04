//! Facts parsed from VAST so packs can check duration, MIME, and size.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MediaFacts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codecs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracking_events: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mezzanine_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privacy_macros: Option<bool>,
}

pub fn media_facts(xml: &str) -> MediaFacts {
    let inspected = vastlint_core::inspect_document(xml);
    let tags = all_media_file_attr_tags(xml);
    let idx = preferred_media_index(xml, &tags);
    let tag = idx.and_then(|i| tags.get(i));
    let file = inspect_file_at(&inspected.media_files, idx);
    let mime = tag
        .and_then(|t| attr(t, "type"))
        .or_else(|| inspect_opt_string(file.map(|m| m.mime_type.as_str())));
    let url = media_url(xml);
    let codecs = tag.and_then(|t| attr(t, "codecs"));
    let codec = tag
        .and_then(|t| attr(t, "codec"))
        .or_else(|| video_codec(codecs.as_deref()));
    MediaFacts {
        duration_seconds: parsed_durations(xml).and_then(|ds| ds.into_iter().max()),
        width: tag
            .and_then(|t| attr(t, "width"))
            .and_then(|s| s.parse().ok())
            .or_else(|| inspect_parse(file.map(|m| m.width.as_str()))),
        height: tag
            .and_then(|t| attr(t, "height"))
            .and_then(|s| s.parse().ok())
            .or_else(|| inspect_parse(file.map(|m| m.height.as_str()))),
        mime: mime.clone(),
        bitrate: tag
            .and_then(|t| attr(t, "bitrate"))
            .and_then(|s| s.parse().ok())
            .or_else(|| {
                tag.and_then(|t| attr(t, "maxBitrate"))
                    .and_then(|s| s.parse().ok())
            })
            .or_else(|| inspect_parse(file.map(|m| m.bitrate.as_str()))),
        codec,
        codecs: codecs.clone(),
        audio_codec: audio_codec(codecs.as_deref()),
        container: container_of(mime.as_deref(), url.as_deref()),
        tracking_events: tracking_events(xml),
        categories: categories(xml),
        mezzanine_url: mezzanine_url(xml),
        privacy_macros: Some(has_privacy_macros(xml)),
    }
}

fn preferred_media_index(xml: &str, tags: &[String]) -> Option<usize> {
    if tags.is_empty() {
        return None;
    }
    let urls = collect_elem_texts(xml, "MediaFile");
    (0..tags.len()).max_by_key(|&i| {
        let https = urls
            .get(i)
            .map(|u| u.to_ascii_lowercase().starts_with("https://"))
            .unwrap_or(false) as u8;
        let tag = &tags[i];
        let br = attr(tag, "bitrate")
            .or_else(|| attr(tag, "maxBitrate"))
            .is_some() as u8;
        let codecs = attr(tag, "codecs").or_else(|| attr(tag, "codec")).is_some() as u8;
        let size = attr(tag, "width")
            .and_then(|_| attr(tag, "height"))
            .is_some() as u8;
        (https, br, codecs, size)
    })
}

fn inspect_nonempty(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|v| !v.is_empty())
}

fn inspect_opt_string(s: Option<&str>) -> Option<String> {
    inspect_nonempty(s).map(str::to_string)
}

fn inspect_parse<T: std::str::FromStr>(s: Option<&str>) -> Option<T> {
    inspect_nonempty(s).and_then(|v| v.parse().ok())
}

fn preferred_inspect_index(files: &[vastlint_core::InspectMediaFile]) -> Option<usize> {
    if files.is_empty() {
        return None;
    }
    (0..files.len()).max_by_key(|&i| {
        let f = &files[i];
        let https = f.url.to_ascii_lowercase().starts_with("https://") as u8;
        let br = !f.bitrate.trim().is_empty() as u8;
        let size = (!f.width.trim().is_empty() && !f.height.trim().is_empty()) as u8;
        (https, br, 0u8, size)
    })
}

fn inspect_file_at(
    files: &[vastlint_core::InspectMediaFile],
    idx: Option<usize>,
) -> Option<&vastlint_core::InspectMediaFile> {
    idx.and_then(|i| files.get(i))
        .or_else(|| preferred_inspect_index(files).and_then(|i| files.get(i)))
}

fn row_from_inspect(m: &vastlint_core::InspectMediaFile) -> MediaFileRow {
    let mime = inspect_opt_string(Some(m.mime_type.as_str()));
    let url = inspect_nonempty(Some(m.url.as_str()));
    MediaFileRow {
        mime: mime.clone(),
        container: container_of(mime.as_deref(), url),
        width: inspect_parse(Some(m.width.as_str())),
        height: inspect_parse(Some(m.height.as_str())),
        bitrate: inspect_parse(Some(m.bitrate.as_str())),
        codecs_hay: None,
        delivery: inspect_opt_string(Some(m.delivery.as_str())),
    }
}

fn fill_row_from_inspect(row: &mut MediaFileRow, m: &vastlint_core::InspectMediaFile) {
    if row.mime.is_none() {
        row.mime = inspect_opt_string(Some(m.mime_type.as_str()));
    }
    if row.container.is_none() {
        row.container = container_of(row.mime.as_deref(), inspect_nonempty(Some(m.url.as_str())));
    }
    if row.width.is_none() {
        row.width = inspect_parse(Some(m.width.as_str()));
    }
    if row.height.is_none() {
        row.height = inspect_parse(Some(m.height.as_str()));
    }
    if row.bitrate.is_none() {
        row.bitrate = inspect_parse(Some(m.bitrate.as_str()));
    }
    if row.delivery.is_none() {
        row.delivery = inspect_opt_string(Some(m.delivery.as_str()));
    }
}

fn video_codec(codecs: Option<&str>) -> Option<String> {
    codecs.and_then(|s| {
        s.split(',')
            .map(str::trim)
            .find(|t| {
                let l = t.to_ascii_lowercase();
                l.starts_with("avc")
                    || l.starts_with("hev")
                    || l.starts_with("hvc")
                    || l.starts_with("vp9")
                    || l.starts_with("av01")
            })
            .map(|t| t.to_string())
    })
}

fn audio_codec(codecs: Option<&str>) -> Option<String> {
    codecs.and_then(|s| {
        s.split(',')
            .map(str::trim)
            .find(|t| {
                let l = t.to_ascii_lowercase();
                l.starts_with("mp4a")
                    || l.starts_with("aac")
                    || l.contains("opus")
                    || l.contains("ac-3")
                    || l.contains("ec-3")
                    || l.starts_with("mp3")
            })
            .map(|t| t.to_string())
    })
}

fn container_of(mime: Option<&str>, url: Option<&str>) -> Option<String> {
    if let Some(m) = mime {
        if let Some((_, sub)) = m.split_once('/') {
            let sub = sub
                .split(';')
                .next()
                .unwrap_or(sub)
                .trim()
                .to_ascii_lowercase();
            if !sub.is_empty() {
                return Some(sub);
            }
        }
    }
    url.and_then(|u| {
        let u = u.split('?').next().unwrap_or(u).to_ascii_lowercase();
        if u.ends_with(".mp4") {
            Some("mp4".into())
        } else if u.ends_with(".webm") {
            Some("webm".into())
        } else if u.ends_with(".m3u8") {
            Some("hls".into())
        } else if u.ends_with(".mpd") {
            Some("dash".into())
        } else {
            None
        }
    })
}

pub fn mezzanine_url(xml: &str) -> Option<String> {
    let urls = collect_elem_texts(xml, "Mezzanine");
    urls.iter()
        .find(|u| u.to_ascii_lowercase().starts_with("https://"))
        .cloned()
        .or_else(|| urls.into_iter().next())
}

pub(crate) fn any_https_mezzanine(xml: &str) -> bool {
    let urls = collect_elem_texts(xml, "Mezzanine");
    urls.is_empty()
        || urls
            .iter()
            .any(|u| u.to_ascii_lowercase().starts_with("https://"))
}

pub fn tracking_events(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    for inner in elem_inners(xml, "Linear") {
        collect_tracking_from(inner, &mut out);
    }
    out
}

fn collect_tracking_from(xml: &str, out: &mut Vec<String>) {
    let xml_l = xml.to_ascii_lowercase();
    let mut from = 0;
    while let Some(rel) = xml_l[from..].find("<tracking") {
        let idx = from + rel;
        let after_i = idx + "<tracking".len();
        let next = xml.get(after_i..).and_then(|s| s.chars().next());
        if next.is_none() || matches!(next, Some(' ' | '>' | '/' | '\n' | '\t' | '\r')) {
            let rest = &xml[idx..];
            let Some(end) = rest.find('>') else {
                break;
            };
            let tag = &rest[..end];
            if let Some(ev) = attr(tag, "event") {
                let ev = ev.to_ascii_lowercase();
                if !out.iter().any(|e| e == &ev) {
                    out.push(ev);
                }
            }
            from = idx + end + 1;
        } else {
            from = after_i;
        }
    }
}

pub fn categories(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    for t in collect_elem_texts(xml, "Category") {
        let t = t.trim();
        if !t.is_empty() && !out.iter().any(|e: &String| e.eq_ignore_ascii_case(t)) {
            out.push(t.to_string());
        }
    }
    out
}

pub fn has_privacy_macros(xml: &str) -> bool {
    xml_has_macro(xml, "CACHEBUSTING") || xml_has_macro(xml, "TIMESTAMP")
}

/// IAB tokens `[NAME]`, `${NAME}`, or `%%NAME%%` on Impression, Tracking, click-tracking, Error, ViewableImpression, Icon, Companion, NonLinear, ClickThrough, StaticResource, IFrameResource, HTMLResource, FlashResource, AdParameters, IconClickFallbackImage, VerificationParameters, Survey, Extension, and AdVerification resource URLs.
pub(crate) fn xml_has_macro(xml: &str, name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() {
        return false;
    }
    let hay = impression_and_tracking_hay(xml).to_ascii_uppercase();
    let needle = name.to_ascii_uppercase();
    wrapped_macro(&hay, "[", "]", &needle)
        || wrapped_macro(&hay, "${", "}", &needle)
        || wrapped_macro(&hay, "%%", "%%", &needle)
}

fn impression_and_tracking_hay(xml: &str) -> String {
    let mut parts = Vec::new();
    for elem in [
        "Impression",
        "Tracking",
        "ClickTracking",
        "CustomClick",
        "Error",
        "Viewable",
        "NotViewable",
        "ViewUndetermined",
        "IconClickTracking",
        "IconViewTracking",
        "CompanionClickTracking",
        "NonLinearClickTracking",
        "ClickThrough",
        "IconClickThrough",
        "CompanionClickThrough",
        "NonLinearClickThrough",
        "StaticResource",
        "IFrameResource",
        "HTMLResource",
        "FlashResource",
        "AdParameters",
        "IconClickFallbackImage",
        "VerificationParameters",
        "Survey",
        "Extension",
        "JavaScriptResource",
        "ExecutableResource",
    ] {
        parts.extend(collect_elem_texts(xml, elem));
    }
    parts.join("\n")
}

fn collect_elem_texts(xml: &str, elem: &str) -> Vec<String> {
    elem_inners(xml, elem)
        .into_iter()
        .filter_map(|inner| {
            let mut text = inner.trim();
            if text.len() >= 9 && text[..9].eq_ignore_ascii_case("<![CDATA[") {
                text = text[9..].trim();
            }
            if text.len() >= 3 && text[text.len() - 3..].eq_ignore_ascii_case("]]>") {
                text = text[..text.len() - 3].trim();
            }
            if text.is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        })
        .collect()
}

fn elem_inners<'a>(xml: &'a str, elem: &str) -> Vec<&'a str> {
    let xml_l = xml.to_ascii_lowercase();
    let needle_l = format!("<{}", elem.to_ascii_lowercase());
    let close_l = format!("</{}>", elem.to_ascii_lowercase());
    let mut from = 0;
    let mut out = Vec::new();
    while let Some(rel) = xml_l[from..].find(&needle_l) {
        let after_open = from + rel + needle_l.len();
        from = after_open;
        let Some(after) = xml.get(after_open..) else {
            break;
        };
        let next = after.chars().next();
        if next.is_none() || matches!(next, Some(' ' | '>' | '/' | '\n' | '\t' | '\r')) {
            let Some(gt) = after.find('>') else {
                break;
            };
            let body_start = after_open + gt + 1;
            let Some(body_l) = xml_l.get(body_start..) else {
                break;
            };
            if let Some(end_rel) = body_l.find(&close_l) {
                if let Some(inner) = xml.get(body_start..body_start + end_rel) {
                    out.push(inner);
                }
                from = body_start + end_rel + close_l.len();
            }
        }
    }
    out
}

fn wrapped_macro(hay: &str, open: &str, close: &str, name: &str) -> bool {
    let mut from = 0;
    while let Some(rel) = hay[from..].find(open) {
        let abs = from + rel;
        // `<![CDATA[` also uses `[` / `]`; do not treat the opener as a macro wrapper.
        if open == "[" && hay[abs..].starts_with("[CDATA[") {
            from = abs + "[CDATA[".len();
            continue;
        }
        let start = abs + open.len();
        if start > hay.len() {
            break;
        }
        if let Some(end_rel) = hay[start..].find(close) {
            if hay[start..start + end_rel].trim() == name {
                return true;
            }
            from = start + end_rel + close.len();
        } else {
            break;
        }
    }
    false
}

pub fn media_url(xml: &str) -> Option<String> {
    let urls = collect_elem_texts(xml, "MediaFile");
    let tags = all_media_file_attr_tags(xml);
    let files = vastlint_core::inspect_document(xml).media_files;
    let idx = preferred_media_index(xml, &tags);
    idx.and_then(|i| files.get(i))
        .map(|m| m.url.trim().to_string())
        .filter(|u| !u.is_empty())
        .or_else(|| {
            idx.and_then(|i| urls.get(i).cloned())
                .filter(|u| !u.trim().is_empty())
        })
        .or_else(|| {
            urls.iter()
                .find(|u| u.to_ascii_lowercase().starts_with("https://"))
                .cloned()
        })
        .or_else(|| urls.into_iter().next())
        .or_else(|| {
            inspect_file_at(&files, None)
                .map(|m| m.url.trim().to_string())
                .filter(|u| !u.is_empty())
        })
}

pub(crate) fn any_https_mediafile(xml: &str) -> bool {
    collect_elem_texts(xml, "MediaFile")
        .iter()
        .any(|u| u.to_ascii_lowercase().starts_with("https://"))
        || vastlint_core::inspect_document(xml)
            .media_files
            .iter()
            .any(|m| m.url.to_ascii_lowercase().starts_with("https://"))
}

pub(crate) fn caption_language_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "ClosedCaptionFile", "language", allowed)
}

pub(crate) fn caption_type_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "ClosedCaptionFile", "type", allowed)
}

pub(crate) fn caption_file_status_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "ClosedCaptionFile", "fileStatus", allowed)
}

pub(crate) fn id_registry_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "UniversalAdId", "idRegistry", allowed)
}

pub(crate) fn icon_program_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Icon", "program", allowed)
}

pub(crate) fn ad_system_version_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "AdSystem", "version", allowed)
}

pub(crate) fn vast_version_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "VAST", "version", allowed)
}

pub(crate) fn creative_sequence_misses(xml: &str, required: &[String]) -> bool {
    sequence_attr_misses(xml, "Creative", required)
}

pub(crate) fn ad_sequence_misses(xml: &str, required: &[String]) -> bool {
    sequence_attr_misses(xml, "Ad", required)
}

fn sequence_attr_misses(xml: &str, elem: &str, required: &[String]) -> bool {
    let mut found = Vec::new();
    for tag in elem_open_attr_tags(xml, elem) {
        if let Some(v) = attr(&tag, "sequence") {
            let t = v.trim();
            if !t.is_empty() {
                found.push(t.to_string());
            }
        }
    }
    required
        .iter()
        .any(|need| !found.iter().any(|have| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn verification_vendor_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Verification", "vendor", allowed)
}

pub(crate) fn pricing_model_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Pricing", "model", allowed)
}

pub(crate) fn pricing_currency_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Pricing", "currency", allowed)
}

pub(crate) fn wrapper_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "VASTAdTagURI", false)
}

pub(crate) fn error_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "Error", true)
}

pub(crate) fn viewable_https_misses(xml: &str) -> bool {
    if elem_inners(xml, "ViewableImpression").is_empty() {
        return true;
    }
    https_elem_misses(xml, "Viewable", true)
        || https_elem_misses(xml, "NotViewable", false)
        || https_elem_misses(xml, "ViewUndetermined", false)
}

pub(crate) fn impression_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "Impression", true)
}

pub(crate) fn clickthrough_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "ClickThrough", true)
}

pub(crate) fn survey_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "Survey", false)
}

pub(crate) fn clicktracking_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "ClickTracking", false)
        || https_elem_misses(xml, "IconClickTracking", false)
        || https_elem_misses(xml, "CompanionClickTracking", false)
        || https_elem_misses(xml, "NonLinearClickTracking", false)
        || https_elem_misses(xml, "CustomClick", false)
}

pub(crate) fn icon_clickthrough_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "IconClickThrough", false)
}

pub(crate) fn category_authority_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Category", "authority", allowed)
}

pub(crate) fn javascript_resource_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "JavaScriptResource", false)
}

pub(crate) fn companion_clickthrough_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "CompanionClickThrough", false)
}

pub(crate) fn advertiser_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "Advertiser");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn advertiser_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Advertiser", "id", allowed)
}

pub(crate) fn ad_type_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Ad", "adType", allowed)
}

pub(crate) fn impression_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Impression", "id", allowed)
}

pub(crate) fn companion_ads_required_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "CompanionAds", "required", allowed)
}

pub(crate) fn mediafile_file_size_below(xml: &str, min: u64) -> bool {
    let mut found_ok = false;
    let mut any = false;
    for tag in elem_open_attr_tags(xml, "MediaFile") {
        any = true;
        if let Some(n) = attr(&tag, "fileSize").and_then(|s| s.trim().parse::<u64>().ok()) {
            if n >= min {
                found_ok = true;
            }
        }
    }
    !any || !found_ok
}

pub(crate) fn mediafile_media_type_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "MediaFile", "mediaType", allowed)
}

pub(crate) fn interactive_creative_file_variable_duration_misses(
    xml: &str,
    allowed: &[String],
) -> bool {
    attr_allowed_misses(xml, "InteractiveCreativeFile", "variableDuration", allowed)
}

pub(crate) fn creative_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Creative", "id", allowed)
}

pub(crate) fn clickthrough_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "ClickThrough", "id", allowed)
}

pub(crate) fn tracking_offset_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Tracking", "offset", allowed)
}

pub(crate) fn clicktracking_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "ClickTracking", "id", allowed)
}

pub(crate) fn custom_click_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "CustomClick", "id", allowed)
}

pub(crate) fn icon_clickthrough_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "IconClickThrough", "id", allowed)
}

pub(crate) fn companion_clickthrough_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "CompanionClickThrough", "id", allowed)
}

pub(crate) fn nonlinear_clickthrough_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "NonLinearClickThrough", "id", allowed)
}

pub(crate) fn icon_clicktracking_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "IconClickTracking", "id", allowed)
}

pub(crate) fn companion_clicktracking_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "CompanionClickTracking", "id", allowed)
}

pub(crate) fn nonlinear_clicktracking_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "NonLinearClickTracking", "id", allowed)
}

pub(crate) fn icon_viewtracking_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "IconViewTracking", "id", allowed)
}

pub(crate) fn tracking_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Tracking", "id", allowed)
}

pub(crate) fn error_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Error", "id", allowed)
}

pub(crate) fn viewable_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Viewable", "id", allowed)
}

pub(crate) fn notviewable_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "NotViewable", "id", allowed)
}

pub(crate) fn viewundetermined_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "ViewUndetermined", "id", allowed)
}

pub(crate) fn survey_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Survey", "id", allowed)
}

pub(crate) fn skipoffset_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Linear", "skipoffset", allowed)
        && attr_allowed_misses(xml, "NonLinear", "skipoffset", allowed)
}

pub(crate) fn duration_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "Duration");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn expires_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "Expires");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn icon_duration_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Icon", "duration", allowed)
}

pub(crate) fn icon_offset_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Icon", "offset", allowed)
}

pub(crate) fn min_suggested_duration_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "NonLinear", "minSuggestedDuration", allowed)
}

pub(crate) fn ad_serving_id_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "AdServingId");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn ad_title_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "AdTitle");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn description_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "Description");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn ad_parameters_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "AdParameters");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn static_resource_creative_type_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "StaticResource", "creativeType", allowed)
}

pub(crate) fn flash_resource_creative_type_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "FlashResource", "creativeType", allowed)
}

pub(crate) fn impression_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "Impression");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn error_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "Error");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn clickthrough_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "ClickThrough");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn survey_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "Survey");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn tracking_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "Tracking");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn viewable_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "Viewable");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn clicktracking_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "ClickTracking");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn mezzanine_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "Mezzanine");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn mediafile_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "MediaFile");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn vast_ad_tag_uri_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "VASTAdTagURI");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn javascript_resource_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "JavaScriptResource");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn closed_caption_file_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "ClosedCaptionFile");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn executable_resource_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "ExecutableResource");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn icon_clickthrough_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "IconClickThrough");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn companion_clickthrough_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "CompanionClickThrough");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn nonlinear_clickthrough_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "NonLinearClickThrough");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn icon_viewtracking_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "IconViewTracking");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn interactive_creative_file_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "InteractiveCreativeFile");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn static_resource_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "StaticResource");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn iframe_resource_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "IFrameResource");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn html_resource_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "HTMLResource");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn flash_resource_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "FlashResource");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn icon_clickfallback_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "IconClickFallbackImage");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn custom_click_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "CustomClick");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn icon_clicktracking_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "IconClickTracking");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn companion_clicktracking_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "CompanionClickTracking");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn nonlinear_clicktracking_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "NonLinearClickTracking");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn notviewable_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "NotViewable");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn viewundetermined_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "ViewUndetermined");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn extension_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "Extension");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn verification_parameters_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "VerificationParameters");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn blocked_ad_categories_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "BlockedAdCategories");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn creative_extension_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "CreativeExtension");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn creative_extension_type_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "CreativeExtension", "type", allowed)
}

pub(crate) fn id_value_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "UniversalAdId", "idValue", allowed)
}

pub(crate) fn mediafile_codec_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "MediaFile", "codec", allowed)
        && attr_allowed_misses(xml, "MediaFile", "codecs", allowed)
}

pub(crate) fn creative_extension_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "CreativeExtension", false)
}

pub(crate) fn notviewable_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "NotViewable", false)
}

pub(crate) fn viewundetermined_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "ViewUndetermined", false)
}

pub(crate) fn custom_click_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "CustomClick", false)
}

pub(crate) fn icon_clicktracking_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "IconClickTracking", false)
}

pub(crate) fn companion_clicktracking_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "CompanionClickTracking", false)
}

pub(crate) fn nonlinear_clicktracking_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "NonLinearClickTracking", false)
}

pub(crate) fn blocked_ad_categories_authority_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "BlockedAdCategories", "authority", allowed)
}

pub(crate) fn mediafile_minbitrate_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "MediaFile", "minBitrate", allowed)
}

pub(crate) fn mediafile_maxbitrate_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "MediaFile", "maxBitrate", allowed)
}

pub(crate) fn mezzanine_minbitrate_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Mezzanine", "minBitrate", allowed)
}

pub(crate) fn mezzanine_maxbitrate_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Mezzanine", "maxBitrate", allowed)
}

pub(crate) fn mediafile_bitrate_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "MediaFile", "bitrate", allowed)
}

pub(crate) fn mezzanine_bitrate_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Mezzanine", "bitrate", allowed)
}

pub(crate) fn mediafile_filesize_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "MediaFile", "fileSize", allowed)
}

pub(crate) fn mezzanine_filesize_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Mezzanine", "fileSize", allowed)
}

pub(crate) fn mediafile_width_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "MediaFile", "width", allowed)
}

pub(crate) fn mediafile_height_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "MediaFile", "height", allowed)
}

pub(crate) fn mezzanine_width_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Mezzanine", "width", allowed)
}

pub(crate) fn mezzanine_height_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Mezzanine", "height", allowed)
}

pub(crate) fn mediafile_delivery_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "MediaFile", "delivery", allowed)
}

pub(crate) fn icon_width_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Icon", "width", allowed)
}

pub(crate) fn icon_height_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Icon", "height", allowed)
}

pub(crate) fn companion_width_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Companion", "width", allowed)
}

pub(crate) fn companion_height_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Companion", "height", allowed)
}

pub(crate) fn nonlinear_width_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "NonLinear", "width", allowed)
}

pub(crate) fn nonlinear_height_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "NonLinear", "height", allowed)
}

pub(crate) fn companion_expanded_width_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Companion", "expandedWidth", allowed)
}

pub(crate) fn companion_expanded_height_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Companion", "expandedHeight", allowed)
}

pub(crate) fn nonlinear_expanded_width_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "NonLinear", "expandedWidth", allowed)
}

pub(crate) fn nonlinear_expanded_height_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "NonLinear", "expandedHeight", allowed)
}

pub(crate) fn companion_asset_width_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Companion", "assetWidth", allowed)
}

pub(crate) fn companion_asset_height_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Companion", "assetHeight", allowed)
}

pub(crate) fn icon_click_fallback_width_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "IconClickFallbackImage", "width", allowed)
}

pub(crate) fn icon_click_fallback_height_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "IconClickFallbackImage", "height", allowed)
}

pub(crate) fn mediafile_type_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "MediaFile", "type", allowed)
}

pub(crate) fn companion_alt_text_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "AltText");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn verification_parameters_https_misses(xml: &str) -> bool {
    let n_tags = elem_inners(xml, "VerificationParameters").len();
    if n_tags == 0 {
        return false;
    }
    let texts = collect_elem_texts(xml, "VerificationParameters");
    if texts.len() != n_tags {
        return true;
    }
    for t in texts {
        let t = t.trim();
        if t.is_empty() {
            return true;
        }
        let lower = t.to_ascii_lowercase();
        if lower.starts_with("https://") {
            continue;
        }
        if lower.contains("http://") || !lower.contains("https://") {
            return true;
        }
    }
    false
}

pub(crate) fn executable_resource_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "ExecutableResource", false)
}

pub(crate) fn nonlinear_clickthrough_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "NonLinearClickThrough", false)
}

pub(crate) fn description_text(xml: &str) -> Option<String> {
    for t in collect_elem_texts(xml, "Description") {
        let t = t.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    None
}

pub(crate) fn description_too_short(xml: &str, min: u64) -> bool {
    match description_text(xml) {
        Some(s) => (s.len() as u64) < min,
        None => true,
    }
}

pub(crate) fn static_resource_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "StaticResource", false)
}

pub(crate) fn iframe_resource_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "IFrameResource", false)
}

pub(crate) fn ad_title_text(xml: &str) -> Option<String> {
    for t in collect_elem_texts(xml, "AdTitle") {
        let t = t.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    None
}

pub(crate) fn ad_title_too_short(xml: &str, min: u64) -> bool {
    match ad_title_text(xml) {
        Some(s) => (s.len() as u64) < min,
        None => true,
    }
}

pub(crate) fn html_resource_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "HTMLResource", false)
}

pub(crate) fn flash_resource_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "FlashResource", false)
}

pub(crate) fn ad_parameters_text(xml: &str) -> Option<String> {
    for t in collect_elem_texts(xml, "AdParameters") {
        let t = t.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    None
}

pub(crate) fn ad_parameters_too_short(xml: &str, min: u64) -> bool {
    match ad_parameters_text(xml) {
        Some(s) => (s.len() as u64) < min,
        None => true,
    }
}

pub(crate) fn closed_caption_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "ClosedCaptionFile", false)
}

pub(crate) fn icon_viewtracking_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "IconViewTracking", false)
}

pub(crate) fn ad_system_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "AdSystem");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn icon_clickfallback_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "IconClickFallbackImage", false)
}

pub(crate) fn mezzanine_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "Mezzanine", false)
}

pub(crate) fn category_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "Category");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn tracking_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "Tracking", false)
}

pub(crate) fn pricing_misses(xml: &str, allowed: &[String]) -> bool {
    let texts = collect_elem_texts(xml, "Pricing");
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn extension_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "Extension", false)
}

pub(crate) fn interactive_creative_file_https_misses(xml: &str) -> bool {
    https_elem_misses(xml, "InteractiveCreativeFile", false)
}

pub(crate) fn universal_ad_id_misses(xml: &str, allowed: &[String]) -> bool {
    let mut texts = collect_elem_texts(xml, "UniversalAdId");
    for tag in elem_open_attr_tags(xml, "UniversalAdId") {
        if let Some(v) = attr(&tag, "idValue") {
            let t = v.trim();
            if !t.is_empty() {
                texts.push(t.to_string());
            }
        }
    }
    if texts.is_empty() {
        return true;
    }
    !texts
        .iter()
        .any(|have| allowed.iter().any(|need| have.eq_ignore_ascii_case(need)))
}

pub(crate) fn api_framework_misses(xml: &str, allowed: &[String]) -> bool {
    let mut found = Vec::new();
    for elem in [
        "MediaFile",
        "InteractiveCreativeFile",
        "NonLinear",
        "JavaScriptResource",
        "ExecutableResource",
        "Companion",
        "Icon",
        "Creative",
        "Mezzanine",
        "Verification",
    ] {
        for tag in elem_open_attr_tags(xml, elem) {
            if let Some(v) = attr(&tag, "apiFramework") {
                let t = v.trim();
                if !t.is_empty() {
                    found.push(t.to_string());
                }
            }
        }
    }
    if found.is_empty() {
        return false;
    }
    found
        .iter()
        .any(|v| !allowed.iter().any(|a| a.eq_ignore_ascii_case(v)))
}

pub(crate) fn creative_adid_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Creative", "AdID", allowed)
}

pub(crate) fn interactive_creative_file_type_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "InteractiveCreativeFile", "type", allowed)
}

pub(crate) fn viewable_impression_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "ViewableImpression", "id", allowed)
}

pub(crate) fn companion_type_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Companion", "type", allowed)
        && attr_allowed_misses(xml, "StaticResource", "creativeType", allowed)
}

pub(crate) fn icon_type_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Icon", "type", allowed)
}

pub(crate) fn ad_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Ad", "id", allowed)
}

pub(crate) fn mediafile_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "MediaFile", "id", allowed)
}

pub(crate) fn companion_adslot_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Companion", "adSlotId", allowed)
}

pub(crate) fn icon_alt_text_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Icon", "altText", allowed)
}

pub(crate) fn companion_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Companion", "id", allowed)
}

pub(crate) fn icon_hover_text_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Icon", "hoverText", allowed)
}

pub(crate) fn nonlinear_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "NonLinear", "id", allowed)
}

pub(crate) fn companion_rendering_mode_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Companion", "renderingMode", allowed)
}

pub(crate) fn icon_pxratio_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Icon", "pxratio", allowed)
}

pub(crate) fn nonlinear_scalable_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "NonLinear", "scalable", allowed)
}

pub(crate) fn nonlinear_maintain_aspect_ratio_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "NonLinear", "maintainAspectRatio", allowed)
}

pub(crate) fn companion_xml_encoded_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Companion", "xmlEncoded", allowed)
}

pub(crate) fn icon_xml_encoded_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Icon", "xmlEncoded", allowed)
}

pub(crate) fn nonlinear_xml_encoded_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "NonLinear", "xmlEncoded", allowed)
}

pub(crate) fn companion_pxratio_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Companion", "pxratio", allowed)
}

pub(crate) fn wrapper_follow_additional_wrappers_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Wrapper", "followAdditionalWrappers", allowed)
}

pub(crate) fn wrapper_allow_multiple_ads_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Wrapper", "allowMultipleAds", allowed)
}

pub(crate) fn wrapper_fallback_on_no_ad_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Wrapper", "fallbackOnNoAd", allowed)
}

pub(crate) fn ad_conditional_ad_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Ad", "conditionalAd", allowed)
}

pub(crate) fn mediafile_scalable_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "MediaFile", "scalable", allowed)
}

pub(crate) fn mediafile_maintain_aspect_ratio_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "MediaFile", "maintainAspectRatio", allowed)
}

pub(crate) fn executable_resource_type_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "ExecutableResource", "type", allowed)
}

pub(crate) fn javascript_resource_browser_optional_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "JavaScriptResource", "browserOptional", allowed)
}

pub(crate) fn extension_type_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Extension", "type", allowed)
}

pub(crate) fn mezzanine_type_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Mezzanine", "type", allowed)
}

pub(crate) fn mezzanine_delivery_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Mezzanine", "delivery", allowed)
}

pub(crate) fn ad_parameters_xml_encoded_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "AdParameters", "xmlEncoded", allowed)
}

pub(crate) fn mezzanine_codec_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Mezzanine", "codec", allowed)
        && attr_allowed_misses(xml, "Mezzanine", "codecs", allowed)
}

pub(crate) fn mezzanine_file_size_below(xml: &str, min: u64) -> bool {
    let mut found_ok = false;
    let mut any = false;
    for tag in elem_open_attr_tags(xml, "Mezzanine") {
        any = true;
        if let Some(n) = attr(&tag, "fileSize").and_then(|s| s.trim().parse::<u64>().ok()) {
            if n >= min {
                found_ok = true;
            }
        }
    }
    !any || !found_ok
}

#[cfg(test)]
pub(crate) fn mezzanine_size_misses(
    xml: &str,
    min_width: Option<u64>,
    min_height: Option<u64>,
) -> bool {
    mezzanine_size_range_misses(xml, min_width, min_height, None, None)
}

pub(crate) fn mezzanine_size_range_misses(
    xml: &str,
    min_width: Option<u64>,
    min_height: Option<u64>,
    max_width: Option<u64>,
    max_height: Option<u64>,
) -> bool {
    elem_size_range_misses(
        xml,
        "Mezzanine",
        "width",
        "height",
        min_width,
        min_height,
        max_width,
        max_height,
    )
}

fn mezzanine_bitrate(tag: &str) -> Option<u64> {
    attr(tag, "bitrate")
        .and_then(|s| s.trim().parse().ok())
        .or_else(|| attr(tag, "maxBitrate").and_then(|s| s.trim().parse().ok()))
}

pub(crate) fn mezzanine_bitrate_misses_range(
    xml: &str,
    min: Option<u64>,
    max: Option<u64>,
) -> bool {
    let mut found_ok = false;
    let mut any = false;
    for tag in elem_open_attr_tags(xml, "Mezzanine") {
        any = true;
        if let Some(n) = mezzanine_bitrate(&tag) {
            let floor_ok = min.map(|m| n >= m).unwrap_or(true);
            let cap_ok = max.map(|m| n <= m).unwrap_or(true);
            if floor_ok && cap_ok {
                found_ok = true;
            }
        }
    }
    !any || !found_ok
}

pub(crate) fn mezzanine_media_type_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Mezzanine", "mediaType", allowed)
}

pub(crate) fn mezzanine_scalable_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Mezzanine", "scalable", allowed)
}

pub(crate) fn mezzanine_maintain_aspect_ratio_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Mezzanine", "maintainAspectRatio", allowed)
}

pub(crate) fn mezzanine_id_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Mezzanine", "id", allowed)
}

#[cfg(test)]
pub(crate) fn icon_size_misses(xml: &str, min_width: Option<u64>, min_height: Option<u64>) -> bool {
    icon_size_range_misses(xml, min_width, min_height, None, None)
}

pub(crate) fn icon_size_range_misses(
    xml: &str,
    min_width: Option<u64>,
    min_height: Option<u64>,
    max_width: Option<u64>,
    max_height: Option<u64>,
) -> bool {
    elem_size_range_misses(
        xml, "Icon", "width", "height", min_width, min_height, max_width, max_height,
    )
}

#[cfg(test)]
pub(crate) fn icon_click_fallback_size_misses(
    xml: &str,
    min_width: Option<u64>,
    min_height: Option<u64>,
) -> bool {
    icon_click_fallback_size_range_misses(xml, min_width, min_height, None, None)
}

pub(crate) fn icon_click_fallback_size_range_misses(
    xml: &str,
    min_width: Option<u64>,
    min_height: Option<u64>,
    max_width: Option<u64>,
    max_height: Option<u64>,
) -> bool {
    elem_size_range_misses(
        xml,
        "IconClickFallbackImage",
        "width",
        "height",
        min_width,
        min_height,
        max_width,
        max_height,
    )
}

#[cfg(test)]
pub(crate) fn nonlinear_size_misses(
    xml: &str,
    min_width: Option<u64>,
    min_height: Option<u64>,
) -> bool {
    nonlinear_size_range_misses(xml, min_width, min_height, None, None)
}

pub(crate) fn nonlinear_size_range_misses(
    xml: &str,
    min_width: Option<u64>,
    min_height: Option<u64>,
    max_width: Option<u64>,
    max_height: Option<u64>,
) -> bool {
    elem_size_range_misses(
        xml,
        "NonLinear",
        "width",
        "height",
        min_width,
        min_height,
        max_width,
        max_height,
    )
}

#[cfg(test)]
pub(crate) fn companion_expanded_size_misses(
    xml: &str,
    min_width: Option<u64>,
    min_height: Option<u64>,
) -> bool {
    companion_expanded_size_range_misses(xml, min_width, min_height, None, None)
}

pub(crate) fn companion_expanded_size_range_misses(
    xml: &str,
    min_width: Option<u64>,
    min_height: Option<u64>,
    max_width: Option<u64>,
    max_height: Option<u64>,
) -> bool {
    elem_size_range_misses(
        xml,
        "Companion",
        "expandedWidth",
        "expandedHeight",
        min_width,
        min_height,
        max_width,
        max_height,
    )
}

#[cfg(test)]
pub(crate) fn companion_asset_size_misses(
    xml: &str,
    min_width: Option<u64>,
    min_height: Option<u64>,
) -> bool {
    companion_asset_size_range_misses(xml, min_width, min_height, None, None)
}

pub(crate) fn companion_asset_size_range_misses(
    xml: &str,
    min_width: Option<u64>,
    min_height: Option<u64>,
    max_width: Option<u64>,
    max_height: Option<u64>,
) -> bool {
    elem_size_range_misses(
        xml,
        "Companion",
        "assetWidth",
        "assetHeight",
        min_width,
        min_height,
        max_width,
        max_height,
    )
}

#[cfg(test)]
pub(crate) fn nonlinear_expanded_size_misses(
    xml: &str,
    min_width: Option<u64>,
    min_height: Option<u64>,
) -> bool {
    nonlinear_expanded_size_range_misses(xml, min_width, min_height, None, None)
}

pub(crate) fn nonlinear_expanded_size_range_misses(
    xml: &str,
    min_width: Option<u64>,
    min_height: Option<u64>,
    max_width: Option<u64>,
    max_height: Option<u64>,
) -> bool {
    elem_size_range_misses(
        xml,
        "NonLinear",
        "expandedWidth",
        "expandedHeight",
        min_width,
        min_height,
        max_width,
        max_height,
    )
}

fn https_elem_misses(xml: &str, elem: &str, required: bool) -> bool {
    let n_tags = elem_inners(xml, elem).len();
    if n_tags == 0 {
        return required;
    }
    let urls = collect_elem_texts(xml, elem);
    if urls.len() != n_tags {
        return true;
    }
    urls.iter()
        .any(|u| !u.trim().to_ascii_lowercase().starts_with("https://"))
}

pub(crate) fn parsed_expires(xml: &str) -> Option<Vec<u64>> {
    let texts = collect_elem_texts(xml, "Expires");
    if texts.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(texts.len());
    for t in texts {
        out.push(parse_vast_duration(&t)?);
    }
    Some(out)
}

pub(crate) fn expires_exceeds(xml: &str, max: u64) -> bool {
    match parsed_expires(xml) {
        Some(ds) if !ds.is_empty() => ds.iter().any(|d| *d > max),
        _ => true,
    }
}

pub(crate) fn expires_below(xml: &str, min: u64) -> bool {
    match parsed_expires(xml) {
        Some(ds) if !ds.is_empty() => ds.iter().any(|d| *d < min),
        _ => true,
    }
}

pub(crate) fn icon_x_position_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Icon", "xPosition", allowed)
}

pub(crate) fn icon_y_position_misses(xml: &str, allowed: &[String]) -> bool {
    attr_allowed_misses(xml, "Icon", "yPosition", allowed)
}

pub(crate) fn ad_serving_id_text(xml: &str) -> Option<String> {
    for t in collect_elem_texts(xml, "AdServingId") {
        let t = t.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    None
}

pub(crate) fn ad_serving_id_too_short(xml: &str, min: u64) -> bool {
    match ad_serving_id_text(xml) {
        Some(s) => (s.len() as u64) < min,
        None => true,
    }
}

pub(crate) fn universal_ad_id_text(xml: &str) -> Option<String> {
    for t in collect_elem_texts(xml, "UniversalAdId") {
        let t = t.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    attr_value(xml, "UniversalAdId", "idValue")
}

fn attr_allowed_misses(xml: &str, elem: &str, attr_name: &str, allowed: &[String]) -> bool {
    let mut found = Vec::new();
    for tag in elem_open_attr_tags(xml, elem) {
        if let Some(v) = attr(&tag, attr_name) {
            let t = v.trim();
            if !t.is_empty() {
                found.push(t.to_string());
            }
        }
    }
    if found.is_empty() {
        return true;
    }
    !found
        .iter()
        .any(|v| allowed.iter().any(|a| a.eq_ignore_ascii_case(v)))
}

pub(crate) fn parsed_durations(xml: &str) -> Option<Vec<u64>> {
    let texts = collect_elem_texts(xml, "Duration");
    if texts.is_empty() {
        let d = parse_vast_duration(vastlint_core::inspect_document(xml).duration.trim())?;
        return Some(vec![d]);
    }
    let mut out = Vec::with_capacity(texts.len());
    for t in texts {
        out.push(parse_vast_duration(&t)?);
    }
    Some(out)
}

pub(crate) fn duration_exceeds(xml: &str, max: u64) -> bool {
    match parsed_durations(xml) {
        Some(ds) if !ds.is_empty() => ds.iter().any(|d| *d > max),
        _ => true,
    }
}

pub(crate) fn duration_below(xml: &str, min: u64) -> bool {
    match parsed_durations(xml) {
        Some(ds) if !ds.is_empty() => ds.iter().any(|d| *d < min),
        _ => true,
    }
}

pub fn parse_vast_duration(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Ok(n) = s.parse::<u64>() {
        return Some(n);
    }
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: u64 = parts[0].parse().ok()?;
    let m: u64 = parts[1].parse().ok()?;
    let sec = parts[2].split('.').next()?.parse::<u64>().ok()?;
    Some(h * 3600 + m * 60 + sec)
}

pub fn aspect_matches(width: Option<u32>, height: Option<u32>, spec: &str) -> bool {
    let (w, h) = match (width, height) {
        (Some(w), Some(h)) if w > 0 && h > 0 => (w as u64, h as u64),
        _ => return false,
    };
    let spec = spec.trim();
    let mut it = spec.split(':');
    let aw: u64 = match it.next().and_then(|s| s.parse().ok()) {
        Some(n) if n > 0 => n,
        _ => return false,
    };
    let ah: u64 = match it.next().and_then(|s| s.parse().ok()) {
        Some(n) if n > 0 => n,
        _ => return false,
    };
    w * ah == h * aw
}

pub(crate) fn attr_value(xml: &str, elem: &str, attr_name: &str) -> Option<String> {
    for tag in elem_open_attr_tags(xml, elem) {
        if let Some(v) = attr(&tag, attr_name) {
            let t = v.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

pub(crate) fn elem_text(xml: &str, elem: &str) -> Option<String> {
    let rest = open_after_elem(xml, elem)?;
    cdata_or_text(rest, &format!("</{elem}>"))
}

pub(crate) fn open_after_elem<'a>(xml: &'a str, elem: &str) -> Option<&'a str> {
    let xml_l = xml.to_ascii_lowercase();
    let needle_l = format!("<{}", elem.to_ascii_lowercase());
    let mut from = 0;
    loop {
        let rel = xml_l[from..].find(&needle_l)?;
        let idx = from + rel;
        let after_i = idx + needle_l.len();
        let after = xml.get(after_i..)?;
        let next = after.chars().next();
        if next.is_none() || matches!(next, Some(' ' | '>' | '/' | '\n' | '\t' | '\r')) {
            return Some(after);
        }
        from = after_i;
    }
}

pub(crate) fn has_vpaid_framework(xml: &str) -> bool {
    for elem in ["MediaFile", "InteractiveCreativeFile", "NonLinear"] {
        for tag in elem_open_attr_tags(xml, elem) {
            if let Some(v) = attr(&tag, "apiFramework") {
                if v.eq_ignore_ascii_case("vpaid") {
                    return true;
                }
            }
        }
    }
    false
}

pub(crate) fn skipoffset_exceeds(xml: &str, max: u64) -> bool {
    for elem in ["Linear", "NonLinear"] {
        for tag in elem_open_attr_tags(xml, elem) {
            let Some(raw) = attr(&tag, "skipoffset") else {
                continue;
            };
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            if raw.contains('%') {
                return true;
            }
            match parse_vast_duration(raw) {
                Some(s) if s > max => return true,
                Some(_) => {}
                None => return true,
            }
        }
    }
    false
}

pub(crate) fn min_suggested_duration_below(xml: &str, min: u64) -> bool {
    for tag in elem_open_attr_tags(xml, "NonLinear") {
        let Some(raw) = attr(&tag, "minSuggestedDuration") else {
            return true;
        };
        let raw = raw.trim();
        if raw.is_empty() || raw.contains('%') {
            return true;
        }
        match parse_vast_duration(raw) {
            Some(s) if s < min => return true,
            Some(_) => {}
            None => return true,
        }
    }
    false
}

pub(crate) fn min_icon_duration_below(xml: &str, min: u64) -> bool {
    for tag in elem_open_attr_tags(xml, "Icon") {
        let Some(raw) = attr(&tag, "duration") else {
            return true;
        };
        let raw = raw.trim();
        if raw.is_empty() || raw.contains('%') {
            return true;
        }
        match parse_vast_duration(raw) {
            Some(s) if s < min => return true,
            Some(_) => {}
            None => return true,
        }
    }
    false
}

pub(crate) fn max_icon_offset_exceeds(xml: &str, max: u64) -> bool {
    for tag in elem_open_attr_tags(xml, "Icon") {
        let Some(raw) = attr(&tag, "offset") else {
            continue;
        };
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        if raw.contains('%') {
            return true;
        }
        match parse_vast_duration(raw) {
            Some(s) if s > max => return true,
            Some(_) => {}
            None => return true,
        }
    }
    false
}

#[cfg(test)]
pub(crate) fn companion_size_misses(
    xml: &str,
    min_width: Option<u64>,
    min_height: Option<u64>,
) -> bool {
    companion_size_range_misses(xml, min_width, min_height, None, None)
}

pub(crate) fn companion_size_range_misses(
    xml: &str,
    min_width: Option<u64>,
    min_height: Option<u64>,
    max_width: Option<u64>,
    max_height: Option<u64>,
) -> bool {
    elem_size_range_misses(
        xml,
        "Companion",
        "width",
        "height",
        min_width,
        min_height,
        max_width,
        max_height,
    )
}

fn elem_size_range_misses(
    xml: &str,
    elem: &str,
    width_attr: &str,
    height_attr: &str,
    min_width: Option<u64>,
    min_height: Option<u64>,
    max_width: Option<u64>,
    max_height: Option<u64>,
) -> bool {
    if min_width.is_none() && min_height.is_none() && max_width.is_none() && max_height.is_none() {
        return false;
    }
    for tag in elem_open_attr_tags(xml, elem) {
        let w = attr(&tag, width_attr).and_then(|s| s.trim().parse::<u64>().ok());
        let h = attr(&tag, height_attr).and_then(|s| s.trim().parse::<u64>().ok());
        let width_ok = if min_width.is_none() && max_width.is_none() {
            true
        } else {
            w.is_some_and(|v| {
                min_width.map(|m| v >= m).unwrap_or(true)
                    && max_width.map(|m| v <= m).unwrap_or(true)
            })
        };
        let height_ok = if min_height.is_none() && max_height.is_none() {
            true
        } else {
            h.is_some_and(|v| {
                min_height.map(|m| v >= m).unwrap_or(true)
                    && max_height.map(|m| v <= m).unwrap_or(true)
            })
        };
        if width_ok && height_ok {
            return false;
        }
    }
    true
}

fn elem_open_attr_tags(xml: &str, elem: &str) -> Vec<String> {
    let xml_l = xml.to_ascii_lowercase();
    let needle_l = format!("<{}", elem.to_ascii_lowercase());
    let mut from = 0;
    let mut out = Vec::new();
    while let Some(rel) = xml_l[from..].find(&needle_l) {
        let after_i = from + rel + needle_l.len();
        from = after_i;
        let Some(after) = xml.get(after_i..) else {
            break;
        };
        let next = after.chars().next();
        if next.is_none() || matches!(next, Some(' ' | '>' | '/' | '\n' | '\t' | '\r')) {
            if let Some(end) = after.find('>') {
                out.push(after[..end].to_string());
                from = after_i + end + 1;
            }
        }
    }
    out
}

fn all_media_file_attr_tags(xml: &str) -> Vec<String> {
    elem_open_attr_tags(xml, "MediaFile")
}

#[cfg(test)]
pub(crate) fn media_mimes(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    for tag in all_media_file_attr_tags(xml) {
        if let Some(t) = attr(&tag, "type") {
            let t = t.trim();
            if !t.is_empty() && !out.iter().any(|e: &String| e.eq_ignore_ascii_case(t)) {
                out.push(t.to_string());
            }
        }
    }
    out
}

#[cfg(test)]
pub(crate) fn media_containers(xml: &str) -> Vec<String> {
    let tags = all_media_file_attr_tags(xml);
    let urls = collect_elem_texts(xml, "MediaFile");
    let mut out = Vec::new();
    for (i, tag) in tags.iter().enumerate() {
        let mime = attr(tag, "type");
        let url = urls.get(i).cloned();
        if let Some(c) = container_of(mime.as_deref(), url.as_deref()) {
            if !out.iter().any(|e: &String| e.eq_ignore_ascii_case(&c)) {
                out.push(c);
            }
        }
    }
    out
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MediaFileRow {
    pub mime: Option<String>,
    pub container: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bitrate: Option<u64>,
    pub codecs_hay: Option<String>,
    pub delivery: Option<String>,
}

pub(crate) fn media_file_rows(xml: &str) -> Vec<MediaFileRow> {
    let tags = all_media_file_attr_tags(xml);
    let urls = collect_elem_texts(xml, "MediaFile");
    let files = vastlint_core::inspect_document(xml).media_files;
    if tags.is_empty() {
        return files.iter().map(row_from_inspect).collect();
    }
    tags.iter()
        .enumerate()
        .map(|(i, tag)| {
            let mime = attr(tag, "type");
            let url = urls.get(i).cloned();
            let codecs = attr(tag, "codecs");
            let codec = attr(tag, "codec");
            let hay = [codecs.as_deref(), codec.as_deref()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(",")
                .to_ascii_lowercase();
            let mut row = MediaFileRow {
                mime: mime.clone(),
                container: container_of(mime.as_deref(), url.as_deref()),
                width: attr(tag, "width").and_then(|s| s.parse().ok()),
                height: attr(tag, "height").and_then(|s| s.parse().ok()),
                bitrate: attr(tag, "bitrate")
                    .and_then(|s| s.parse().ok())
                    .or_else(|| attr(tag, "maxBitrate").and_then(|s| s.parse().ok())),
                codecs_hay: if hay.is_empty() { None } else { Some(hay) },
                delivery: attr(tag, "delivery"),
            };
            if let Some(m) = files.get(i) {
                fill_row_from_inspect(&mut row, m);
            }
            row
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn media_sizes(xml: &str) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    for tag in all_media_file_attr_tags(xml) {
        let w = attr(&tag, "width").and_then(|s| s.parse().ok());
        let h = attr(&tag, "height").and_then(|s| s.parse().ok());
        if let (Some(w), Some(h)) = (w, h) {
            if w > 0 && h > 0 && !out.contains(&(w, h)) {
                out.push((w, h));
            }
        }
    }
    out
}

#[cfg(test)]
pub(crate) fn media_bitrates(xml: &str) -> Vec<u64> {
    let mut out = Vec::new();
    for tag in all_media_file_attr_tags(xml) {
        if let Some(b) = attr(&tag, "bitrate")
            .and_then(|s| s.parse().ok())
            .or_else(|| attr(&tag, "maxBitrate").and_then(|s| s.parse().ok()))
        {
            if !out.contains(&b) {
                out.push(b);
            }
        }
    }
    out
}

#[cfg(test)]
pub(crate) fn media_codec_hays(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    for tag in all_media_file_attr_tags(xml) {
        let mut parts = Vec::new();
        if let Some(c) = attr(&tag, "codecs") {
            parts.push(c);
        }
        if let Some(c) = attr(&tag, "codec") {
            parts.push(c);
        }
        let hay = parts.join(",").to_ascii_lowercase();
        if !hay.is_empty() && !out.iter().any(|e| e == &hay) {
            out.push(hay);
        }
    }
    out
}

fn attr(attrs: &str, name: &str) -> Option<String> {
    let attrs_l = attrs.to_ascii_lowercase();
    let name_l = name.to_ascii_lowercase();
    let bytes = attrs_l.as_bytes();
    let mut from = 0;
    while let Some(rel) = attrs_l[from..].find(&name_l) {
        let abs = from + rel;
        let before_ok = abs == 0 || !bytes[abs - 1].is_ascii_alphanumeric();
        let mut i = abs + name_l.len();
        if before_ok {
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if bytes.get(i) == Some(&b'=') {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                let q = bytes.get(i).copied().filter(|c| *c == b'"' || *c == b'\'');
                if let Some(q) = q {
                    let start = i + 1;
                    let end = attrs.get(start..)?.find(q as char)?;
                    return Some(attrs[start..start + end].to_string());
                }
            }
        }
        from = abs + 1;
    }
    None
}

fn cdata_or_text(rest: &str, close: &str) -> Option<String> {
    let start = rest.find('>')? + 1;
    let body = &rest[start..];
    let body_l = body.to_ascii_lowercase();
    let close_l = close.to_ascii_lowercase();
    let end = body_l.find(&close_l)?;
    let mut text = body[..end].trim();
    if text.len() >= 9 && text[..9].eq_ignore_ascii_case("<![CDATA[") {
        text = text[9..].trim();
    }
    if text.len() >= 3 && text[text.len() - 3..].eq_ignore_ascii_case("]]>") {
        text = text[..text.len() - 3].trim();
    }
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_https_mp4_fixture() {
        let xml = include_str!("../../../fixtures/ok-https-mp4.xml");
        let f = media_facts(xml);
        assert_eq!(f.duration_seconds, Some(30));
        assert_eq!(f.width, Some(640));
        assert_eq!(f.height, Some(360));
        assert_eq!(f.mime.as_deref(), Some("video/mp4"));
        assert_eq!(f.bitrate, Some(1500));
        assert_eq!(f.container.as_deref(), Some("mp4"));
        assert!(aspect_matches(f.width, f.height, "16:9"));
        assert!(f.tracking_events.contains(&"start".into()));
        assert!(f.tracking_events.contains(&"complete".into()));
        assert_eq!(f.privacy_macros, Some(true));
        assert_eq!(
            media_url(xml).as_deref(),
            Some("https://cdn.example.com/ad.mp4")
        );
        let inspected = vastlint_core::inspect_document(xml);
        assert_eq!(inspected.duration, "00:00:30");
        assert_eq!(inspected.media_files[0].mime_type, "video/mp4");
        assert_eq!(inspected.media_files[0].width, "640");
        assert_eq!(inspected.media_files[0].bitrate, "1500");
    }

    #[test]
    fn whitespace_bracketed_macros_count_and_title_prose_does_not() {
        assert!(has_privacy_macros(
            r#"<Impression>https://t.example.com/i?cb=[ CACHEBUSTING ]&amp;ts=[ TIMESTAMP ]</Impression>"#
        ));
        assert!(xml_has_macro(
            r#"<Tracking>https://t.example.com/s?cb=${CACHEBUSTING}</Tracking>"#,
            "CACHEBUSTING"
        ));
        assert!(!has_privacy_macros(
            r#"<AdTitle>CACHEBUSTING TIMESTAMP</AdTitle>"#
        ));
        let fixture = include_str!("../../../fixtures/ok-https-mp4.xml");
        assert!(xml_has_macro(fixture, "CACHEBUSTING"));
        assert!(xml_has_macro(fixture, "TIMESTAMP"));
        assert!(!xml_has_macro(
            r#"<AdTitle>[CACHEBUSTING] [TIMESTAMP]</AdTitle><Impression>https://t.example.com/imp</Impression>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<AdTitle>[CACHEBUSTING]</AdTitle><ViewableImpression><Viewable><![CDATA[https://t.example.com/v?cb=[CACHEBUSTING]&ts=[TIMESTAMP]]]></Viewable></ViewableImpression>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<ViewableImpression><NotViewable>https://t.example.com/nv?cb=[CACHEBUSTING]</NotViewable></ViewableImpression>"#,
            "CACHEBUSTING"
        ));
        assert!(!xml_has_macro(
            r#"<AdTitle>[CACHEBUSTING] [TIMESTAMP]</AdTitle><ViewableImpression id="x"></ViewableImpression>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<IconClickTracking><![CDATA[https://t.example.com/icon?cb=[CACHEBUSTING]]]></IconClickTracking>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<CompanionClickTracking>https://t.example.com/comp?cb=[CACHEBUSTING]</CompanionClickTracking>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<IconViewTracking>https://t.example.com/iv?cb=[CACHEBUSTING]</IconViewTracking>"#,
            "CACHEBUSTING"
        ));
        assert!(!xml_has_macro(
            r#"<AdTitle>[CACHEBUSTING]</AdTitle><IconClicks></IconClicks><CompanionAds></CompanionAds>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<NonLinearClickTracking>https://t.example.com/nl?cb=[CACHEBUSTING]</NonLinearClickTracking>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<ClickThrough><![CDATA[https://adv.example.com/l?cb=[CACHEBUSTING]]]></ClickThrough>"#,
            "CACHEBUSTING"
        ));
        assert!(!xml_has_macro(
            r#"<AdTitle>[CACHEBUSTING]</AdTitle><VideoClicks></VideoClicks><NonLinearAds></NonLinearAds>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<Survey><![CDATA[https://t.example.com/survey?cb=[CACHEBUSTING]]]></Survey>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<Extensions><Extension type="custom"><Pixel>https://t.example.com/ext?cb=[CACHEBUSTING]</Pixel></Extension></Extensions>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<JavaScriptResource apiFramework="omid"><![CDATA[https://t.example.com/omid.js?cb=[CACHEBUSTING]]]></JavaScriptResource>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<IconClickThrough><![CDATA[https://adv.example.com/icon?cb=[CACHEBUSTING]]]></IconClickThrough>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<StaticResource creativeType="image/png"><![CDATA[https://cdn.example.com/i.png?cb=[CACHEBUSTING]]]></StaticResource>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<IFrameResource><![CDATA[https://cdn.example.com/frame.html?cb=[CACHEBUSTING]]]></IFrameResource>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<HTMLResource><![CDATA[<img src="https://t.example.com/i.png?cb=[CACHEBUSTING]" />]]></HTMLResource>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<FlashResource><![CDATA[https://cdn.example.com/ad.swf?cb=[CACHEBUSTING]]]></FlashResource>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<AdParameters><![CDATA[{"url":"https://t.example.com/vpaid?cb=[CACHEBUSTING]"}]]></AdParameters>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<IconClickFallbackImage width="300" height="250"><![CDATA[https://cdn.example.com/icon.png?cb=[CACHEBUSTING]]]></IconClickFallbackImage>"#,
            "CACHEBUSTING"
        ));
        assert!(xml_has_macro(
            r#"<VerificationParameters><![CDATA[{"ping":"https://t.example.com/omid?cb=[CACHEBUSTING]"}]]></VerificationParameters>"#,
            "CACHEBUSTING"
        ));
        assert!(!xml_has_macro(
            r#"<AdTitle>[CACHEBUSTING]</AdTitle><HTMLResource></HTMLResource><AdParameters></AdParameters>"#,
            "CACHEBUSTING"
        ));
        assert!(!xml_has_macro(
            r#"<AdTitle>[CACHEBUSTING]</AdTitle><Extensions></Extensions><AdVerifications></AdVerifications>"#,
            "CACHEBUSTING"
        ));
    }

    #[test]
    fn maxbitrate_attr_fills_bitrate() {
        let xml = r#"<vast><ad><inline><creative><linear>
<mediafile type="video/mp4" maxBitrate="4000"><![cdata[https://cdn.example.com/ad.mp4]]></mediafile>
</linear></creative></inline></ad></vast>"#;
        assert_eq!(media_facts(xml).bitrate, Some(4000));
    }

    #[test]
    fn later_mediafile_fills_codecs_and_maxbitrate() {
        let xml = r#"<VAST version="4.2"><Ad><InLine>
<Creative><Linear>
<Duration>00:00:15</Duration>
<MediaFiles>
<MediaFile delivery="progressive" type="video/mp4" width="640" height="360"><![CDATA[https://cdn.example.com/a.mp4]]></MediaFile>
<MediaFile delivery="progressive" type="video/mp4" width="640" height="360" maxBitrate="4000" codecs="avc1.42E01E,mp4a.40.2"><![CDATA[https://cdn.example.com/b.mp4]]></MediaFile>
</MediaFiles>
</Linear></Creative>
</InLine></Ad></VAST>"#;
        let f = media_facts(xml);
        assert_eq!(
            media_url(xml).as_deref(),
            Some("https://cdn.example.com/b.mp4")
        );
        assert_eq!(f.bitrate, Some(4000));
        assert_eq!(f.codec.as_deref(), Some("avc1.42E01E"));
        assert_eq!(f.audio_codec.as_deref(), Some("mp4a.40.2"));
    }

    #[test]
    fn later_https_mediafile_is_preferred_over_http() {
        let xml = r#"<VAST version="4.2"><Ad><InLine>
<Creative><Linear>
<MediaFiles>
<MediaFile delivery="progressive" type="video/mp4" width="640" height="360" bitrate="200"><![CDATA[http://cdn.example.com/a.mp4]]></MediaFile>
<MediaFile delivery="progressive" type="video/mp4" width="640" height="360" bitrate="1500"><![CDATA[https://cdn.example.com/b.mp4]]></MediaFile>
</MediaFiles>
</Linear></Creative>
</InLine></Ad></VAST>"#;
        assert_eq!(
            media_url(xml).as_deref(),
            Some("https://cdn.example.com/b.mp4")
        );
        assert!(any_https_mediafile(xml));
        let f = media_facts(xml);
        assert_eq!(f.bitrate, Some(1500));
        assert_eq!(f.mime.as_deref(), Some("video/mp4"));
    }

    #[test]
    fn inspect_fills_spaced_mediafile_attrs() {
        let xml = r#"<VAST version="4.2"><Ad><InLine>
<Creative><Linear>
<Duration>00:00:15</Duration>
<MediaFiles>
<MediaFile delivery="progressive" type="video/mp4" width="640" height="360" bitrate = "4000"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>
</MediaFiles>
</Linear></Creative>
</InLine></Ad></VAST>"#;
        let f = media_facts(xml);
        assert_eq!(f.bitrate, Some(4000));
        assert_eq!(f.width, Some(640));
        assert_eq!(f.mime.as_deref(), Some("video/mp4"));
        let rows = media_file_rows(xml);
        assert_eq!(rows[0].bitrate, Some(4000));
        assert_eq!(
            media_url(xml).as_deref(),
            Some("https://cdn.example.com/ad.mp4")
        );
    }

    #[test]
    fn later_mediafile_mime_is_collected() {
        let xml = r#"<VAST version="4.2"><Ad><InLine>
<Creative><Linear>
<MediaFiles>
<MediaFile delivery="progressive" type="video/webm" width="640" height="360"><![CDATA[https://cdn.example.com/a.webm]]></MediaFile>
<MediaFile delivery="progressive" type="video/mp4" width="640" height="360"><![CDATA[https://cdn.example.com/b.mp4]]></MediaFile>
</MediaFiles>
</Linear></Creative>
</InLine></Ad></VAST>"#;
        let mimes = media_mimes(xml);
        assert!(mimes.iter().any(|m| m.eq_ignore_ascii_case("video/webm")));
        assert!(mimes.iter().any(|m| m.eq_ignore_ascii_case("video/mp4")));
    }

    #[test]
    fn later_mediafile_container_and_size_are_collected() {
        let xml = r#"<VAST version="4.2"><Ad><InLine>
<Creative><Linear>
<MediaFiles>
<MediaFile delivery="progressive" type="video/webm" width="320" height="240"><![CDATA[https://cdn.example.com/a.webm]]></MediaFile>
<MediaFile delivery="progressive" type="video/mp4" width="640" height="360"><![CDATA[https://cdn.example.com/b.mp4]]></MediaFile>
</MediaFiles>
</Linear></Creative>
</InLine></Ad></VAST>"#;
        let containers = media_containers(xml);
        assert!(containers.iter().any(|c| c.eq_ignore_ascii_case("webm")));
        assert!(containers.iter().any(|c| c.eq_ignore_ascii_case("mp4")));
        let sizes = media_sizes(xml);
        assert!(sizes.contains(&(320, 240)));
        assert!(sizes.contains(&(640, 360)));
    }

    #[test]
    fn later_mediafile_bitrate_and_codecs_are_collected() {
        let xml = r#"<VAST version="4.2"><Ad><InLine>
<Creative><Linear>
<MediaFiles>
<MediaFile delivery="progressive" type="video/mp4" width="640" height="360" bitrate="200" codecs="vp09.00.10.08"><![CDATA[https://cdn.example.com/a.mp4]]></MediaFile>
<MediaFile delivery="progressive" type="video/mp4" width="640" height="360" bitrate="1500" codecs="avc1.42E01E,mp4a.40.2"><![CDATA[https://cdn.example.com/b.mp4]]></MediaFile>
</MediaFiles>
</Linear></Creative>
</InLine></Ad></VAST>"#;
        let rates = media_bitrates(xml);
        assert!(rates.contains(&200));
        assert!(rates.contains(&1500));
        let hays = media_codec_hays(xml);
        assert!(hays.iter().any(|h| h.contains("vp09")));
        assert!(hays.iter().any(|h| h.contains("avc1")));
        assert!(hays.iter().any(|h| h.contains("mp4a")));
        assert_eq!(
            media_url(xml).as_deref(),
            Some("https://cdn.example.com/b.mp4")
        );
    }

    #[test]
    fn later_https_mezzanine_is_preferred_over_http() {
        let xml = r#"<VAST version="4.2"><Ad><InLine>
<Creative><Linear>
<MediaFiles>
<Mezzanine delivery="progressive" type="video/mp4"><![CDATA[http://cdn.example.com/a.mp4]]></Mezzanine>
<Mezzanine delivery="progressive" type="video/mp4"><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>
</MediaFiles>
</Linear></Creative>
</InLine></Ad></VAST>"#;
        assert_eq!(
            mezzanine_url(xml).as_deref(),
            Some("https://cdn.example.com/mezz.mp4")
        );
        assert!(any_https_mezzanine(xml));
    }

    #[test]
    fn parses_declared_codecs() {
        let xml = include_str!("../../../fixtures/ok-codecs.xml");
        let f = media_facts(xml);
        assert_eq!(f.codec.as_deref(), Some("avc1.42E01E"));
        assert_eq!(f.audio_codec.as_deref(), Some("mp4a.40.2"));
        assert_eq!(f.container.as_deref(), Some("mp4"));
    }

    #[test]
    fn mixed_case_tags_and_single_quoted_attrs() {
        let xml = r#"<vast version="4.2"><ad><inline>
<creative><linear>
<duration>00:00:15</duration>
<trackingevents>
<tracking event='start'><![cdata[https://t.example.com/s]]></tracking>
</trackingevents>
<mediafiles>
<mediafile delivery='progressive' type='video/mp4' width='640' height='360'><![cdata[https://cdn.example.com/ad.mp4]]></mediafile>
</mediafiles>
</linear></creative>
</inline></ad></vast>"#;
        let f = media_facts(xml);
        assert_eq!(f.duration_seconds, Some(15));
        assert_eq!(f.mime.as_deref(), Some("video/mp4"));
        assert_eq!(f.width, Some(640));
        assert_eq!(f.height, Some(360));
        assert_eq!(f.container.as_deref(), Some("mp4"));
        assert!(f.tracking_events.contains(&"start".into()));
        assert_eq!(
            media_url(xml).as_deref(),
            Some("https://cdn.example.com/ad.mp4")
        );
        assert!(elem_text(xml, "MediaFile").is_some());
        assert!(elem_text(xml, "Duration").is_some());
    }

    #[test]
    fn linear_tracking_ignores_companion_events() {
        let xml = r#"<VAST version="4.2"><Ad><InLine>
<Creative>
<Linear>
<Duration>00:00:15</Duration>
<MediaFiles>
<MediaFile type="video/mp4"><![CDATA[https://cdn.example.com/ad.mp4]]></MediaFile>
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
</Creative>
</InLine></Ad></VAST>"#;
        assert!(tracking_events(xml).is_empty());
    }

    #[test]
    fn spaced_tracking_event_attr_is_read() {
        let xml = r#"<VAST version="4.2"><Ad><InLine>
<Creative><Linear>
<TrackingEvents>
<Tracking event = "start"><![CDATA[https://t.example.com/s]]></Tracking>
</TrackingEvents>
</Linear></Creative>
</InLine></Ad></VAST>"#;
        assert!(tracking_events(xml).contains(&"start".into()));
    }

    #[test]
    fn blank_category_text_is_dropped() {
        let xml = r#"<VAST version="4.2"><Ad><InLine>
<Category>   </Category>
<Category><![CDATA[IAB1-1]]></Category>
</InLine></Ad></VAST>"#;
        let cats = categories(xml);
        assert_eq!(cats, vec!["IAB1-1".to_string()]);
    }

    #[test]
    fn empty_universaladid_has_no_text() {
        assert!(elem_text(
            "<vast><UniversalAdId></UniversalAdId></vast>",
            "UniversalAdId"
        )
        .is_none());
        assert!(elem_text(
            "<vast><UniversalAdId idRegistry=\"ad-id.org\"/></vast>",
            "UniversalAdId"
        )
        .is_none());
        assert!(elem_text(
            "<vast><UniversalAdId>   </UniversalAdId></vast>",
            "UniversalAdId"
        )
        .is_none());
        assert_eq!(
            universal_ad_id_text(
                r#"<vast><UniversalAdId idRegistry="ad-id.org" idValue="ABCD1234567"/></vast>"#
            )
            .as_deref(),
            Some("ABCD1234567")
        );
        assert_eq!(
            universal_ad_id_text(
                r#"<vast><UniversalAdId></UniversalAdId><UniversalAdId>UAD-2</UniversalAdId></vast>"#
            )
            .as_deref(),
            Some("UAD-2")
        );
        assert!(
            universal_ad_id_text("<vast><UniversalAdId idRegistry=\"ad-id.org\"/></vast>")
                .is_none()
        );
    }

    #[test]
    fn vpaid_api_framework_is_detected_on_later_mediafile() {
        let xml = include_str!("../../../fixtures/vpaid.xml");
        assert!(has_vpaid_framework(xml));
        let quoted = r#"<vast><ad><inline><creative><linear><mediafiles>
<mediafile type="video/mp4">https://cdn.example.com/ad.mp4</mediafile>
<mediafile type='application/javascript' apiframework='VPAID'>https://cdn.example.com/v.js</mediafile>
</mediafiles></linear></creative></inline></ad></vast>"#;
        assert!(has_vpaid_framework(quoted));
        assert!(!has_vpaid_framework(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        let icf = r#"<VAST><Ad><InLine><Creative>
<Linear><MediaFiles>
<MediaFile type="video/mp4">https://cdn.example.com/ad.mp4</MediaFile>
</MediaFiles></Linear>
<NonLinearAds>
<InteractiveCreativeFile type="application/javascript">https://cdn.example.com/a.js</InteractiveCreativeFile>
<InteractiveCreativeFile type="application/javascript" apiFramework="VPAID">https://cdn.example.com/v.js</InteractiveCreativeFile>
</NonLinearAds>
</Creative></InLine></Ad></VAST>"#;
        assert!(has_vpaid_framework(icf));
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
        assert!(has_vpaid_framework(nonlinear));
    }

    #[test]
    fn skipoffset_exceeds_duration_and_percent() {
        let ok = r#"<VAST><Ad><InLine><Creative>
<Linear skipoffset="00:00:03"><Duration>00:00:30</Duration></Linear>
</Creative></InLine></Ad></VAST>"#;
        assert!(!skipoffset_exceeds(ok, 5));
        let late = r#"<VAST><Ad><InLine><Creative>
<Linear><Duration>00:00:30</Duration></Linear>
<Linear skipoffset="00:00:10"><Duration>00:00:30</Duration></Linear>
</Creative></InLine></Ad></VAST>"#;
        assert!(skipoffset_exceeds(late, 5));
        let pct = r#"<VAST><Ad><InLine><Creative>
<Linear skipoffset="25%"><Duration>00:00:30</Duration></Linear>
</Creative></InLine></Ad></VAST>"#;
        assert!(skipoffset_exceeds(pct, 5));
        let spaced = r#"<VAST><Ad><InLine><Creative>
<Linear skipoffset = "00:00:10"><Duration>00:00:30</Duration></Linear>
</Creative></InLine></Ad></VAST>"#;
        assert!(skipoffset_exceeds(spaced, 5));
        let nonlinear = r#"<VAST><Ad><InLine><Creative>
<Linear><Duration>00:00:30</Duration></Linear>
<NonLinearAds>
<NonLinear width="300" height="250" skipoffset="00:00:10">
<StaticResource>https://cdn.example.com/nl.png</StaticResource>
</NonLinear>
</NonLinearAds>
</Creative></InLine></Ad></VAST>"#;
        assert!(skipoffset_exceeds(nonlinear, 5));
        assert!(!skipoffset_exceeds(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            0
        ));
    }

    #[test]
    fn later_linear_duration_is_collected() {
        let later = r#"<VAST><Ad><InLine><Creatives>
<Creative><Linear><Duration>00:00:15</Duration></Linear></Creative>
<Creative><Linear><Duration>00:01:00</Duration>
<MediaFiles><MediaFile type="video/mp4">https://cdn.example.com/ad.mp4</MediaFile></MediaFiles>
</Linear></Creative>
</Creatives></InLine></Ad></VAST>"#;
        assert_eq!(media_facts(later).duration_seconds, Some(60));
        assert!(duration_exceeds(later, 30));
        assert!(!duration_below(later, 6));
        let short_then_ok = r#"<VAST><Ad><InLine><Creatives>
<Creative><Linear><Duration>00:00:05</Duration></Linear></Creative>
<Creative><Linear><Duration>00:00:30</Duration></Linear></Creative>
</Creatives></InLine></Ad></VAST>"#;
        assert!(duration_below(short_then_ok, 6));
        assert_eq!(media_facts(short_then_ok).duration_seconds, Some(30));
        let empty_then_ok = r#"<VAST><Ad><InLine><Creatives>
<Creative><Linear><Duration></Duration></Linear></Creative>
<Creative><Linear><Duration>00:00:15</Duration></Linear></Creative>
</Creatives></InLine></Ad></VAST>"#;
        assert_eq!(media_facts(empty_then_ok).duration_seconds, Some(15));
        assert!(!duration_below(empty_then_ok, 6));
        assert!(!duration_exceeds(empty_then_ok, 30));
        let garbage = r#"<VAST><Ad><InLine><Creatives>
<Creative><Linear><Duration>n/a</Duration></Linear></Creative>
<Creative><Linear><Duration>00:00:15</Duration></Linear></Creative>
</Creatives></InLine></Ad></VAST>"#;
        assert_eq!(media_facts(garbage).duration_seconds, None);
        assert!(duration_exceeds(garbage, 30));
        assert!(duration_below(garbage, 6));
        let only_garbage = r#"<VAST><Ad><InLine><Creative>
<Linear><Duration>not-a-clock</Duration></Linear>
</Creative></InLine></Ad></VAST>"#;
        assert!(duration_exceeds(only_garbage, 60));
        assert!(duration_below(only_garbage, 1));
    }

    #[test]
    fn caption_language_reads_later_file_and_spaced_attr() {
        let missing = include_str!("../../../fixtures/ok-https-mp4.xml");
        assert!(caption_language_misses(missing, &["en".to_string()]));
        let later = r#"<VAST><Ad><InLine><Creative><Linear><MediaFiles>
<ClosedCaptionFile type="text/vtt"><![CDATA[https://cdn.example.com/a.vtt]]></ClosedCaptionFile>
<ClosedCaptionFile type="text/vtt" language = "en"><![CDATA[https://cdn.example.com/ad.vtt]]></ClosedCaptionFile>
</MediaFiles></Linear></Creative></InLine></Ad></VAST>"#;
        assert!(!caption_language_misses(later, &["en".to_string()]));
        assert!(caption_language_misses(later, &["es".to_string()]));
        assert!(!caption_type_misses(later, &["text/vtt".to_string()]));
        assert!(caption_type_misses(
            later,
            &["application/ttml+xml".to_string()]
        ));
        let no_type = r#"<VAST><Ad><InLine><Creative><Linear><MediaFiles>
<ClosedCaptionFile language="en"><![CDATA[https://cdn.example.com/ad.vtt]]></ClosedCaptionFile>
</MediaFiles></Linear></Creative></InLine></Ad></VAST>"#;
        assert!(caption_type_misses(no_type, &["text/vtt".to_string()]));
        let later_reg = r#"<VAST><Ad><InLine>
<UniversalAdId>ABCD1234567</UniversalAdId>
<UniversalAdId idRegistry = "ad-id.org">ABCD1234567</UniversalAdId>
</InLine></Ad></VAST>"#;
        assert!(!id_registry_misses(later_reg, &["ad-id.org".to_string()]));
        assert!(id_registry_misses(later_reg, &["clearcast".to_string()]));
        assert!(id_registry_misses(
            "<VAST><UniversalAdId>ABCD1234567</UniversalAdId></VAST>",
            &["ad-id.org".to_string()]
        ));
    }

    #[test]
    fn icon_program_and_min_suggested_duration() {
        assert!(icon_program_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["adChoices".to_string()]
        ));
        let later = r#"<VAST><Ad><InLine><Creative><Linear>
<Icons>
<Icon program="other" width="20" height="20"></Icon>
<Icon program = "adChoices" width="20" height="20"></Icon>
</Icons>
</Linear></Creative></InLine></Ad></VAST>"#;
        assert!(!icon_program_misses(later, &["adChoices".to_string()]));
        assert!(icon_program_misses(later, &["daiy".to_string()]));
        assert!(!min_suggested_duration_below(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            6
        ));
        let short = r#"<VAST><Ad><InLine><Creative>
<NonLinearAds>
<NonLinear width="300" height="250" minSuggestedDuration="00:00:03"></NonLinear>
</NonLinearAds>
</Creative></InLine></Ad></VAST>"#;
        assert!(min_suggested_duration_below(short, 6));
        let ok = r#"<VAST><Ad><InLine><Creative>
<NonLinearAds>
<NonLinear width="300" height="250"></NonLinear>
<NonLinear width="300" height="250" minSuggestedDuration = "00:00:15"></NonLinear>
</NonLinearAds>
</Creative></InLine></Ad></VAST>"#;
        assert!(min_suggested_duration_below(ok, 6));
        let both_ok = r#"<VAST><Ad><InLine><Creative>
<NonLinearAds>
<NonLinear width="300" height="250" minSuggestedDuration="00:00:15"></NonLinear>
</NonLinearAds>
</Creative></InLine></Ad></VAST>"#;
        assert!(!min_suggested_duration_below(both_ok, 6));
        assert!(ad_system_version_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["1.0".to_string()]
        ));
        let later_sys = r#"<VAST><Ad><InLine>
<AdSystem>Test</AdSystem>
<AdSystem version = "1.0">Other</AdSystem>
</InLine></Ad></VAST>"#;
        assert!(!ad_system_version_misses(later_sys, &["1.0".to_string()]));
        assert!(!min_icon_duration_below(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            6
        ));
        let short_icon = r#"<VAST><Ad><InLine><Creative><Linear>
<Icons><Icon program="adChoices" duration="00:00:03"></Icon></Icons>
</Linear></Creative></InLine></Ad></VAST>"#;
        assert!(min_icon_duration_below(short_icon, 6));
        let later_icon = r#"<VAST><Ad><InLine><Creative><Linear>
<Icons>
<Icon program="adChoices"></Icon>
<Icon program="adChoices" duration = "00:00:15"></Icon>
</Icons>
</Linear></Creative></InLine></Ad></VAST>"#;
        assert!(min_icon_duration_below(later_icon, 6));
        let ok_icon = r#"<VAST><Ad><InLine><Creative><Linear>
<Icons><Icon program="adChoices" duration="00:00:15" offset="00:00:00"></Icon></Icons>
</Linear></Creative></InLine></Ad></VAST>"#;
        assert!(!min_icon_duration_below(ok_icon, 6));
        assert!(!max_icon_offset_exceeds(ok_icon, 0));
        let late_off = r#"<VAST><Ad><InLine><Creative><Linear>
<Icons><Icon program="adChoices" offset="00:00:10"></Icon></Icons>
</Linear></Creative></InLine></Ad></VAST>"#;
        assert!(max_icon_offset_exceeds(late_off, 5));
        assert!(companion_size_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            Some(300),
            None
        ));
        let later_comp = r#"<VAST><Ad><InLine><Creative>
<CompanionAds>
<Companion width="1" height="1"></Companion>
<Companion width = "300" height="250">
<StaticResource>https://cdn.example.com/c.png</StaticResource>
</Companion>
</CompanionAds>
</Creative></InLine></Ad></VAST>"#;
        assert!(!companion_size_misses(later_comp, Some(300), None));
        assert!(companion_size_misses(later_comp, Some(640), None));
        assert!(companion_size_misses(later_comp, Some(300), Some(600)));
        assert!(!companion_size_misses(later_comp, Some(300), Some(250)));
        let split = r#"<VAST><Ad><InLine><Creative>
<CompanionAds>
<Companion width="300" height="1"></Companion>
<Companion width="1" height="250"></Companion>
</CompanionAds>
</Creative></InLine></Ad></VAST>"#;
        assert!(companion_size_misses(split, Some(300), Some(250)));
        assert!(icon_x_position_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["right".to_string()]
        ));
        let pos = r#"<VAST><Ad><InLine><Creative><Linear>
<Icons>
<Icon program="adChoices" xPosition="left" yPosition="bottom"></Icon>
<Icon program="adChoices" xPosition = "right" yPosition = "top"></Icon>
</Icons>
</Linear></Creative></InLine></Ad></VAST>"#;
        assert!(!icon_x_position_misses(pos, &["right".to_string()]));
        assert!(!icon_y_position_misses(pos, &["top".to_string()]));
        assert!(icon_x_position_misses(pos, &["center".to_string()]));
        assert!(ad_serving_id_too_short("<VAST></VAST>", 1));
        assert!(ad_serving_id_too_short(
            "<VAST><AdServingId>ab</AdServingId></VAST>",
            3
        ));
        assert!(!ad_serving_id_too_short(
            "<VAST><AdServingId></AdServingId><AdServingId>srv-001</AdServingId></VAST>",
            3
        ));
        assert_eq!(
            ad_serving_id_text(
                "<VAST><AdServingId></AdServingId><AdServingId>srv-2</AdServingId></VAST>"
            )
            .as_deref(),
            Some("srv-2")
        );
        assert!(!vast_version_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["4.2".to_string()]
        ));
        assert!(vast_version_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["3.0".to_string()]
        ));
        let later_vast = r#"<VAST><Ad></Ad></VAST><VAST version = "4.2"><Ad></Ad></VAST>"#;
        assert!(!vast_version_misses(later_vast, &["4.2".to_string()]));
        assert!(creative_sequence_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["1".to_string()]
        ));
        let later_seq = r#"<VAST><Ad><InLine><Creatives>
<Creative></Creative>
<Creative sequence = "2"></Creative>
</Creatives></InLine></Ad></VAST>"#;
        assert!(!creative_sequence_misses(later_seq, &["2".to_string()]));
        assert!(creative_sequence_misses(
            later_seq,
            &["1".to_string(), "2".to_string()]
        ));
        let both_seq = r#"<VAST><Ad><InLine><Creatives>
<Creative sequence="1"></Creative>
<Creative sequence="2"></Creative>
</Creatives></InLine></Ad></VAST>"#;
        assert!(!creative_sequence_misses(
            both_seq,
            &["1".to_string(), "2".to_string()]
        ));
        assert!(!expires_exceeds(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            7200
        ));
        assert!(expires_exceeds(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            1800
        ));
        assert!(expires_exceeds("<VAST></VAST>", 7200));
        let later_exp = r#"<VAST><Ad><InLine>
<Expires></Expires>
<Expires>86400</Expires>
</InLine></Ad></VAST>"#;
        assert!(expires_exceeds(later_exp, 7200));
        assert!(expires_exceeds(
            "<VAST><Expires>not-a-clock</Expires></VAST>",
            7200
        ));
        assert!(!expires_below(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            1800
        ));
        assert!(expires_below(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            7200
        ));
        assert!(ad_sequence_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["1".to_string()]
        ));
        let later_ad = r#"<VAST><Ad id="1"></Ad><Ad id="2" sequence = "2"></Ad></VAST>"#;
        assert!(!ad_sequence_misses(later_ad, &["2".to_string()]));
        assert!(ad_sequence_misses(
            later_ad,
            &["1".to_string(), "2".to_string()]
        ));
        let both_ad = r#"<VAST><Ad sequence="1"></Ad><Ad sequence="2"></Ad></VAST>"#;
        assert!(!ad_sequence_misses(
            both_ad,
            &["1".to_string(), "2".to_string()]
        ));
        assert!(!wrapper_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(!wrapper_https_misses(include_str!(
            "../../../fixtures/wrapper.xml"
        )));
        assert!(wrapper_https_misses(
            r#"<VAST><Ad><Wrapper><VASTAdTagURI></VASTAdTagURI></Wrapper></Ad></VAST>"#
        ));
        assert!(wrapper_https_misses(
            r#"<VAST><Ad><Wrapper><VASTAdTagURI><![CDATA[http://ad.example.com/vast.xml]]></VASTAdTagURI></Wrapper></Ad></VAST>"#
        ));
        let later_uri = r#"<VAST><Ad><Wrapper>
<VASTAdTagURI><![CDATA[http://ad.example.com/a.xml]]></VASTAdTagURI>
<VASTAdTagURI><![CDATA[https://ad.example.com/b.xml]]></VASTAdTagURI>
</Wrapper></Ad></VAST>"#;
        assert!(wrapper_https_misses(later_uri));
        assert!(!wrapper_https_misses(
            r#"<VAST><Ad><Wrapper><VASTAdTagURI><![CDATA[HTTPS://ad.example.com/vast.xml]]></VASTAdTagURI></Wrapper></Ad></VAST>"#
        ));
        assert!(verification_vendor_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["iabtechlab.com-omid".to_string()]
        ));
        let later_vendor = r#"<VAST><Ad><InLine><AdVerifications>
<Verification></Verification>
<Verification vendor = "iabtechlab.com-omid"></Verification>
</AdVerifications></InLine></Ad></VAST>"#;
        assert!(!verification_vendor_misses(
            later_vendor,
            &["iabtechlab.com-omid".to_string()]
        ));
        assert!(pricing_model_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["CPM".to_string()]
        ));
        let later_price = r#"<VAST><Ad><InLine>
<Pricing></Pricing>
<Pricing model = "CPM" currency="USD">2.50</Pricing>
</InLine></Ad></VAST>"#;
        assert!(!pricing_model_misses(later_price, &["CPM".to_string()]));
        assert!(pricing_model_misses(later_price, &["CPC".to_string()]));
        assert!(!pricing_currency_misses(later_price, &["USD".to_string()]));
        assert!(pricing_currency_misses(later_price, &["EUR".to_string()]));
        assert!(error_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(error_https_misses(r#"<VAST><Error></Error></VAST>"#));
        assert!(error_https_misses(
            r#"<VAST><Error><![CDATA[http://t.example.com/e]]></Error></VAST>"#
        ));
        let later_err = r#"<VAST>
<Error></Error>
<Error><![CDATA[https://t.example.com/e]]></Error>
</VAST>"#;
        assert!(error_https_misses(later_err));
        assert!(!error_https_misses(
            r#"<VAST><Error><![CDATA[HTTPS://t.example.com/e]]></Error></VAST>"#
        ));
        assert!(viewable_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(viewable_https_misses(
            r#"<VAST><ViewableImpression></ViewableImpression></VAST>"#
        ));
        assert!(viewable_https_misses(
            r#"<VAST><ViewableImpression><Viewable><![CDATA[http://t.example.com/v]]></Viewable></ViewableImpression></VAST>"#
        ));
        let mixed_view = r#"<VAST><ViewableImpression>
<Viewable><![CDATA[https://t.example.com/v]]></Viewable>
<NotViewable><![CDATA[http://t.example.com/nv]]></NotViewable>
</ViewableImpression></VAST>"#;
        assert!(viewable_https_misses(mixed_view));
        let later_view = r#"<VAST><ViewableImpression>
<Viewable></Viewable>
<Viewable><![CDATA[https://t.example.com/v]]></Viewable>
</ViewableImpression></VAST>"#;
        assert!(viewable_https_misses(later_view));
        assert!(!viewable_https_misses(
            r#"<VAST><ViewableImpression><Viewable><![CDATA[HTTPS://t.example.com/v]]></Viewable></ViewableImpression></VAST>"#
        ));
        assert!(!impression_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(impression_https_misses(r#"<VAST></VAST>"#));
        assert!(impression_https_misses(
            r#"<VAST><Impression></Impression></VAST>"#
        ));
        assert!(impression_https_misses(
            r#"<VAST><Impression><![CDATA[http://t.example.com/i]]></Impression></VAST>"#
        ));
        let later_imp = r#"<VAST>
<Impression></Impression>
<Impression><![CDATA[https://t.example.com/i]]></Impression>
</VAST>"#;
        assert!(impression_https_misses(later_imp));
        assert!(clickthrough_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(clickthrough_https_misses(
            r#"<VAST><VideoClicks><ClickThrough><![CDATA[http://adv.example.com/l]]></ClickThrough></VideoClicks></VAST>"#
        ));
        let later_clk = r#"<VAST><VideoClicks>
<ClickThrough></ClickThrough>
<ClickThrough><![CDATA[https://adv.example.com/l]]></ClickThrough>
</VideoClicks></VAST>"#;
        assert!(clickthrough_https_misses(later_clk));
        assert!(!clickthrough_https_misses(
            r#"<VAST><VideoClicks><ClickThrough><![CDATA[HTTPS://adv.example.com/l]]></ClickThrough></VideoClicks></VAST>"#
        ));
        assert!(!survey_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(survey_https_misses(r#"<VAST><Survey></Survey></VAST>"#));
        assert!(survey_https_misses(
            r#"<VAST><Survey><![CDATA[http://t.example.com/s]]></Survey></VAST>"#
        ));
        assert!(!survey_https_misses(
            r#"<VAST><Survey><![CDATA[HTTPS://t.example.com/s]]></Survey></VAST>"#
        ));
        assert!(!clicktracking_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(clicktracking_https_misses(
            r#"<VAST><VideoClicks><ClickTracking><![CDATA[http://t.example.com/c]]></ClickTracking></VideoClicks></VAST>"#
        ));
        assert!(clicktracking_https_misses(
            r#"<VAST><IconClickTracking></IconClickTracking></VAST>"#
        ));
        let later_ct = r#"<VAST><VideoClicks>
<ClickTracking></ClickTracking>
<ClickTracking><![CDATA[https://t.example.com/c]]></ClickTracking>
</VideoClicks></VAST>"#;
        assert!(clicktracking_https_misses(later_ct));
        assert!(!clicktracking_https_misses(
            r#"<VAST><VideoClicks><ClickTracking><![CDATA[HTTPS://t.example.com/c]]></ClickTracking></VideoClicks></VAST>"#
        ));
        assert!(clicktracking_https_misses(
            r#"<VAST><CompanionClickTracking><![CDATA[http://t.example.com/cc]]></CompanionClickTracking></VAST>"#
        ));
        assert!(clicktracking_https_misses(
            r#"<VAST><NonLinearClickTracking><![CDATA[http://t.example.com/nl]]></NonLinearClickTracking></VAST>"#
        ));
        assert!(clicktracking_https_misses(
            r#"<VAST><CustomClick></CustomClick></VAST>"#
        ));
        assert!(clicktracking_https_misses(
            r#"<VAST><CustomClick><![CDATA[http://t.example.com/custom]]></CustomClick></VAST>"#
        ));
        assert!(!clicktracking_https_misses(
            r#"<VAST><CustomClick><![CDATA[HTTPS://t.example.com/custom]]></CustomClick></VAST>"#
        ));
        assert!(!icon_clickthrough_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(icon_clickthrough_https_misses(
            r#"<VAST><IconClickThrough></IconClickThrough></VAST>"#
        ));
        assert!(icon_clickthrough_https_misses(
            r#"<VAST><IconClickThrough><![CDATA[http://adv.example.com/icon]]></IconClickThrough></VAST>"#
        ));
        assert!(!icon_clickthrough_https_misses(
            r#"<VAST><IconClickThrough><![CDATA[HTTPS://adv.example.com/icon]]></IconClickThrough></VAST>"#
        ));
        assert!(category_authority_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["IAB".to_string()]
        ));
        let later_cat = r#"<VAST><Ad><InLine>
<Category>IAB1-1</Category>
<Category authority = "IAB">IAB2-1</Category>
</InLine></Ad></VAST>"#;
        assert!(!category_authority_misses(later_cat, &["IAB".to_string()]));
        assert!(category_authority_misses(
            later_cat,
            &["IAB-Tech-Lab".to_string()]
        ));
        assert!(!javascript_resource_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(javascript_resource_https_misses(
            r#"<VAST><JavaScriptResource></JavaScriptResource></VAST>"#
        ));
        assert!(javascript_resource_https_misses(
            r#"<VAST><JavaScriptResource apiFramework="omid"><![CDATA[http://t.example.com/omid.js]]></JavaScriptResource></VAST>"#
        ));
        let later_js = r#"<VAST><AdVerifications>
<JavaScriptResource></JavaScriptResource>
<JavaScriptResource><![CDATA[https://t.example.com/omid.js]]></JavaScriptResource>
</AdVerifications></VAST>"#;
        assert!(javascript_resource_https_misses(later_js));
        assert!(!javascript_resource_https_misses(
            r#"<VAST><JavaScriptResource><![CDATA[HTTPS://t.example.com/omid.js]]></JavaScriptResource></VAST>"#
        ));
        assert!(!companion_clickthrough_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(companion_clickthrough_https_misses(
            r#"<VAST><CompanionClickThrough></CompanionClickThrough></VAST>"#
        ));
        assert!(companion_clickthrough_https_misses(
            r#"<VAST><CompanionClickThrough><![CDATA[http://adv.example.com/c]]></CompanionClickThrough></VAST>"#
        ));
        assert!(!companion_clickthrough_https_misses(
            r#"<VAST><CompanionClickThrough><![CDATA[HTTPS://adv.example.com/c]]></CompanionClickThrough></VAST>"#
        ));
        assert!(advertiser_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["Acme".to_string()]
        ));
        let later_adv = r#"<VAST><Ad><InLine>
<Advertiser></Advertiser>
<Advertiser>Acme</Advertiser>
</InLine></Ad></VAST>"#;
        assert!(!advertiser_misses(later_adv, &["Acme".to_string()]));
        assert!(advertiser_misses(later_adv, &["Beta".to_string()]));
        assert!(advertiser_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["adv-1".to_string()]
        ));
        let later_adv_id = r#"<VAST>
<Advertiser></Advertiser>
<Advertiser id = "adv-1">Acme</Advertiser>
</VAST>"#;
        assert!(!advertiser_id_misses(later_adv_id, &["adv-1".to_string()]));
        assert!(advertiser_id_misses(later_adv_id, &["adv-2".to_string()]));
        assert!(!executable_resource_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(executable_resource_https_misses(
            r#"<VAST><ExecutableResource></ExecutableResource></VAST>"#
        ));
        assert!(executable_resource_https_misses(
            r#"<VAST><ExecutableResource apiFramework="omid"><![CDATA[http://t.example.com/omid.exe]]></ExecutableResource></VAST>"#
        ));
        let later_ex = r#"<VAST><AdVerifications>
<ExecutableResource></ExecutableResource>
<ExecutableResource><![CDATA[https://t.example.com/omid.exe]]></ExecutableResource>
</AdVerifications></VAST>"#;
        assert!(executable_resource_https_misses(later_ex));
        assert!(!executable_resource_https_misses(
            r#"<VAST><ExecutableResource><![CDATA[HTTPS://t.example.com/omid.exe]]></ExecutableResource></VAST>"#
        ));
        assert!(!nonlinear_clickthrough_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(nonlinear_clickthrough_https_misses(
            r#"<VAST><NonLinearClickThrough></NonLinearClickThrough></VAST>"#
        ));
        assert!(nonlinear_clickthrough_https_misses(
            r#"<VAST><NonLinearClickThrough><![CDATA[http://adv.example.com/nl]]></NonLinearClickThrough></VAST>"#
        ));
        assert!(!nonlinear_clickthrough_https_misses(
            r#"<VAST><NonLinearClickThrough><![CDATA[HTTPS://adv.example.com/nl]]></NonLinearClickThrough></VAST>"#
        ));
        assert!(description_too_short(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            1
        ));
        assert!(description_too_short(
            "<VAST><Description></Description><Description>ab</Description></VAST>",
            3
        ));
        assert!(!description_too_short(
            "<VAST><Description></Description><Description>abcd</Description></VAST>",
            3
        ));
        assert_eq!(
            description_text(
                "<VAST><Description></Description><Description> later </Description></VAST>"
            )
            .as_deref(),
            Some("later")
        );
        assert!(!static_resource_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(static_resource_https_misses(
            r#"<VAST><StaticResource></StaticResource></VAST>"#
        ));
        assert!(static_resource_https_misses(
            r#"<VAST><StaticResource creativeType="image/png"><![CDATA[http://cdn.example.com/i.png]]></StaticResource></VAST>"#
        ));
        let later_static = r#"<VAST>
<StaticResource></StaticResource>
<StaticResource><![CDATA[https://cdn.example.com/i.png]]></StaticResource>
</VAST>"#;
        assert!(static_resource_https_misses(later_static));
        assert!(!static_resource_https_misses(
            r#"<VAST><StaticResource><![CDATA[HTTPS://cdn.example.com/i.png]]></StaticResource></VAST>"#
        ));
        assert!(!iframe_resource_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(iframe_resource_https_misses(
            r#"<VAST><IFrameResource></IFrameResource></VAST>"#
        ));
        assert!(iframe_resource_https_misses(
            r#"<VAST><IFrameResource><![CDATA[http://cdn.example.com/frame.html]]></IFrameResource></VAST>"#
        ));
        assert!(!iframe_resource_https_misses(
            r#"<VAST><IFrameResource><![CDATA[HTTPS://cdn.example.com/frame.html]]></IFrameResource></VAST>"#
        ));
        assert!(ad_title_too_short(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            8
        ));
        assert!(!ad_title_too_short(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            3
        ));
        assert!(ad_title_too_short(
            "<VAST><AdTitle></AdTitle><AdTitle>ab</AdTitle></VAST>",
            3
        ));
        assert!(!ad_title_too_short(
            "<VAST><AdTitle></AdTitle><AdTitle>abcd</AdTitle></VAST>",
            3
        ));
        assert_eq!(
            ad_title_text("<VAST><AdTitle></AdTitle><AdTitle> later </AdTitle></VAST>").as_deref(),
            Some("later")
        );
        assert!(!html_resource_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(html_resource_https_misses(
            r#"<VAST><HTMLResource></HTMLResource></VAST>"#
        ));
        assert!(html_resource_https_misses(
            r#"<VAST><HTMLResource><![CDATA[http://cdn.example.com/ad.html]]></HTMLResource></VAST>"#
        ));
        let later_html = r#"<VAST>
<HTMLResource></HTMLResource>
<HTMLResource><![CDATA[https://cdn.example.com/ad.html]]></HTMLResource>
</VAST>"#;
        assert!(html_resource_https_misses(later_html));
        assert!(!html_resource_https_misses(
            r#"<VAST><HTMLResource><![CDATA[HTTPS://cdn.example.com/ad.html]]></HTMLResource></VAST>"#
        ));
        assert!(!flash_resource_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(flash_resource_https_misses(
            r#"<VAST><FlashResource></FlashResource></VAST>"#
        ));
        assert!(flash_resource_https_misses(
            r#"<VAST><FlashResource><![CDATA[http://cdn.example.com/ad.swf]]></FlashResource></VAST>"#
        ));
        assert!(!flash_resource_https_misses(
            r#"<VAST><FlashResource><![CDATA[HTTPS://cdn.example.com/ad.swf]]></FlashResource></VAST>"#
        ));
        assert!(ad_parameters_too_short(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            1
        ));
        assert!(ad_parameters_too_short(
            "<VAST><AdParameters></AdParameters><AdParameters>ab</AdParameters></VAST>",
            3
        ));
        assert!(!ad_parameters_too_short(
            "<VAST><AdParameters></AdParameters><AdParameters>{\"x\":1}</AdParameters></VAST>",
            3
        ));
        assert_eq!(
            ad_parameters_text(
                "<VAST><AdParameters></AdParameters><AdParameters> later </AdParameters></VAST>"
            )
            .as_deref(),
            Some("later")
        );
        assert!(!closed_caption_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(closed_caption_https_misses(
            r#"<VAST><ClosedCaptionFile></ClosedCaptionFile></VAST>"#
        ));
        assert!(closed_caption_https_misses(
            r#"<VAST><ClosedCaptionFile language="en"><![CDATA[http://cdn.example.com/ad.vtt]]></ClosedCaptionFile></VAST>"#
        ));
        let later_cc = r#"<VAST>
<ClosedCaptionFile></ClosedCaptionFile>
<ClosedCaptionFile><![CDATA[https://cdn.example.com/ad.vtt]]></ClosedCaptionFile>
</VAST>"#;
        assert!(closed_caption_https_misses(later_cc));
        assert!(!closed_caption_https_misses(
            r#"<VAST><ClosedCaptionFile><![CDATA[HTTPS://cdn.example.com/ad.vtt]]></ClosedCaptionFile></VAST>"#
        ));
        assert!(!icon_viewtracking_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(icon_viewtracking_https_misses(
            r#"<VAST><IconViewTracking></IconViewTracking></VAST>"#
        ));
        assert!(icon_viewtracking_https_misses(
            r#"<VAST><IconViewTracking><![CDATA[http://t.example.com/iv]]></IconViewTracking></VAST>"#
        ));
        assert!(!icon_viewtracking_https_misses(
            r#"<VAST><IconViewTracking><![CDATA[HTTPS://t.example.com/iv]]></IconViewTracking></VAST>"#
        ));
        assert!(!ad_system_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["Test".to_string()]
        ));
        assert!(ad_system_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["Other".to_string()]
        ));
        let later_sys = r#"<VAST><Ad><InLine>
<AdSystem></AdSystem>
<AdSystem>Acme DSP</AdSystem>
</InLine></Ad></VAST>"#;
        assert!(!ad_system_misses(later_sys, &["Acme DSP".to_string()]));
        assert!(ad_system_misses("<VAST></VAST>", &["Test".to_string()]));
        assert!(!icon_clickfallback_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(icon_clickfallback_https_misses(
            r#"<VAST><IconClickFallbackImage></IconClickFallbackImage></VAST>"#
        ));
        assert!(icon_clickfallback_https_misses(
            r#"<VAST><IconClickFallbackImage><![CDATA[http://cdn.example.com/icon.png]]></IconClickFallbackImage></VAST>"#
        ));
        let later_fb = r#"<VAST>
<IconClickFallbackImage></IconClickFallbackImage>
<IconClickFallbackImage><![CDATA[https://cdn.example.com/icon.png]]></IconClickFallbackImage>
</VAST>"#;
        assert!(icon_clickfallback_https_misses(later_fb));
        assert!(!icon_clickfallback_https_misses(
            r#"<VAST><IconClickFallbackImage><![CDATA[HTTPS://cdn.example.com/icon.png]]></IconClickFallbackImage></VAST>"#
        ));
        assert!(!mezzanine_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(mezzanine_https_misses(
            r#"<VAST><Mezzanine></Mezzanine></VAST>"#
        ));
        assert!(mezzanine_https_misses(
            r#"<VAST><Mezzanine><![CDATA[http://cdn.example.com/mezz.mp4]]></Mezzanine></VAST>"#
        ));
        let later_mezz = r#"<VAST>
<Mezzanine></Mezzanine>
<Mezzanine><![CDATA[https://cdn.example.com/mezz.mp4]]></Mezzanine>
</VAST>"#;
        assert!(mezzanine_https_misses(later_mezz));
        assert!(!mezzanine_https_misses(
            r#"<VAST><Mezzanine><![CDATA[HTTPS://cdn.example.com/mezz.mp4]]></Mezzanine></VAST>"#
        ));
        assert!(category_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["IAB1-1".to_string()]
        ));
        let later_cat = r#"<VAST><Ad><InLine>
<Category></Category>
<Category>IAB1-1</Category>
</InLine></Ad></VAST>"#;
        assert!(!category_misses(later_cat, &["IAB1-1".to_string()]));
        assert!(category_misses(later_cat, &["IAB2-1".to_string()]));
        assert!(category_misses("<VAST></VAST>", &["IAB1-1".to_string()]));
        assert!(!tracking_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(tracking_https_misses(
            r#"<VAST><Tracking event="start"></Tracking></VAST>"#
        ));
        assert!(tracking_https_misses(
            r#"<VAST><Tracking event="start"><![CDATA[http://t.example.com/start]]></Tracking></VAST>"#
        ));
        let later_tr = r#"<VAST>
<Tracking event="start"></Tracking>
<Tracking event="complete"><![CDATA[https://t.example.com/complete]]></Tracking>
</VAST>"#;
        assert!(tracking_https_misses(later_tr));
        assert!(!tracking_https_misses(
            r#"<VAST><Tracking event="start"><![CDATA[HTTPS://t.example.com/start]]></Tracking></VAST>"#
        ));
        assert!(pricing_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["2.50".to_string()]
        ));
        let later_price = r#"<VAST><Ad><InLine>
<Pricing></Pricing>
<Pricing model="CPM">2.50</Pricing>
</InLine></Ad></VAST>"#;
        assert!(!pricing_misses(later_price, &["2.50".to_string()]));
        assert!(pricing_misses(later_price, &["1.00".to_string()]));
        assert!(pricing_misses("<VAST></VAST>", &["2.50".to_string()]));
        assert!(!extension_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(extension_https_misses(
            r#"<VAST><Extension type="geo"></Extension></VAST>"#
        ));
        assert!(extension_https_misses(
            r#"<VAST><Extension type="geo"><![CDATA[http://t.example.com/ext]]></Extension></VAST>"#
        ));
        let later_ext = r#"<VAST>
<Extension></Extension>
<Extension><![CDATA[https://t.example.com/ext]]></Extension>
</VAST>"#;
        assert!(extension_https_misses(later_ext));
        assert!(!extension_https_misses(
            r#"<VAST><Extension><![CDATA[HTTPS://t.example.com/ext]]></Extension></VAST>"#
        ));
        assert!(extension_https_misses(
            r#"<VAST><Extension type="geo"><Custom><City>NYC</City></Custom></Extension></VAST>"#
        ));
        assert!(!interactive_creative_file_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(interactive_creative_file_https_misses(
            r#"<VAST><InteractiveCreativeFile></InteractiveCreativeFile></VAST>"#
        ));
        assert!(interactive_creative_file_https_misses(
            r#"<VAST><InteractiveCreativeFile type="application/javascript" apiFramework="omid"><![CDATA[http://cdn.example.com/v.js]]></InteractiveCreativeFile></VAST>"#
        ));
        let later_icf = r#"<VAST>
<InteractiveCreativeFile></InteractiveCreativeFile>
<InteractiveCreativeFile><![CDATA[https://cdn.example.com/v.js]]></InteractiveCreativeFile>
</VAST>"#;
        assert!(interactive_creative_file_https_misses(later_icf));
        assert!(!interactive_creative_file_https_misses(
            r#"<VAST><InteractiveCreativeFile><![CDATA[HTTPS://cdn.example.com/v.js]]></InteractiveCreativeFile></VAST>"#
        ));
        assert!(!universal_ad_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["ABCD1234567".to_string()]
        ));
        assert!(universal_ad_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["OTHER".to_string()]
        ));
        let later_uad = r#"<VAST><Ad><InLine>
<UniversalAdId></UniversalAdId>
<UniversalAdId idValue="later-id"></UniversalAdId>
</InLine></Ad></VAST>"#;
        assert!(!universal_ad_id_misses(
            later_uad,
            &["later-id".to_string()]
        ));
        assert!(universal_ad_id_misses(
            "<VAST></VAST>",
            &["ABCD1234567".to_string()]
        ));
        assert!(!api_framework_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["omid".to_string()]
        ));
        let omid = r#"<VAST><MediaFile apiFramework="omid" type="video/mp4">https://cdn.example.com/ad.mp4</MediaFile></VAST>"#;
        assert!(!api_framework_misses(omid, &["omid".to_string()]));
        assert!(api_framework_misses(omid, &["VPAID".to_string()]));
        let mixed = r#"<VAST>
<MediaFile apiFramework="omid"></MediaFile>
<InteractiveCreativeFile apiFramework="VPAID"></InteractiveCreativeFile>
</VAST>"#;
        assert!(api_framework_misses(mixed, &["omid".to_string()]));
        let later_js = r#"<VAST>
<JavaScriptResource apiFramework="omid"></JavaScriptResource>
</VAST>"#;
        assert!(!api_framework_misses(later_js, &["OMID".to_string()]));
        let icon_api = r#"<VAST><Icon apiFramework="omid"></Icon></VAST>"#;
        assert!(!api_framework_misses(icon_api, &["omid".to_string()]));
        assert!(api_framework_misses(icon_api, &["VPAID".to_string()]));
        let comp_api = r#"<VAST><Companion apiFramework="VPAID"></Companion></VAST>"#;
        assert!(api_framework_misses(comp_api, &["omid".to_string()]));
        assert!(!api_framework_misses(comp_api, &["VPAID".to_string()]));
        let creative_api = r#"<VAST><Creative apiFramework="omid"></Creative></VAST>"#;
        assert!(!api_framework_misses(creative_api, &["omid".to_string()]));
        assert!(api_framework_misses(creative_api, &["VPAID".to_string()]));
        let mezz_api = r#"<VAST><Mezzanine apiFramework="omid"></Mezzanine></VAST>"#;
        assert!(!api_framework_misses(mezz_api, &["omid".to_string()]));
        assert!(api_framework_misses(mezz_api, &["VPAID".to_string()]));
        let ver_api = r#"<VAST><Verification apiFramework="omid"></Verification></VAST>"#;
        assert!(!api_framework_misses(ver_api, &["omid".to_string()]));
        assert!(api_framework_misses(ver_api, &["VPAID".to_string()]));
        assert!(creative_adid_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["ad-1".to_string()]
        ));
        let later_adid = r#"<VAST><Ad><InLine><Creatives>
<Creative></Creative>
<Creative AdID = "ad-1"></Creative>
</Creatives></InLine></Ad></VAST>"#;
        assert!(!creative_adid_misses(later_adid, &["ad-1".to_string()]));
        assert!(creative_adid_misses(later_adid, &["ad-2".to_string()]));
        assert!(interactive_creative_file_type_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["application/javascript".to_string()]
        ));
        let later_icf_type = r#"<VAST>
<InteractiveCreativeFile></InteractiveCreativeFile>
<InteractiveCreativeFile type = "application/javascript"></InteractiveCreativeFile>
</VAST>"#;
        assert!(!interactive_creative_file_type_misses(
            later_icf_type,
            &["application/javascript".to_string()]
        ));
        assert!(interactive_creative_file_type_misses(
            later_icf_type,
            &["application/x-shockwave-flash".to_string()]
        ));
        assert!(viewable_impression_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["view-1".to_string()]
        ));
        let later_view = r#"<VAST>
<ViewableImpression></ViewableImpression>
<ViewableImpression id = "view-1"></ViewableImpression>
</VAST>"#;
        assert!(!viewable_impression_id_misses(
            later_view,
            &["view-1".to_string()]
        ));
        assert!(viewable_impression_id_misses(
            later_view,
            &["view-2".to_string()]
        ));
        assert!(companion_type_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["image/png".to_string()]
        ));
        let later_comp = r#"<VAST>
<Companion></Companion>
<Companion type = "image/png"></Companion>
</VAST>"#;
        assert!(!companion_type_misses(
            later_comp,
            &["image/png".to_string()]
        ));
        assert!(companion_type_misses(
            later_comp,
            &["image/jpeg".to_string()]
        ));
        let later_static = r#"<VAST>
<StaticResource></StaticResource>
<StaticResource creativeType = "image/png"></StaticResource>
</VAST>"#;
        assert!(!companion_type_misses(
            later_static,
            &["image/png".to_string()]
        ));
        assert!(icon_type_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["image/png".to_string()]
        ));
        let later_icon_type = r#"<VAST>
<Icon></Icon>
<Icon type = "image/png"></Icon>
</VAST>"#;
        assert!(!icon_type_misses(
            later_icon_type,
            &["image/png".to_string()]
        ));
        assert!(icon_type_misses(
            later_icon_type,
            &["image/jpeg".to_string()]
        ));
        assert!(!ad_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["1".to_string()]
        ));
        assert!(ad_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["2".to_string()]
        ));
        let later_ad = r#"<VAST>
<Ad></Ad>
<Ad id = "later"></Ad>
</VAST>"#;
        assert!(!ad_id_misses(later_ad, &["later".to_string()]));
        assert!(ad_id_misses("<VAST></VAST>", &["1".to_string()]));
        assert!(mediafile_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["mf-1".to_string()]
        ));
        let later_mf = r#"<VAST>
<MediaFile></MediaFile>
<MediaFile id = "mf-1"></MediaFile>
</VAST>"#;
        assert!(!mediafile_id_misses(later_mf, &["mf-1".to_string()]));
        assert!(mediafile_id_misses(later_mf, &["mf-2".to_string()]));
        assert!(companion_adslot_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["slot-1".to_string()]
        ));
        let later_slot = r#"<VAST>
<Companion></Companion>
<Companion adSlotId = "slot-1"></Companion>
</VAST>"#;
        assert!(!companion_adslot_id_misses(
            later_slot,
            &["slot-1".to_string()]
        ));
        assert!(companion_adslot_id_misses(
            later_slot,
            &["slot-2".to_string()]
        ));
        assert!(icon_alt_text_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["AdChoices".to_string()]
        ));
        let later_alt = r#"<VAST>
<Icon></Icon>
<Icon altText = "AdChoices"></Icon>
</VAST>"#;
        assert!(!icon_alt_text_misses(later_alt, &["AdChoices".to_string()]));
        assert!(icon_alt_text_misses(later_alt, &["Other".to_string()]));
        assert!(companion_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["comp-1".to_string()]
        ));
        let later_cid = r#"<VAST>
<Companion></Companion>
<Companion id = "comp-1"></Companion>
</VAST>"#;
        assert!(!companion_id_misses(later_cid, &["comp-1".to_string()]));
        assert!(companion_id_misses(later_cid, &["comp-2".to_string()]));
        assert!(icon_hover_text_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["Learn more".to_string()]
        ));
        let later_hover = r#"<VAST>
<Icon></Icon>
<Icon hoverText = "Learn more"></Icon>
</VAST>"#;
        assert!(!icon_hover_text_misses(
            later_hover,
            &["Learn more".to_string()]
        ));
        assert!(icon_hover_text_misses(later_hover, &["Other".to_string()]));
        assert!(nonlinear_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["nl-1".to_string()]
        ));
        let later_nl = r#"<VAST>
<NonLinear></NonLinear>
<NonLinear id = "nl-1"></NonLinear>
</VAST>"#;
        assert!(!nonlinear_id_misses(later_nl, &["nl-1".to_string()]));
        assert!(nonlinear_id_misses(later_nl, &["nl-2".to_string()]));
        assert!(companion_rendering_mode_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["end-card".to_string()]
        ));
        let later_mode = r#"<VAST>
<Companion></Companion>
<Companion renderingMode = "end-card"></Companion>
</VAST>"#;
        assert!(!companion_rendering_mode_misses(
            later_mode,
            &["end-card".to_string()]
        ));
        assert!(companion_rendering_mode_misses(
            later_mode,
            &["default".to_string()]
        ));
        assert!(icon_pxratio_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["2".to_string()]
        ));
        let later_px = r#"<VAST>
<Icon></Icon>
<Icon pxratio = "2"></Icon>
</VAST>"#;
        assert!(!icon_pxratio_misses(later_px, &["2".to_string()]));
        assert!(icon_pxratio_misses(later_px, &["1".to_string()]));
        assert!(nonlinear_scalable_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["true".to_string()]
        ));
        let later_scal = r#"<VAST>
<NonLinear></NonLinear>
<NonLinear scalable = "true"></NonLinear>
</VAST>"#;
        assert!(!nonlinear_scalable_misses(
            later_scal,
            &["true".to_string()]
        ));
        assert!(nonlinear_scalable_misses(
            later_scal,
            &["false".to_string()]
        ));
        assert!(companion_expanded_size_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            Some(600),
            None
        ));
        let later_exp = r#"<VAST><Ad><InLine><Creative>
<CompanionAds>
<Companion width="300" height="250"></Companion>
<Companion expandedWidth = "600" expandedHeight="500"></Companion>
</CompanionAds>
</Creative></InLine></Ad></VAST>"#;
        assert!(!companion_expanded_size_misses(later_exp, Some(600), None));
        assert!(companion_expanded_size_misses(later_exp, Some(800), None));
        assert!(companion_expanded_size_misses(
            later_exp,
            Some(600),
            Some(900)
        ));
        assert!(!companion_expanded_size_misses(
            later_exp,
            Some(600),
            Some(500)
        ));
        let split_exp = r#"<VAST><Ad><InLine><Creative>
<CompanionAds>
<Companion expandedWidth="600" expandedHeight="1"></Companion>
<Companion expandedWidth="1" expandedHeight="500"></Companion>
</CompanionAds>
</Creative></InLine></Ad></VAST>"#;
        assert!(companion_expanded_size_misses(
            split_exp,
            Some(600),
            Some(500)
        ));
        assert!(nonlinear_maintain_aspect_ratio_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["true".to_string()]
        ));
        let later_mar = r#"<VAST>
<NonLinear></NonLinear>
<NonLinear maintainAspectRatio = "true"></NonLinear>
</VAST>"#;
        assert!(!nonlinear_maintain_aspect_ratio_misses(
            later_mar,
            &["true".to_string()]
        ));
        assert!(nonlinear_maintain_aspect_ratio_misses(
            later_mar,
            &["false".to_string()]
        ));
        assert!(companion_asset_size_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            Some(300),
            None
        ));
        let later_asset = r#"<VAST><Ad><InLine><Creative>
<CompanionAds>
<Companion width="300" height="250"></Companion>
<Companion assetWidth = "300" assetHeight="250"></Companion>
</CompanionAds>
</Creative></InLine></Ad></VAST>"#;
        assert!(!companion_asset_size_misses(later_asset, Some(300), None));
        assert!(companion_asset_size_misses(later_asset, Some(640), None));
        assert!(companion_asset_size_misses(
            later_asset,
            Some(300),
            Some(600)
        ));
        assert!(!companion_asset_size_misses(
            later_asset,
            Some(300),
            Some(250)
        ));
        let split_asset = r#"<VAST><Ad><InLine><Creative>
<CompanionAds>
<Companion assetWidth="300" assetHeight="1"></Companion>
<Companion assetWidth="1" assetHeight="250"></Companion>
</CompanionAds>
</Creative></InLine></Ad></VAST>"#;
        assert!(companion_asset_size_misses(
            split_asset,
            Some(300),
            Some(250)
        ));
        assert!(nonlinear_expanded_size_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            Some(600),
            None
        ));
        let later_nl_exp = r#"<VAST>
<NonLinear width="300" height="250"></NonLinear>
<NonLinear expandedWidth = "600" expandedHeight="500"></NonLinear>
</VAST>"#;
        assert!(!nonlinear_expanded_size_misses(
            later_nl_exp,
            Some(600),
            None
        ));
        assert!(nonlinear_expanded_size_misses(
            later_nl_exp,
            Some(800),
            None
        ));
        assert!(nonlinear_expanded_size_misses(
            later_nl_exp,
            Some(600),
            Some(900)
        ));
        assert!(!nonlinear_expanded_size_misses(
            later_nl_exp,
            Some(600),
            Some(500)
        ));
        let split_nl = r#"<VAST>
<NonLinear expandedWidth="600" expandedHeight="1"></NonLinear>
<NonLinear expandedWidth="1" expandedHeight="500"></NonLinear>
</VAST>"#;
        assert!(nonlinear_expanded_size_misses(
            split_nl,
            Some(600),
            Some(500)
        ));
        assert!(companion_xml_encoded_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["true".to_string()]
        ));
        let later_xml = r#"<VAST>
<Companion></Companion>
<Companion xmlEncoded = "true"></Companion>
</VAST>"#;
        assert!(!companion_xml_encoded_misses(
            later_xml,
            &["true".to_string()]
        ));
        assert!(companion_xml_encoded_misses(
            later_xml,
            &["false".to_string()]
        ));
        assert!(companion_pxratio_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["2".to_string()]
        ));
        let later_cpx = r#"<VAST>
<Companion></Companion>
<Companion pxratio = "2"></Companion>
</VAST>"#;
        assert!(!companion_pxratio_misses(later_cpx, &["2".to_string()]));
        assert!(companion_pxratio_misses(later_cpx, &["1".to_string()]));
        assert!(nonlinear_size_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            Some(300),
            None
        ));
        let later_nl = r#"<VAST>
<NonLinear width="1" height="1"></NonLinear>
<NonLinear width = "300" height="250"></NonLinear>
</VAST>"#;
        assert!(!nonlinear_size_misses(later_nl, Some(300), None));
        assert!(nonlinear_size_misses(later_nl, Some(640), None));
        assert!(nonlinear_size_misses(later_nl, Some(300), Some(600)));
        assert!(!nonlinear_size_misses(later_nl, Some(300), Some(250)));
        let split_nl_size = r#"<VAST>
<NonLinear width="300" height="1"></NonLinear>
<NonLinear width="1" height="250"></NonLinear>
</VAST>"#;
        assert!(nonlinear_size_misses(split_nl_size, Some(300), Some(250)));
        assert!(wrapper_follow_additional_wrappers_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["true".to_string()]
        ));
        let later_wrap = r#"<VAST>
<Wrapper></Wrapper>
<Wrapper followAdditionalWrappers = "true" allowMultipleAds = "true" fallbackOnNoAd = "false"></Wrapper>
</VAST>"#;
        assert!(!wrapper_follow_additional_wrappers_misses(
            later_wrap,
            &["true".to_string()]
        ));
        assert!(wrapper_follow_additional_wrappers_misses(
            later_wrap,
            &["false".to_string()]
        ));
        assert!(!wrapper_allow_multiple_ads_misses(
            later_wrap,
            &["true".to_string()]
        ));
        assert!(wrapper_allow_multiple_ads_misses(
            later_wrap,
            &["false".to_string()]
        ));
        assert!(!wrapper_fallback_on_no_ad_misses(
            later_wrap,
            &["false".to_string()]
        ));
        assert!(wrapper_fallback_on_no_ad_misses(
            later_wrap,
            &["true".to_string()]
        ));
        assert!(ad_conditional_ad_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["true".to_string()]
        ));
        let later_cond = r#"<VAST>
<Ad></Ad>
<Ad conditionalAd = "true"></Ad>
</VAST>"#;
        assert!(!ad_conditional_ad_misses(later_cond, &["true".to_string()]));
        assert!(ad_conditional_ad_misses(later_cond, &["false".to_string()]));
        assert!(mediafile_scalable_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["true".to_string()]
        ));
        let later_mf = r#"<VAST>
<MediaFile></MediaFile>
<MediaFile scalable = "true" maintainAspectRatio = "true"></MediaFile>
</VAST>"#;
        assert!(!mediafile_scalable_misses(later_mf, &["true".to_string()]));
        assert!(mediafile_scalable_misses(later_mf, &["false".to_string()]));
        assert!(!mediafile_maintain_aspect_ratio_misses(
            later_mf,
            &["true".to_string()]
        ));
        assert!(mediafile_maintain_aspect_ratio_misses(
            later_mf,
            &["false".to_string()]
        ));
        assert!(mediafile_maintain_aspect_ratio_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["true".to_string()]
        ));
        assert!(executable_resource_type_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["application/javascript".to_string()]
        ));
        let later_exec = r#"<VAST>
<ExecutableResource></ExecutableResource>
<ExecutableResource type = "application/javascript"></ExecutableResource>
</VAST>"#;
        assert!(!executable_resource_type_misses(
            later_exec,
            &["application/javascript".to_string()]
        ));
        assert!(executable_resource_type_misses(
            later_exec,
            &["application/x-javascript".to_string()]
        ));
        assert!(javascript_resource_browser_optional_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["true".to_string()]
        ));
        let later_js_opt = r#"<VAST>
<JavaScriptResource></JavaScriptResource>
<JavaScriptResource browserOptional = "true"></JavaScriptResource>
</VAST>"#;
        assert!(!javascript_resource_browser_optional_misses(
            later_js_opt,
            &["true".to_string()]
        ));
        assert!(javascript_resource_browser_optional_misses(
            later_js_opt,
            &["false".to_string()]
        ));
        assert!(extension_type_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["iab".to_string()]
        ));
        let later_ext_type = r#"<VAST>
<Extension></Extension>
<Extension type = "iab"></Extension>
</VAST>"#;
        assert!(!extension_type_misses(later_ext_type, &["iab".to_string()]));
        assert!(extension_type_misses(
            later_ext_type,
            &["other".to_string()]
        ));
        assert!(mezzanine_type_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["video/mp4".to_string()]
        ));
        let later_mezz_type = r#"<VAST>
<Mezzanine></Mezzanine>
<Mezzanine type = "video/mp4" delivery = "progressive"></Mezzanine>
</VAST>"#;
        assert!(!mezzanine_type_misses(
            later_mezz_type,
            &["video/mp4".to_string()]
        ));
        assert!(mezzanine_type_misses(
            later_mezz_type,
            &["video/webm".to_string()]
        ));
        assert!(!mezzanine_delivery_misses(
            later_mezz_type,
            &["progressive".to_string()]
        ));
        assert!(mezzanine_delivery_misses(
            later_mezz_type,
            &["streaming".to_string()]
        ));
        assert!(mezzanine_delivery_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["progressive".to_string()]
        ));
        assert!(ad_parameters_xml_encoded_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["true".to_string()]
        ));
        let later_params_enc = r#"<VAST>
<AdParameters></AdParameters>
<AdParameters xmlEncoded = "true"></AdParameters>
</VAST>"#;
        assert!(!ad_parameters_xml_encoded_misses(
            later_params_enc,
            &["true".to_string()]
        ));
        assert!(ad_parameters_xml_encoded_misses(
            later_params_enc,
            &["false".to_string()]
        ));
        assert!(mezzanine_codec_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["avc1.42E01E".to_string()]
        ));
        let later_mezz_codec = r#"<VAST>
<Mezzanine></Mezzanine>
<Mezzanine codec = "avc1.42E01E"></Mezzanine>
</VAST>"#;
        assert!(!mezzanine_codec_misses(
            later_mezz_codec,
            &["avc1.42E01E".to_string()]
        ));
        assert!(mezzanine_codec_misses(
            later_mezz_codec,
            &["vp9".to_string()]
        ));
        let later_mezz_codecs = r#"<VAST>
<Mezzanine codecs = "avc1.42E01E"></Mezzanine>
</VAST>"#;
        assert!(!mezzanine_codec_misses(
            later_mezz_codecs,
            &["avc1.42E01E".to_string()]
        ));
        assert!(mezzanine_file_size_below(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            1
        ));
        let later_mezz_fs = r#"<VAST>
<Mezzanine fileSize="1"></Mezzanine>
<Mezzanine fileSize = "1000000"></Mezzanine>
</VAST>"#;
        assert!(!mezzanine_file_size_below(later_mezz_fs, 1000000));
        assert!(mezzanine_file_size_below(later_mezz_fs, 2000000));
        assert!(mezzanine_size_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            Some(1920),
            None
        ));
        let later_mezz_size = r#"<VAST>
<Mezzanine width="1" height="1"></Mezzanine>
<Mezzanine width = "1920" height="1080"></Mezzanine>
</VAST>"#;
        assert!(!mezzanine_size_misses(later_mezz_size, Some(1920), None));
        assert!(mezzanine_size_misses(later_mezz_size, Some(3840), None));
        assert!(mezzanine_size_misses(
            later_mezz_size,
            Some(1920),
            Some(2160)
        ));
        assert!(!mezzanine_size_misses(
            later_mezz_size,
            Some(1920),
            Some(1080)
        ));
        let split_mezz = r#"<VAST>
<Mezzanine width="1920" height="1"></Mezzanine>
<Mezzanine width="1" height="1080"></Mezzanine>
</VAST>"#;
        assert!(mezzanine_size_misses(split_mezz, Some(1920), Some(1080)));
        assert!(mezzanine_bitrate_misses_range(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            Some(1000),
            None
        ));
        let later_mezz_br = r#"<VAST>
<Mezzanine bitrate="1"></Mezzanine>
<Mezzanine bitrate = "8000"></Mezzanine>
</VAST>"#;
        assert!(!mezzanine_bitrate_misses_range(
            later_mezz_br,
            Some(1000),
            None
        ));
        assert!(mezzanine_bitrate_misses_range(
            later_mezz_br,
            Some(9000),
            None
        ));
        assert!(!mezzanine_bitrate_misses_range(
            later_mezz_br,
            Some(1000),
            Some(15000)
        ));
        assert!(mezzanine_bitrate_misses_range(
            later_mezz_br,
            Some(1000),
            Some(7000)
        ));
        let later_mezz_maxbr = r#"<VAST>
<Mezzanine maxBitrate = "8000"></Mezzanine>
</VAST>"#;
        assert!(!mezzanine_bitrate_misses_range(
            later_mezz_maxbr,
            Some(1000),
            Some(15000)
        ));
        let split_mezz_br = r#"<VAST>
<Mezzanine bitrate="500"></Mezzanine>
<Mezzanine bitrate="20000"></Mezzanine>
</VAST>"#;
        assert!(mezzanine_bitrate_misses_range(
            split_mezz_br,
            Some(1000),
            Some(15000)
        ));
        assert!(mezzanine_media_type_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["video".to_string()]
        ));
        let later_mezz_mt = r#"<VAST>
<Mezzanine></Mezzanine>
<Mezzanine mediaType = "video"></Mezzanine>
</VAST>"#;
        assert!(!mezzanine_media_type_misses(
            later_mezz_mt,
            &["video".to_string()]
        ));
        assert!(mezzanine_media_type_misses(
            later_mezz_mt,
            &["audio".to_string()]
        ));
        assert!(mezzanine_scalable_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["true".to_string()]
        ));
        let later_mezz_scal = r#"<VAST>
<Mezzanine></Mezzanine>
<Mezzanine scalable = "true" maintainAspectRatio = "true" id = "mezz-1"></Mezzanine>
</VAST>"#;
        assert!(!mezzanine_scalable_misses(
            later_mezz_scal,
            &["true".to_string()]
        ));
        assert!(mezzanine_scalable_misses(
            later_mezz_scal,
            &["false".to_string()]
        ));
        assert!(!mezzanine_maintain_aspect_ratio_misses(
            later_mezz_scal,
            &["true".to_string()]
        ));
        assert!(mezzanine_maintain_aspect_ratio_misses(
            later_mezz_scal,
            &["false".to_string()]
        ));
        assert!(mezzanine_maintain_aspect_ratio_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["true".to_string()]
        ));
        assert!(!mezzanine_id_misses(
            later_mezz_scal,
            &["mezz-1".to_string()]
        ));
        assert!(mezzanine_id_misses(
            later_mezz_scal,
            &["mezz-2".to_string()]
        ));
        assert!(mezzanine_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["mezz-1".to_string()]
        ));
        assert!(icon_xml_encoded_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["true".to_string()]
        ));
        let later_icon_xmlenc = r#"<VAST>
<Icon></Icon>
<Icon xmlEncoded = "true"></Icon>
</VAST>"#;
        assert!(!icon_xml_encoded_misses(
            later_icon_xmlenc,
            &["true".to_string()]
        ));
        assert!(icon_xml_encoded_misses(
            later_icon_xmlenc,
            &["false".to_string()]
        ));
        assert!(nonlinear_xml_encoded_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["true".to_string()]
        ));
        let later_nl_xmlenc = r#"<VAST>
<NonLinear></NonLinear>
<NonLinear xmlEncoded = "true"></NonLinear>
</VAST>"#;
        assert!(!nonlinear_xml_encoded_misses(
            later_nl_xmlenc,
            &["true".to_string()]
        ));
        assert!(nonlinear_xml_encoded_misses(
            later_nl_xmlenc,
            &["false".to_string()]
        ));
        assert!(icon_size_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            Some(20),
            None
        ));
        let later_icon_sz = r#"<VAST>
<Icon width="1" height="1"></Icon>
<Icon width = "20" height="20"></Icon>
</VAST>"#;
        assert!(!icon_size_misses(later_icon_sz, Some(20), None));
        assert!(icon_size_misses(later_icon_sz, Some(40), None));
        assert!(icon_size_misses(later_icon_sz, Some(20), Some(40)));
        assert!(!icon_size_misses(later_icon_sz, Some(20), Some(20)));
        let split_icon_sz = r#"<VAST>
<Icon width="20" height="1"></Icon>
<Icon width="1" height="20"></Icon>
</VAST>"#;
        assert!(icon_size_misses(split_icon_sz, Some(20), Some(20)));
        assert!(caption_file_status_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["onDemand".to_string()]
        ));
        let later_cap_fs = r#"<VAST>
<ClosedCaptionFile></ClosedCaptionFile>
<ClosedCaptionFile fileStatus = "onDemand"></ClosedCaptionFile>
</VAST>"#;
        assert!(!caption_file_status_misses(
            later_cap_fs,
            &["onDemand".to_string()]
        ));
        assert!(caption_file_status_misses(
            later_cap_fs,
            &["live".to_string()]
        ));
        assert!(icon_click_fallback_size_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            Some(20),
            None
        ));
        let later_fb_sz = r#"<VAST>
<IconClickFallbackImage width="1" height="1"></IconClickFallbackImage>
<IconClickFallbackImage width = "20" height="20"></IconClickFallbackImage>
</VAST>"#;
        assert!(!icon_click_fallback_size_misses(
            later_fb_sz,
            Some(20),
            None
        ));
        assert!(icon_click_fallback_size_misses(later_fb_sz, Some(40), None));
        assert!(icon_click_fallback_size_misses(
            later_fb_sz,
            Some(20),
            Some(40)
        ));
        assert!(!icon_click_fallback_size_misses(
            later_fb_sz,
            Some(20),
            Some(20)
        ));
        let split_fb_sz = r#"<VAST>
<IconClickFallbackImage width="20" height="1"></IconClickFallbackImage>
<IconClickFallbackImage width="1" height="20"></IconClickFallbackImage>
</VAST>"#;
        assert!(icon_click_fallback_size_misses(
            split_fb_sz,
            Some(20),
            Some(20)
        ));
        assert!(companion_alt_text_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["Learn more".to_string()]
        ));
        let later_alt_elem = r#"<VAST>
<AltText></AltText>
<AltText>Learn more</AltText>
</VAST>"#;
        assert!(!companion_alt_text_misses(
            later_alt_elem,
            &["Learn more".to_string()]
        ));
        assert!(companion_alt_text_misses(
            later_alt_elem,
            &["Skip".to_string()]
        ));
        assert!(!verification_parameters_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(verification_parameters_https_misses(
            r#"<VAST><VerificationParameters></VerificationParameters></VAST>"#
        ));
        assert!(verification_parameters_https_misses(
            r#"<VAST><VerificationParameters><![CDATA[http://t.example.com/omid]]></VerificationParameters></VAST>"#
        ));
        assert!(!verification_parameters_https_misses(
            r#"<VAST><VerificationParameters><![CDATA[HTTPS://t.example.com/omid]]></VerificationParameters></VAST>"#
        ));
        assert!(!verification_parameters_https_misses(
            r#"<VAST><VerificationParameters><![CDATA[{"ping":"https://t.example.com/omid"}]]></VerificationParameters></VAST>"#
        ));
        assert!(verification_parameters_https_misses(
            r#"<VAST><VerificationParameters><![CDATA[{"ping":"http://t.example.com/omid"}]]></VerificationParameters></VAST>"#
        ));
        assert!(verification_parameters_https_misses(
            r#"<VAST><VerificationParameters><![CDATA[{"k":"v"}]]></VerificationParameters></VAST>"#
        ));
        assert!(ad_type_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["video".to_string()]
        ));
        let later_ad_type = r#"<VAST>
<Ad></Ad>
<Ad adType = "video"></Ad>
</VAST>"#;
        assert!(!ad_type_misses(later_ad_type, &["video".to_string()]));
        assert!(ad_type_misses(later_ad_type, &["audio".to_string()]));
        assert!(impression_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["imp-1".to_string()]
        ));
        let later_imp_id = r#"<VAST>
<Impression></Impression>
<Impression id = "imp-1"></Impression>
</VAST>"#;
        assert!(!impression_id_misses(later_imp_id, &["imp-1".to_string()]));
        assert!(impression_id_misses(later_imp_id, &["imp-2".to_string()]));
        assert!(companion_ads_required_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["all".to_string()]
        ));
        let later_comp_req = r#"<VAST>
<CompanionAds></CompanionAds>
<CompanionAds required = "all"></CompanionAds>
</VAST>"#;
        assert!(!companion_ads_required_misses(
            later_comp_req,
            &["all".to_string()]
        ));
        assert!(companion_ads_required_misses(
            later_comp_req,
            &["any".to_string()]
        ));
        assert!(mediafile_file_size_below(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            1
        ));
        let later_mf_fs = r#"<VAST>
<MediaFile fileSize="1"></MediaFile>
<MediaFile fileSize = "1000000"></MediaFile>
</VAST>"#;
        assert!(!mediafile_file_size_below(later_mf_fs, 1000000));
        assert!(mediafile_file_size_below(later_mf_fs, 2000000));
        assert!(mediafile_media_type_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["2D".to_string()]
        ));
        let later_mf_mt = r#"<VAST>
<MediaFile></MediaFile>
<MediaFile mediaType = "2D"></MediaFile>
</VAST>"#;
        assert!(!mediafile_media_type_misses(
            later_mf_mt,
            &["2D".to_string()]
        ));
        assert!(mediafile_media_type_misses(
            later_mf_mt,
            &["360".to_string()]
        ));
        assert!(interactive_creative_file_variable_duration_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["true".to_string()]
        ));
        let later_icf_var = r#"<VAST>
<InteractiveCreativeFile></InteractiveCreativeFile>
<InteractiveCreativeFile variableDuration = "true"></InteractiveCreativeFile>
</VAST>"#;
        assert!(!interactive_creative_file_variable_duration_misses(
            later_icf_var,
            &["true".to_string()]
        ));
        assert!(interactive_creative_file_variable_duration_misses(
            later_icf_var,
            &["false".to_string()]
        ));
        assert!(creative_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["cr-1".to_string()]
        ));
        let later_cr_id = r#"<VAST>
<Creative></Creative>
<Creative id = "cr-1"></Creative>
</VAST>"#;
        assert!(!creative_id_misses(later_cr_id, &["cr-1".to_string()]));
        assert!(creative_id_misses(later_cr_id, &["cr-2".to_string()]));
        assert!(clickthrough_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["ct-1".to_string()]
        ));
        let later_ct_id = r#"<VAST>
<ClickThrough></ClickThrough>
<ClickThrough id = "ct-1"></ClickThrough>
</VAST>"#;
        assert!(!clickthrough_id_misses(later_ct_id, &["ct-1".to_string()]));
        assert!(clickthrough_id_misses(later_ct_id, &["ct-2".to_string()]));
        assert!(tracking_offset_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["00:00:15".to_string()]
        ));
        let later_trk_off = r#"<VAST>
<Tracking event="start"></Tracking>
<Tracking event="progress" offset = "00:00:15"></Tracking>
</VAST>"#;
        assert!(!tracking_offset_misses(
            later_trk_off,
            &["00:00:15".to_string()]
        ));
        assert!(tracking_offset_misses(
            later_trk_off,
            &["00:00:10".to_string()]
        ));
        assert!(clicktracking_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["clk-1".to_string()]
        ));
        let later_clk_trk = r#"<VAST>
<ClickTracking></ClickTracking>
<ClickTracking id = "clk-1"></ClickTracking>
</VAST>"#;
        assert!(!clicktracking_id_misses(
            later_clk_trk,
            &["clk-1".to_string()]
        ));
        assert!(clicktracking_id_misses(
            later_clk_trk,
            &["clk-2".to_string()]
        ));
        assert!(custom_click_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["cust-1".to_string()]
        ));
        let later_cust_clk = r#"<VAST>
<CustomClick></CustomClick>
<CustomClick id = "cust-1"></CustomClick>
</VAST>"#;
        assert!(!custom_click_id_misses(
            later_cust_clk,
            &["cust-1".to_string()]
        ));
        assert!(custom_click_id_misses(
            later_cust_clk,
            &["cust-2".to_string()]
        ));
        assert!(icon_clickthrough_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["ict-1".to_string()]
        ));
        let later_ict_id = r#"<VAST>
<IconClickThrough></IconClickThrough>
<IconClickThrough id = "ict-1"></IconClickThrough>
</VAST>"#;
        assert!(!icon_clickthrough_id_misses(
            later_ict_id,
            &["ict-1".to_string()]
        ));
        assert!(icon_clickthrough_id_misses(
            later_ict_id,
            &["ict-2".to_string()]
        ));
        assert!(companion_clickthrough_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["cct-1".to_string()]
        ));
        let later_cct_id = r#"<VAST>
<CompanionClickThrough></CompanionClickThrough>
<CompanionClickThrough id = "cct-1"></CompanionClickThrough>
</VAST>"#;
        assert!(!companion_clickthrough_id_misses(
            later_cct_id,
            &["cct-1".to_string()]
        ));
        assert!(companion_clickthrough_id_misses(
            later_cct_id,
            &["cct-2".to_string()]
        ));
        assert!(nonlinear_clickthrough_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["nlc-1".to_string()]
        ));
        let later_nlc_id = r#"<VAST>
<NonLinearClickThrough></NonLinearClickThrough>
<NonLinearClickThrough id = "nlc-1"></NonLinearClickThrough>
</VAST>"#;
        assert!(!nonlinear_clickthrough_id_misses(
            later_nlc_id,
            &["nlc-1".to_string()]
        ));
        assert!(nonlinear_clickthrough_id_misses(
            later_nlc_id,
            &["nlc-2".to_string()]
        ));
        assert!(icon_clicktracking_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["ictk-1".to_string()]
        ));
        let later_ictk_id = r#"<VAST>
<IconClickTracking></IconClickTracking>
<IconClickTracking id = "ictk-1"></IconClickTracking>
</VAST>"#;
        assert!(!icon_clicktracking_id_misses(
            later_ictk_id,
            &["ictk-1".to_string()]
        ));
        assert!(icon_clicktracking_id_misses(
            later_ictk_id,
            &["ictk-2".to_string()]
        ));
        assert!(companion_clicktracking_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["cctk-1".to_string()]
        ));
        let later_cctk_id = r#"<VAST>
<CompanionClickTracking></CompanionClickTracking>
<CompanionClickTracking id = "cctk-1"></CompanionClickTracking>
</VAST>"#;
        assert!(!companion_clicktracking_id_misses(
            later_cctk_id,
            &["cctk-1".to_string()]
        ));
        assert!(companion_clicktracking_id_misses(
            later_cctk_id,
            &["cctk-2".to_string()]
        ));
        assert!(nonlinear_clicktracking_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["nlct-1".to_string()]
        ));
        let later_nlct_id = r#"<VAST>
<NonLinearClickTracking></NonLinearClickTracking>
<NonLinearClickTracking id = "nlct-1"></NonLinearClickTracking>
</VAST>"#;
        assert!(!nonlinear_clicktracking_id_misses(
            later_nlct_id,
            &["nlct-1".to_string()]
        ));
        assert!(nonlinear_clicktracking_id_misses(
            later_nlct_id,
            &["nlct-2".to_string()]
        ));
        assert!(icon_viewtracking_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["ivt-1".to_string()]
        ));
        let later_ivt_id = r#"<VAST>
<IconViewTracking></IconViewTracking>
<IconViewTracking id = "ivt-1"></IconViewTracking>
</VAST>"#;
        assert!(!icon_viewtracking_id_misses(
            later_ivt_id,
            &["ivt-1".to_string()]
        ));
        assert!(icon_viewtracking_id_misses(
            later_ivt_id,
            &["ivt-2".to_string()]
        ));
        assert!(tracking_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["trk-1".to_string()]
        ));
        let later_trk_id = r#"<VAST>
<Tracking event="start"></Tracking>
<Tracking event="start" id = "trk-1"></Tracking>
</VAST>"#;
        assert!(!tracking_id_misses(later_trk_id, &["trk-1".to_string()]));
        assert!(tracking_id_misses(later_trk_id, &["trk-2".to_string()]));
        assert!(error_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["err-1".to_string()]
        ));
        let later_err_id = r#"<VAST>
<Error></Error>
<Error id = "err-1"></Error>
</VAST>"#;
        assert!(!error_id_misses(later_err_id, &["err-1".to_string()]));
        assert!(error_id_misses(later_err_id, &["err-2".to_string()]));
        assert!(viewable_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["v-1".to_string()]
        ));
        let later_view_elem = r#"<VAST>
<Viewable></Viewable>
<Viewable id = "v-1"></Viewable>
</VAST>"#;
        assert!(!viewable_id_misses(later_view_elem, &["v-1".to_string()]));
        assert!(viewable_id_misses(later_view_elem, &["v-2".to_string()]));
        assert!(notviewable_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["nv-1".to_string()]
        ));
        let later_nv_id = r#"<VAST>
<NotViewable></NotViewable>
<NotViewable id = "nv-1"></NotViewable>
</VAST>"#;
        assert!(!notviewable_id_misses(later_nv_id, &["nv-1".to_string()]));
        assert!(notviewable_id_misses(later_nv_id, &["nv-2".to_string()]));
        assert!(viewundetermined_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["vu-1".to_string()]
        ));
        let later_vu_id = r#"<VAST>
<ViewUndetermined></ViewUndetermined>
<ViewUndetermined id = "vu-1"></ViewUndetermined>
</VAST>"#;
        assert!(!viewundetermined_id_misses(
            later_vu_id,
            &["vu-1".to_string()]
        ));
        assert!(viewundetermined_id_misses(
            later_vu_id,
            &["vu-2".to_string()]
        ));
        assert!(survey_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["surv-1".to_string()]
        ));
        let later_surv_id = r#"<VAST>
<Survey></Survey>
<Survey id = "surv-1"></Survey>
</VAST>"#;
        assert!(!survey_id_misses(later_surv_id, &["surv-1".to_string()]));
        assert!(survey_id_misses(later_surv_id, &["surv-2".to_string()]));
        assert!(skipoffset_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["00:00:05".to_string()]
        ));
        let later_skip_off = r#"<VAST>
<Linear></Linear>
<Linear skipoffset = "00:00:05"></Linear>
</VAST>"#;
        assert!(!skipoffset_misses(
            later_skip_off,
            &["00:00:05".to_string()]
        ));
        assert!(skipoffset_misses(later_skip_off, &["00:00:10".to_string()]));
        let later_skip_nl = r#"<VAST>
<NonLinear skipoffset = "00:00:05"></NonLinear>
</VAST>"#;
        assert!(!skipoffset_misses(later_skip_nl, &["00:00:05".to_string()]));
        assert!(!duration_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["00:00:30".to_string()]
        ));
        assert!(duration_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["00:00:15".to_string()]
        ));
        let later_dur_txt = r#"<VAST>
<Duration></Duration>
<Duration>00:00:30</Duration>
</VAST>"#;
        assert!(!duration_misses(later_dur_txt, &["00:00:30".to_string()]));
        assert!(duration_misses(later_dur_txt, &["00:00:15".to_string()]));
        assert!(!expires_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["3600".to_string()]
        ));
        assert!(expires_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["7200".to_string()]
        ));
        let later_exp_txt = r#"<VAST>
<Expires></Expires>
<Expires>3600</Expires>
</VAST>"#;
        assert!(!expires_misses(later_exp_txt, &["3600".to_string()]));
        assert!(expires_misses(later_exp_txt, &["7200".to_string()]));
        assert!(icon_duration_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["00:00:15".to_string()]
        ));
        let later_icon_dur = r#"<VAST>
<Icon></Icon>
<Icon duration = "00:00:15"></Icon>
</VAST>"#;
        assert!(!icon_duration_misses(
            later_icon_dur,
            &["00:00:15".to_string()]
        ));
        assert!(icon_duration_misses(
            later_icon_dur,
            &["00:00:30".to_string()]
        ));
        assert!(icon_offset_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["00:00:00".to_string()]
        ));
        let later_icon_off = r#"<VAST>
<Icon></Icon>
<Icon offset = "00:00:00"></Icon>
</VAST>"#;
        assert!(!icon_offset_misses(
            later_icon_off,
            &["00:00:00".to_string()]
        ));
        assert!(icon_offset_misses(
            later_icon_off,
            &["00:00:05".to_string()]
        ));
        assert!(min_suggested_duration_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["00:00:15".to_string()]
        ));
        let later_msd = r#"<VAST>
<NonLinear></NonLinear>
<NonLinear minSuggestedDuration = "00:00:15"></NonLinear>
</VAST>"#;
        assert!(!min_suggested_duration_misses(
            later_msd,
            &["00:00:15".to_string()]
        ));
        assert!(min_suggested_duration_misses(
            later_msd,
            &["00:00:30".to_string()]
        ));
        assert!(!ad_serving_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["srv-001".to_string()]
        ));
        assert!(ad_serving_id_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["srv-002".to_string()]
        ));
        let later_asid_txt = r#"<VAST>
<AdServingId></AdServingId>
<AdServingId>srv-001</AdServingId>
</VAST>"#;
        assert!(!ad_serving_id_misses(
            later_asid_txt,
            &["srv-001".to_string()]
        ));
        assert!(ad_serving_id_misses(
            later_asid_txt,
            &["srv-002".to_string()]
        ));
        assert!(!ad_title_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["Test".to_string()]
        ));
        assert!(ad_title_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["Other".to_string()]
        ));
        let later_adtitle_txt = r#"<VAST>
<AdTitle></AdTitle>
<AdTitle>Test</AdTitle>
</VAST>"#;
        assert!(!ad_title_misses(later_adtitle_txt, &["Test".to_string()]));
        assert!(ad_title_misses(later_adtitle_txt, &["Other".to_string()]));
        assert!(description_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["A test ad".to_string()]
        ));
        let later_desc_txt = r#"<VAST>
<Description></Description>
<Description>A test ad</Description>
</VAST>"#;
        assert!(!description_misses(
            later_desc_txt,
            &["A test ad".to_string()]
        ));
        assert!(description_misses(later_desc_txt, &["Other".to_string()]));
        assert!(ad_parameters_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["foo=1".to_string()]
        ));
        let later_params_txt = r#"<VAST>
<AdParameters></AdParameters>
<AdParameters>foo=1</AdParameters>
</VAST>"#;
        assert!(!ad_parameters_misses(
            later_params_txt,
            &["foo=1".to_string()]
        ));
        assert!(ad_parameters_misses(
            later_params_txt,
            &["bar=2".to_string()]
        ));
        assert!(static_resource_creative_type_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["image/png".to_string()]
        ));
        let later_static_ct = r#"<VAST>
<StaticResource></StaticResource>
<StaticResource creativeType = "image/png"></StaticResource>
</VAST>"#;
        assert!(!static_resource_creative_type_misses(
            later_static_ct,
            &["image/png".to_string()]
        ));
        assert!(static_resource_creative_type_misses(
            later_static_ct,
            &["image/jpeg".to_string()]
        ));
        assert!(flash_resource_creative_type_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["application/x-shockwave-flash".to_string()]
        ));
        let later_flash_ct = r#"<VAST>
<FlashResource></FlashResource>
<FlashResource creativeType = "application/x-shockwave-flash"></FlashResource>
</VAST>"#;
        assert!(!flash_resource_creative_type_misses(
            later_flash_ct,
            &["application/x-shockwave-flash".to_string()]
        ));
        assert!(flash_resource_creative_type_misses(
            later_flash_ct,
            &["application/x-shockwave-flash2".to_string()]
        ));
        assert!(!impression_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/imp?cb=[CACHEBUSTING]&ts=[TIMESTAMP]".to_string()]
        ));
        assert!(impression_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/other".to_string()]
        ));
        let later_imp_url = r#"<VAST>
<Impression></Impression>
<Impression>https://t.example.com/imp</Impression>
</VAST>"#;
        assert!(!impression_misses(
            later_imp_url,
            &["https://t.example.com/imp".to_string()]
        ));
        assert!(impression_misses(
            later_imp_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(error_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/error".to_string()]
        ));
        let later_err_url = r#"<VAST>
<Error></Error>
<Error>https://t.example.com/error</Error>
</VAST>"#;
        assert!(!error_misses(
            later_err_url,
            &["https://t.example.com/error".to_string()]
        ));
        assert!(error_misses(
            later_err_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(clickthrough_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/click".to_string()]
        ));
        let later_clk_url = r#"<VAST>
<ClickThrough></ClickThrough>
<ClickThrough>https://t.example.com/click</ClickThrough>
</VAST>"#;
        assert!(!clickthrough_misses(
            later_clk_url,
            &["https://t.example.com/click".to_string()]
        ));
        assert!(clickthrough_misses(
            later_clk_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(survey_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/survey".to_string()]
        ));
        let later_surv_url = r#"<VAST>
<Survey></Survey>
<Survey>https://t.example.com/survey</Survey>
</VAST>"#;
        assert!(!survey_misses(
            later_surv_url,
            &["https://t.example.com/survey".to_string()]
        ));
        assert!(survey_misses(
            later_surv_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(!tracking_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/start".to_string()]
        ));
        assert!(tracking_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/other".to_string()]
        ));
        let later_trk_url = r#"<VAST>
<Tracking></Tracking>
<Tracking>https://t.example.com/start</Tracking>
</VAST>"#;
        assert!(!tracking_misses(
            later_trk_url,
            &["https://t.example.com/start".to_string()]
        ));
        assert!(tracking_misses(
            later_trk_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(viewable_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/view".to_string()]
        ));
        let later_view_url = r#"<VAST>
<Viewable></Viewable>
<Viewable>https://t.example.com/view</Viewable>
</VAST>"#;
        assert!(!viewable_misses(
            later_view_url,
            &["https://t.example.com/view".to_string()]
        ));
        assert!(viewable_misses(
            later_view_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(clicktracking_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/clk".to_string()]
        ));
        let later_clk_trk_url = r#"<VAST>
<ClickTracking></ClickTracking>
<ClickTracking>https://t.example.com/clk</ClickTracking>
</VAST>"#;
        assert!(!clicktracking_misses(
            later_clk_trk_url,
            &["https://t.example.com/clk".to_string()]
        ));
        assert!(clicktracking_misses(
            later_clk_trk_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(mezzanine_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://cdn.example.com/mezz.mp4".to_string()]
        ));
        let later_mezz_url = r#"<VAST>
<Mezzanine></Mezzanine>
<Mezzanine>https://cdn.example.com/mezz.mp4</Mezzanine>
</VAST>"#;
        assert!(!mezzanine_misses(
            later_mezz_url,
            &["https://cdn.example.com/mezz.mp4".to_string()]
        ));
        assert!(mezzanine_misses(
            later_mezz_url,
            &["https://cdn.example.com/other.mp4".to_string()]
        ));
        assert!(!mediafile_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://cdn.example.com/ad.mp4".to_string()]
        ));
        assert!(mediafile_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://cdn.example.com/other.mp4".to_string()]
        ));
        let later_mf_url = r#"<VAST>
<MediaFile></MediaFile>
<MediaFile>https://cdn.example.com/ad.mp4</MediaFile>
</VAST>"#;
        assert!(!mediafile_misses(
            later_mf_url,
            &["https://cdn.example.com/ad.mp4".to_string()]
        ));
        assert!(mediafile_misses(
            later_mf_url,
            &["https://cdn.example.com/other.mp4".to_string()]
        ));
        assert!(vast_ad_tag_uri_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://example.com/vast".to_string()]
        ));
        let later_tag_uri = r#"<VAST>
<VASTAdTagURI></VASTAdTagURI>
<VASTAdTagURI>https://example.com/vast</VASTAdTagURI>
</VAST>"#;
        assert!(!vast_ad_tag_uri_misses(
            later_tag_uri,
            &["https://example.com/vast".to_string()]
        ));
        assert!(vast_ad_tag_uri_misses(
            later_tag_uri,
            &["https://example.com/other".to_string()]
        ));
        assert!(javascript_resource_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/omid.js".to_string()]
        ));
        let later_js_url = r#"<VAST>
<JavaScriptResource></JavaScriptResource>
<JavaScriptResource>https://t.example.com/omid.js</JavaScriptResource>
</VAST>"#;
        assert!(!javascript_resource_misses(
            later_js_url,
            &["https://t.example.com/omid.js".to_string()]
        ));
        assert!(javascript_resource_misses(
            later_js_url,
            &["https://t.example.com/other.js".to_string()]
        ));
        assert!(closed_caption_file_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://cdn.example.com/cap.vtt".to_string()]
        ));
        let later_cc_url = r#"<VAST>
<ClosedCaptionFile></ClosedCaptionFile>
<ClosedCaptionFile>https://cdn.example.com/cap.vtt</ClosedCaptionFile>
</VAST>"#;
        assert!(!closed_caption_file_misses(
            later_cc_url,
            &["https://cdn.example.com/cap.vtt".to_string()]
        ));
        assert!(closed_caption_file_misses(
            later_cc_url,
            &["https://cdn.example.com/other.vtt".to_string()]
        ));
        assert!(executable_resource_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/omid.exe".to_string()]
        ));
        let later_exec_url = r#"<VAST>
<ExecutableResource></ExecutableResource>
<ExecutableResource>https://t.example.com/omid.exe</ExecutableResource>
</VAST>"#;
        assert!(!executable_resource_misses(
            later_exec_url,
            &["https://t.example.com/omid.exe".to_string()]
        ));
        assert!(executable_resource_misses(
            later_exec_url,
            &["https://t.example.com/other.exe".to_string()]
        ));
        assert!(icon_clickthrough_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://adv.example.com/i".to_string()]
        ));
        let later_ict_url = r#"<VAST>
<IconClickThrough></IconClickThrough>
<IconClickThrough>https://adv.example.com/i</IconClickThrough>
</VAST>"#;
        assert!(!icon_clickthrough_misses(
            later_ict_url,
            &["https://adv.example.com/i".to_string()]
        ));
        assert!(icon_clickthrough_misses(
            later_ict_url,
            &["https://adv.example.com/other".to_string()]
        ));
        assert!(companion_clickthrough_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://adv.example.com/c".to_string()]
        ));
        let later_cct_url = r#"<VAST>
<CompanionClickThrough></CompanionClickThrough>
<CompanionClickThrough>https://adv.example.com/c</CompanionClickThrough>
</VAST>"#;
        assert!(!companion_clickthrough_misses(
            later_cct_url,
            &["https://adv.example.com/c".to_string()]
        ));
        assert!(companion_clickthrough_misses(
            later_cct_url,
            &["https://adv.example.com/other".to_string()]
        ));
        assert!(nonlinear_clickthrough_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://adv.example.com/nl".to_string()]
        ));
        let later_nlc_url = r#"<VAST>
<NonLinearClickThrough></NonLinearClickThrough>
<NonLinearClickThrough>https://adv.example.com/nl</NonLinearClickThrough>
</VAST>"#;
        assert!(!nonlinear_clickthrough_misses(
            later_nlc_url,
            &["https://adv.example.com/nl".to_string()]
        ));
        assert!(nonlinear_clickthrough_misses(
            later_nlc_url,
            &["https://adv.example.com/other".to_string()]
        ));
        assert!(icon_viewtracking_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/iv".to_string()]
        ));
        let later_ivt_url = r#"<VAST>
<IconViewTracking></IconViewTracking>
<IconViewTracking>https://t.example.com/iv</IconViewTracking>
</VAST>"#;
        assert!(!icon_viewtracking_misses(
            later_ivt_url,
            &["https://t.example.com/iv".to_string()]
        ));
        assert!(icon_viewtracking_misses(
            later_ivt_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(interactive_creative_file_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://cdn.example.com/v.js".to_string()]
        ));
        let later_icf_url = r#"<VAST>
<InteractiveCreativeFile></InteractiveCreativeFile>
<InteractiveCreativeFile>https://cdn.example.com/v.js</InteractiveCreativeFile>
</VAST>"#;
        assert!(!interactive_creative_file_misses(
            later_icf_url,
            &["https://cdn.example.com/v.js".to_string()]
        ));
        assert!(interactive_creative_file_misses(
            later_icf_url,
            &["https://cdn.example.com/other.js".to_string()]
        ));
        assert!(static_resource_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://cdn.example.com/i.png".to_string()]
        ));
        let later_static_url = r#"<VAST>
<StaticResource></StaticResource>
<StaticResource>https://cdn.example.com/i.png</StaticResource>
</VAST>"#;
        assert!(!static_resource_misses(
            later_static_url,
            &["https://cdn.example.com/i.png".to_string()]
        ));
        assert!(static_resource_misses(
            later_static_url,
            &["https://cdn.example.com/other.png".to_string()]
        ));
        assert!(iframe_resource_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://cdn.example.com/frame.html".to_string()]
        ));
        let later_iframe_url = r#"<VAST>
<IFrameResource></IFrameResource>
<IFrameResource>https://cdn.example.com/frame.html</IFrameResource>
</VAST>"#;
        assert!(!iframe_resource_misses(
            later_iframe_url,
            &["https://cdn.example.com/frame.html".to_string()]
        ));
        assert!(iframe_resource_misses(
            later_iframe_url,
            &["https://cdn.example.com/other.html".to_string()]
        ));
        assert!(html_resource_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://cdn.example.com/ad.html".to_string()]
        ));
        let later_html_url = r#"<VAST>
<HTMLResource></HTMLResource>
<HTMLResource>https://cdn.example.com/ad.html</HTMLResource>
</VAST>"#;
        assert!(!html_resource_misses(
            later_html_url,
            &["https://cdn.example.com/ad.html".to_string()]
        ));
        assert!(html_resource_misses(
            later_html_url,
            &["https://cdn.example.com/other.html".to_string()]
        ));
        assert!(flash_resource_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://cdn.example.com/ad.swf".to_string()]
        ));
        let later_flash_url = r#"<VAST>
<FlashResource></FlashResource>
<FlashResource>https://cdn.example.com/ad.swf</FlashResource>
</VAST>"#;
        assert!(!flash_resource_misses(
            later_flash_url,
            &["https://cdn.example.com/ad.swf".to_string()]
        ));
        assert!(flash_resource_misses(
            later_flash_url,
            &["https://cdn.example.com/other.swf".to_string()]
        ));
        assert!(icon_clickfallback_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://cdn.example.com/icon.png".to_string()]
        ));
        let later_icfb_url = r#"<VAST>
<IconClickFallbackImage></IconClickFallbackImage>
<IconClickFallbackImage>https://cdn.example.com/icon.png</IconClickFallbackImage>
</VAST>"#;
        assert!(!icon_clickfallback_misses(
            later_icfb_url,
            &["https://cdn.example.com/icon.png".to_string()]
        ));
        assert!(icon_clickfallback_misses(
            later_icfb_url,
            &["https://cdn.example.com/other.png".to_string()]
        ));
        assert!(custom_click_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/custom".to_string()]
        ));
        let later_custom_url = r#"<VAST>
<CustomClick></CustomClick>
<CustomClick>https://t.example.com/custom</CustomClick>
</VAST>"#;
        assert!(!custom_click_misses(
            later_custom_url,
            &["https://t.example.com/custom".to_string()]
        ));
        assert!(custom_click_misses(
            later_custom_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(icon_clicktracking_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/i".to_string()]
        ));
        let later_ictk_url = r#"<VAST>
<IconClickTracking></IconClickTracking>
<IconClickTracking>https://t.example.com/i</IconClickTracking>
</VAST>"#;
        assert!(!icon_clicktracking_misses(
            later_ictk_url,
            &["https://t.example.com/i".to_string()]
        ));
        assert!(icon_clicktracking_misses(
            later_ictk_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(companion_clicktracking_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/c".to_string()]
        ));
        let later_cctk_url = r#"<VAST>
<CompanionClickTracking></CompanionClickTracking>
<CompanionClickTracking>https://t.example.com/c</CompanionClickTracking>
</VAST>"#;
        assert!(!companion_clicktracking_misses(
            later_cctk_url,
            &["https://t.example.com/c".to_string()]
        ));
        assert!(companion_clicktracking_misses(
            later_cctk_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(nonlinear_clicktracking_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/nl".to_string()]
        ));
        let later_nlct_url = r#"<VAST>
<NonLinearClickTracking></NonLinearClickTracking>
<NonLinearClickTracking>https://t.example.com/nl</NonLinearClickTracking>
</VAST>"#;
        assert!(!nonlinear_clicktracking_misses(
            later_nlct_url,
            &["https://t.example.com/nl".to_string()]
        ));
        assert!(nonlinear_clicktracking_misses(
            later_nlct_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(notviewable_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/nv".to_string()]
        ));
        let later_nv_url = r#"<VAST>
<NotViewable></NotViewable>
<NotViewable>https://t.example.com/nv</NotViewable>
</VAST>"#;
        assert!(!notviewable_misses(
            later_nv_url,
            &["https://t.example.com/nv".to_string()]
        ));
        assert!(notviewable_misses(
            later_nv_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(viewundetermined_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/vu".to_string()]
        ));
        let later_vu_url = r#"<VAST>
<ViewUndetermined></ViewUndetermined>
<ViewUndetermined>https://t.example.com/vu</ViewUndetermined>
</VAST>"#;
        assert!(!viewundetermined_misses(
            later_vu_url,
            &["https://t.example.com/vu".to_string()]
        ));
        assert!(viewundetermined_misses(
            later_vu_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(extension_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/ext".to_string()]
        ));
        let later_ext_url = r#"<VAST>
<Extension></Extension>
<Extension>https://t.example.com/ext</Extension>
</VAST>"#;
        assert!(!extension_misses(
            later_ext_url,
            &["https://t.example.com/ext".to_string()]
        ));
        assert!(extension_misses(
            later_ext_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(verification_parameters_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/omid".to_string()]
        ));
        let later_vp_url = r#"<VAST>
<VerificationParameters></VerificationParameters>
<VerificationParameters>https://t.example.com/omid</VerificationParameters>
</VAST>"#;
        assert!(!verification_parameters_misses(
            later_vp_url,
            &["https://t.example.com/omid".to_string()]
        ));
        assert!(verification_parameters_misses(
            later_vp_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(blocked_ad_categories_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["IAB1-1".to_string()]
        ));
        let later_blocked = r#"<VAST>
<BlockedAdCategories></BlockedAdCategories>
<BlockedAdCategories>IAB1-1</BlockedAdCategories>
</VAST>"#;
        assert!(!blocked_ad_categories_misses(
            later_blocked,
            &["IAB1-1".to_string()]
        ));
        assert!(blocked_ad_categories_misses(
            later_blocked,
            &["IAB2-2".to_string()]
        ));
        assert!(creative_extension_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["https://t.example.com/cext".to_string()]
        ));
        let later_cext_url = r#"<VAST>
<CreativeExtension></CreativeExtension>
<CreativeExtension>https://t.example.com/cext</CreativeExtension>
</VAST>"#;
        assert!(!creative_extension_misses(
            later_cext_url,
            &["https://t.example.com/cext".to_string()]
        ));
        assert!(creative_extension_misses(
            later_cext_url,
            &["https://t.example.com/other".to_string()]
        ));
        assert!(creative_extension_type_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["iab".to_string()]
        ));
        let later_cext_type = r#"<VAST>
<CreativeExtension></CreativeExtension>
<CreativeExtension type = "iab"></CreativeExtension>
</VAST>"#;
        assert!(!creative_extension_type_misses(
            later_cext_type,
            &["iab".to_string()]
        ));
        assert!(creative_extension_type_misses(
            later_cext_type,
            &["geo".to_string()]
        ));
        assert!(id_value_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["FROM-ATTR".to_string()]
        ));
        let later_id_value = r#"<VAST>
<UniversalAdId></UniversalAdId>
<UniversalAdId idValue = "FROM-ATTR"></UniversalAdId>
</VAST>"#;
        assert!(!id_value_misses(later_id_value, &["FROM-ATTR".to_string()]));
        assert!(id_value_misses(later_id_value, &["OTHER".to_string()]));
        assert!(!mediafile_codec_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["avc1.42E01E,mp4a.40.2".to_string()]
        ));
        assert!(mediafile_codec_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["hev1".to_string()]
        ));
        let later_mf_codec = r#"<VAST>
<MediaFile></MediaFile>
<MediaFile codec = "avc1.42E01E"></MediaFile>
</VAST>"#;
        assert!(!mediafile_codec_misses(
            later_mf_codec,
            &["avc1.42E01E".to_string()]
        ));
        assert!(mediafile_codec_misses(
            later_mf_codec,
            &["hev1".to_string()]
        ));
        assert!(!creative_extension_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(creative_extension_https_misses(
            r#"<VAST><CreativeExtension></CreativeExtension></VAST>"#
        ));
        assert!(creative_extension_https_misses(
            r#"<VAST><CreativeExtension><![CDATA[http://t.example.com/cext]]></CreativeExtension></VAST>"#
        ));
        let later_cext_https = r#"<VAST>
<CreativeExtension></CreativeExtension>
<CreativeExtension><![CDATA[https://t.example.com/cext]]></CreativeExtension>
</VAST>"#;
        assert!(creative_extension_https_misses(later_cext_https));
        assert!(!creative_extension_https_misses(
            r#"<VAST><CreativeExtension><![CDATA[HTTPS://t.example.com/cext]]></CreativeExtension></VAST>"#
        ));
        assert!(!notviewable_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(notviewable_https_misses(
            r#"<VAST><NotViewable></NotViewable></VAST>"#
        ));
        assert!(notviewable_https_misses(
            r#"<VAST><NotViewable><![CDATA[http://t.example.com/nv]]></NotViewable></VAST>"#
        ));
        let later_nv_https = r#"<VAST>
<NotViewable></NotViewable>
<NotViewable><![CDATA[https://t.example.com/nv]]></NotViewable>
</VAST>"#;
        assert!(notviewable_https_misses(later_nv_https));
        assert!(!notviewable_https_misses(
            r#"<VAST><NotViewable><![CDATA[HTTPS://t.example.com/nv]]></NotViewable></VAST>"#
        ));
        assert!(!viewundetermined_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(viewundetermined_https_misses(
            r#"<VAST><ViewUndetermined></ViewUndetermined></VAST>"#
        ));
        assert!(viewundetermined_https_misses(
            r#"<VAST><ViewUndetermined><![CDATA[http://t.example.com/vu]]></ViewUndetermined></VAST>"#
        ));
        let later_vu_https = r#"<VAST>
<ViewUndetermined></ViewUndetermined>
<ViewUndetermined><![CDATA[https://t.example.com/vu]]></ViewUndetermined>
</VAST>"#;
        assert!(viewundetermined_https_misses(later_vu_https));
        assert!(!viewundetermined_https_misses(
            r#"<VAST><ViewUndetermined><![CDATA[HTTPS://t.example.com/vu]]></ViewUndetermined></VAST>"#
        ));
        assert!(!custom_click_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(custom_click_https_misses(
            r#"<VAST><CustomClick></CustomClick></VAST>"#
        ));
        assert!(custom_click_https_misses(
            r#"<VAST><CustomClick><![CDATA[http://t.example.com/custom]]></CustomClick></VAST>"#
        ));
        let later_custom_https = r#"<VAST>
<CustomClick></CustomClick>
<CustomClick><![CDATA[https://t.example.com/custom]]></CustomClick>
</VAST>"#;
        assert!(custom_click_https_misses(later_custom_https));
        assert!(!custom_click_https_misses(
            r#"<VAST><CustomClick><![CDATA[HTTPS://t.example.com/custom]]></CustomClick></VAST>"#
        ));
        assert!(!icon_clicktracking_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(icon_clicktracking_https_misses(
            r#"<VAST><IconClickTracking></IconClickTracking></VAST>"#
        ));
        assert!(icon_clicktracking_https_misses(
            r#"<VAST><IconClickTracking><![CDATA[http://t.example.com/i]]></IconClickTracking></VAST>"#
        ));
        let later_ictk_https = r#"<VAST>
<IconClickTracking></IconClickTracking>
<IconClickTracking><![CDATA[https://t.example.com/i]]></IconClickTracking>
</VAST>"#;
        assert!(icon_clicktracking_https_misses(later_ictk_https));
        assert!(!icon_clicktracking_https_misses(
            r#"<VAST><IconClickTracking><![CDATA[HTTPS://t.example.com/i]]></IconClickTracking></VAST>"#
        ));
        assert!(!companion_clicktracking_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(companion_clicktracking_https_misses(
            r#"<VAST><CompanionClickTracking></CompanionClickTracking></VAST>"#
        ));
        assert!(companion_clicktracking_https_misses(
            r#"<VAST><CompanionClickTracking><![CDATA[http://t.example.com/c]]></CompanionClickTracking></VAST>"#
        ));
        let later_cctk_https = r#"<VAST>
<CompanionClickTracking></CompanionClickTracking>
<CompanionClickTracking><![CDATA[https://t.example.com/c]]></CompanionClickTracking>
</VAST>"#;
        assert!(companion_clicktracking_https_misses(later_cctk_https));
        assert!(!companion_clicktracking_https_misses(
            r#"<VAST><CompanionClickTracking><![CDATA[HTTPS://t.example.com/c]]></CompanionClickTracking></VAST>"#
        ));
        assert!(!nonlinear_clicktracking_https_misses(include_str!(
            "../../../fixtures/ok-https-mp4.xml"
        )));
        assert!(nonlinear_clicktracking_https_misses(
            r#"<VAST><NonLinearClickTracking></NonLinearClickTracking></VAST>"#
        ));
        assert!(nonlinear_clicktracking_https_misses(
            r#"<VAST><NonLinearClickTracking><![CDATA[http://t.example.com/nl]]></NonLinearClickTracking></VAST>"#
        ));
        let later_nlct_https = r#"<VAST>
<NonLinearClickTracking></NonLinearClickTracking>
<NonLinearClickTracking><![CDATA[https://t.example.com/nl]]></NonLinearClickTracking>
</VAST>"#;
        assert!(nonlinear_clicktracking_https_misses(later_nlct_https));
        assert!(!nonlinear_clicktracking_https_misses(
            r#"<VAST><NonLinearClickTracking><![CDATA[HTTPS://t.example.com/nl]]></NonLinearClickTracking></VAST>"#
        ));
        assert!(blocked_ad_categories_authority_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["IAB".to_string()]
        ));
        let later_blocked_auth = r#"<VAST>
<BlockedAdCategories></BlockedAdCategories>
<BlockedAdCategories authority = "IAB">IAB1-1</BlockedAdCategories>
</VAST>"#;
        assert!(!blocked_ad_categories_authority_misses(
            later_blocked_auth,
            &["IAB".to_string()]
        ));
        assert!(blocked_ad_categories_authority_misses(
            later_blocked_auth,
            &["IABTECHLAB".to_string()]
        ));
        assert!(mediafile_minbitrate_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["1000".to_string()]
        ));
        let later_mf_minbr = r#"<VAST>
<MediaFile></MediaFile>
<MediaFile minBitrate = "1000"></MediaFile>
</VAST>"#;
        assert!(!mediafile_minbitrate_misses(
            later_mf_minbr,
            &["1000".to_string()]
        ));
        assert!(mediafile_minbitrate_misses(
            later_mf_minbr,
            &["2000".to_string()]
        ));
        assert!(mediafile_maxbitrate_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["5000".to_string()]
        ));
        let later_mf_maxbr = r#"<VAST>
<MediaFile></MediaFile>
<MediaFile maxBitrate = "5000"></MediaFile>
</VAST>"#;
        assert!(!mediafile_maxbitrate_misses(
            later_mf_maxbr,
            &["5000".to_string()]
        ));
        assert!(mediafile_maxbitrate_misses(
            later_mf_maxbr,
            &["8000".to_string()]
        ));
        assert!(mezzanine_minbitrate_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["1000".to_string()]
        ));
        let later_mezz_minbr = r#"<VAST>
<Mezzanine></Mezzanine>
<Mezzanine minBitrate = "1000"></Mezzanine>
</VAST>"#;
        assert!(!mezzanine_minbitrate_misses(
            later_mezz_minbr,
            &["1000".to_string()]
        ));
        assert!(mezzanine_minbitrate_misses(
            later_mezz_minbr,
            &["2000".to_string()]
        ));
        assert!(mezzanine_maxbitrate_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["50000".to_string()]
        ));
        let later_mezz_maxbr = r#"<VAST>
<Mezzanine></Mezzanine>
<Mezzanine maxBitrate = "50000"></Mezzanine>
</VAST>"#;
        assert!(!mezzanine_maxbitrate_misses(
            later_mezz_maxbr,
            &["50000".to_string()]
        ));
        assert!(mezzanine_maxbitrate_misses(
            later_mezz_maxbr,
            &["80000".to_string()]
        ));
        assert!(!mediafile_bitrate_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["1500".to_string()]
        ));
        assert!(mediafile_bitrate_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["4000".to_string()]
        ));
        let later_mf_br = r#"<VAST>
<MediaFile></MediaFile>
<MediaFile bitrate = "1500"></MediaFile>
</VAST>"#;
        assert!(!mediafile_bitrate_misses(
            later_mf_br,
            &["1500".to_string()]
        ));
        assert!(mediafile_bitrate_misses(later_mf_br, &["4000".to_string()]));
        assert!(mezzanine_bitrate_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["8000".to_string()]
        ));
        let later_mezz_br_attr = r#"<VAST>
<Mezzanine></Mezzanine>
<Mezzanine bitrate = "8000"></Mezzanine>
</VAST>"#;
        assert!(!mezzanine_bitrate_misses(
            later_mezz_br_attr,
            &["8000".to_string()]
        ));
        assert!(mezzanine_bitrate_misses(
            later_mezz_br_attr,
            &["4000".to_string()]
        ));
        assert!(mediafile_filesize_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["1000000".to_string()]
        ));
        let later_mf_fs_attr = r#"<VAST>
<MediaFile></MediaFile>
<MediaFile fileSize = "1000000"></MediaFile>
</VAST>"#;
        assert!(!mediafile_filesize_misses(
            later_mf_fs_attr,
            &["1000000".to_string()]
        ));
        assert!(mediafile_filesize_misses(
            later_mf_fs_attr,
            &["2000000".to_string()]
        ));
        assert!(mezzanine_filesize_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["5000000".to_string()]
        ));
        let later_mezz_fs_attr = r#"<VAST>
<Mezzanine></Mezzanine>
<Mezzanine fileSize = "5000000"></Mezzanine>
</VAST>"#;
        assert!(!mezzanine_filesize_misses(
            later_mezz_fs_attr,
            &["5000000".to_string()]
        ));
        assert!(mezzanine_filesize_misses(
            later_mezz_fs_attr,
            &["1000000".to_string()]
        ));
        assert!(!mediafile_width_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["640".to_string()]
        ));
        assert!(mediafile_width_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["1920".to_string()]
        ));
        let later_mf_w = r#"<VAST>
<MediaFile></MediaFile>
<MediaFile width = "640"></MediaFile>
</VAST>"#;
        assert!(!mediafile_width_misses(later_mf_w, &["640".to_string()]));
        assert!(mediafile_width_misses(later_mf_w, &["1920".to_string()]));
        assert!(!mediafile_height_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["360".to_string()]
        ));
        assert!(mediafile_height_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["1080".to_string()]
        ));
        let later_mf_h = r#"<VAST>
<MediaFile></MediaFile>
<MediaFile height = "360"></MediaFile>
</VAST>"#;
        assert!(!mediafile_height_misses(later_mf_h, &["360".to_string()]));
        assert!(mediafile_height_misses(later_mf_h, &["1080".to_string()]));
        assert!(mezzanine_width_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["1920".to_string()]
        ));
        let later_mezz_w = r#"<VAST>
<Mezzanine></Mezzanine>
<Mezzanine width = "1920"></Mezzanine>
</VAST>"#;
        assert!(!mezzanine_width_misses(later_mezz_w, &["1920".to_string()]));
        assert!(mezzanine_width_misses(later_mezz_w, &["1280".to_string()]));
        assert!(mezzanine_height_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["1080".to_string()]
        ));
        let later_mezz_h = r#"<VAST>
<Mezzanine></Mezzanine>
<Mezzanine height = "1080"></Mezzanine>
</VAST>"#;
        assert!(!mezzanine_height_misses(
            later_mezz_h,
            &["1080".to_string()]
        ));
        assert!(mezzanine_height_misses(later_mezz_h, &["720".to_string()]));
        assert!(!mediafile_delivery_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["progressive".to_string()]
        ));
        assert!(mediafile_delivery_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["streaming".to_string()]
        ));
        let later_mf_del = r#"<VAST>
<MediaFile></MediaFile>
<MediaFile delivery = "progressive"></MediaFile>
</VAST>"#;
        assert!(!mediafile_delivery_misses(
            later_mf_del,
            &["progressive".to_string()]
        ));
        assert!(mediafile_delivery_misses(
            later_mf_del,
            &["streaming".to_string()]
        ));
        assert!(icon_width_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["20".to_string()]
        ));
        let later_icon_w = r#"<VAST>
<Icon></Icon>
<Icon width = "20"></Icon>
</VAST>"#;
        assert!(!icon_width_misses(later_icon_w, &["20".to_string()]));
        assert!(icon_width_misses(later_icon_w, &["30".to_string()]));
        assert!(icon_height_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["20".to_string()]
        ));
        let later_icon_h = r#"<VAST>
<Icon></Icon>
<Icon height = "20"></Icon>
</VAST>"#;
        assert!(!icon_height_misses(later_icon_h, &["20".to_string()]));
        assert!(icon_height_misses(later_icon_h, &["30".to_string()]));
        assert!(companion_width_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["300".to_string()]
        ));
        let later_comp_w = r#"<VAST>
<Companion></Companion>
<Companion width = "300"></Companion>
</VAST>"#;
        assert!(!companion_width_misses(later_comp_w, &["300".to_string()]));
        assert!(companion_width_misses(later_comp_w, &["728".to_string()]));
        assert!(companion_height_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["250".to_string()]
        ));
        let later_comp_h = r#"<VAST>
<Companion></Companion>
<Companion height = "250"></Companion>
</VAST>"#;
        assert!(!companion_height_misses(later_comp_h, &["250".to_string()]));
        assert!(companion_height_misses(later_comp_h, &["90".to_string()]));
        assert!(nonlinear_width_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["300".to_string()]
        ));
        let later_nl_w = r#"<VAST>
<NonLinear></NonLinear>
<NonLinear width = "300"></NonLinear>
</VAST>"#;
        assert!(!nonlinear_width_misses(later_nl_w, &["300".to_string()]));
        assert!(nonlinear_width_misses(later_nl_w, &["728".to_string()]));
        assert!(nonlinear_height_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["250".to_string()]
        ));
        let later_nl_h = r#"<VAST>
<NonLinear></NonLinear>
<NonLinear height = "250"></NonLinear>
</VAST>"#;
        assert!(!nonlinear_height_misses(later_nl_h, &["250".to_string()]));
        assert!(nonlinear_height_misses(later_nl_h, &["90".to_string()]));
        assert!(companion_expanded_width_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["600".to_string()]
        ));
        let later_comp_exp_w = r#"<VAST>
<Companion></Companion>
<Companion expandedWidth = "600"></Companion>
</VAST>"#;
        assert!(!companion_expanded_width_misses(
            later_comp_exp_w,
            &["600".to_string()]
        ));
        assert!(companion_expanded_width_misses(
            later_comp_exp_w,
            &["728".to_string()]
        ));
        assert!(companion_expanded_height_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["500".to_string()]
        ));
        let later_comp_exp_h = r#"<VAST>
<Companion></Companion>
<Companion expandedHeight = "500"></Companion>
</VAST>"#;
        assert!(!companion_expanded_height_misses(
            later_comp_exp_h,
            &["500".to_string()]
        ));
        assert!(companion_expanded_height_misses(
            later_comp_exp_h,
            &["90".to_string()]
        ));
        assert!(nonlinear_expanded_width_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["600".to_string()]
        ));
        let later_nl_exp_w = r#"<VAST>
<NonLinear></NonLinear>
<NonLinear expandedWidth = "600"></NonLinear>
</VAST>"#;
        assert!(!nonlinear_expanded_width_misses(
            later_nl_exp_w,
            &["600".to_string()]
        ));
        assert!(nonlinear_expanded_width_misses(
            later_nl_exp_w,
            &["728".to_string()]
        ));
        assert!(nonlinear_expanded_height_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["500".to_string()]
        ));
        let later_nl_exp_h = r#"<VAST>
<NonLinear></NonLinear>
<NonLinear expandedHeight = "500"></NonLinear>
</VAST>"#;
        assert!(!nonlinear_expanded_height_misses(
            later_nl_exp_h,
            &["500".to_string()]
        ));
        assert!(nonlinear_expanded_height_misses(
            later_nl_exp_h,
            &["90".to_string()]
        ));
        assert!(companion_asset_width_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["300".to_string()]
        ));
        let later_comp_aw = r#"<VAST>
<Companion></Companion>
<Companion assetWidth = "300"></Companion>
</VAST>"#;
        assert!(!companion_asset_width_misses(
            later_comp_aw,
            &["300".to_string()]
        ));
        assert!(companion_asset_width_misses(
            later_comp_aw,
            &["728".to_string()]
        ));
        assert!(companion_asset_height_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["250".to_string()]
        ));
        let later_comp_ah = r#"<VAST>
<Companion></Companion>
<Companion assetHeight = "250"></Companion>
</VAST>"#;
        assert!(!companion_asset_height_misses(
            later_comp_ah,
            &["250".to_string()]
        ));
        assert!(companion_asset_height_misses(
            later_comp_ah,
            &["90".to_string()]
        ));
        assert!(icon_click_fallback_width_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["20".to_string()]
        ));
        let later_icfb_w = r#"<VAST>
<IconClickFallbackImage></IconClickFallbackImage>
<IconClickFallbackImage width = "20"></IconClickFallbackImage>
</VAST>"#;
        assert!(!icon_click_fallback_width_misses(
            later_icfb_w,
            &["20".to_string()]
        ));
        assert!(icon_click_fallback_width_misses(
            later_icfb_w,
            &["30".to_string()]
        ));
        assert!(icon_click_fallback_height_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["20".to_string()]
        ));
        let later_icfb_h = r#"<VAST>
<IconClickFallbackImage></IconClickFallbackImage>
<IconClickFallbackImage height = "20"></IconClickFallbackImage>
</VAST>"#;
        assert!(!icon_click_fallback_height_misses(
            later_icfb_h,
            &["20".to_string()]
        ));
        assert!(icon_click_fallback_height_misses(
            later_icfb_h,
            &["30".to_string()]
        ));
        assert!(!mediafile_type_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["video/mp4".to_string()]
        ));
        assert!(mediafile_type_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            &["video/webm".to_string()]
        ));
        let later_mf_type = r#"<VAST>
<MediaFile></MediaFile>
<MediaFile type = "video/mp4"></MediaFile>
</VAST>"#;
        assert!(!mediafile_type_misses(
            later_mf_type,
            &["video/mp4".to_string()]
        ));
        assert!(mediafile_type_misses(
            later_mf_type,
            &["video/webm".to_string()]
        ));
        assert!(mezzanine_size_range_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            None,
            None,
            Some(1920),
            None
        ));
        let later_mezz_maxsz = r#"<VAST>
<Mezzanine></Mezzanine>
<Mezzanine width = "1920" height="1080"></Mezzanine>
</VAST>"#;
        assert!(!mezzanine_size_range_misses(
            later_mezz_maxsz,
            None,
            None,
            Some(1920),
            Some(1080)
        ));
        assert!(mezzanine_size_range_misses(
            later_mezz_maxsz,
            None,
            None,
            Some(1280),
            None
        ));
        assert!(companion_size_range_misses(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            None,
            None,
            Some(300),
            None
        ));
        let later_comp_maxw = r#"<VAST>
<Companion></Companion>
<Companion width = "300"></Companion>
</VAST>"#;
        assert!(!companion_size_range_misses(
            later_comp_maxw,
            None,
            None,
            Some(300),
            None
        ));
        assert!(companion_size_range_misses(
            later_comp_maxw,
            None,
            None,
            Some(200),
            None
        ));
        let later_comp_maxh = r#"<VAST>
<Companion></Companion>
<Companion height = "250"></Companion>
</VAST>"#;
        assert!(!companion_size_range_misses(
            later_comp_maxh,
            None,
            None,
            None,
            Some(250)
        ));
        assert!(companion_size_range_misses(
            later_comp_maxh,
            None,
            None,
            None,
            Some(200)
        ));
        let later_icon_maxsz = r#"<VAST>
<Icon></Icon>
<Icon width = "20" height="20"></Icon>
</VAST>"#;
        assert!(!icon_size_range_misses(
            later_icon_maxsz,
            None,
            None,
            Some(20),
            Some(20)
        ));
        assert!(icon_size_range_misses(
            later_icon_maxsz,
            None,
            None,
            Some(10),
            None
        ));
        assert!(icon_size_range_misses(
            later_icon_maxsz,
            None,
            None,
            None,
            Some(10)
        ));
        let later_nl_maxsz = r#"<VAST>
<NonLinear></NonLinear>
<NonLinear width = "300" height="250"></NonLinear>
</VAST>"#;
        assert!(!nonlinear_size_range_misses(
            later_nl_maxsz,
            None,
            None,
            Some(300),
            Some(250)
        ));
        assert!(nonlinear_size_range_misses(
            later_nl_maxsz,
            None,
            None,
            Some(200),
            None
        ));
        assert!(nonlinear_size_range_misses(
            later_nl_maxsz,
            None,
            None,
            None,
            Some(200)
        ));
        let later_icfb_maxw = r#"<VAST>
<IconClickFallbackImage></IconClickFallbackImage>
<IconClickFallbackImage width = "20"></IconClickFallbackImage>
</VAST>"#;
        assert!(!icon_click_fallback_size_range_misses(
            later_icfb_maxw,
            None,
            None,
            Some(20),
            None
        ));
        assert!(icon_click_fallback_size_range_misses(
            later_icfb_maxw,
            None,
            None,
            Some(10),
            None
        ));
        let later_icfb_maxh = r#"<VAST>
<IconClickFallbackImage></IconClickFallbackImage>
<IconClickFallbackImage height = "20"></IconClickFallbackImage>
</VAST>"#;
        assert!(!icon_click_fallback_size_range_misses(
            later_icfb_maxh,
            None,
            None,
            None,
            Some(20)
        ));
        assert!(icon_click_fallback_size_range_misses(
            later_icfb_maxh,
            None,
            None,
            None,
            Some(10)
        ));
        let later_comp_exp_maxsz = r#"<VAST>
<Companion></Companion>
<Companion expandedWidth = "600" expandedHeight="400"></Companion>
</VAST>"#;
        assert!(!companion_expanded_size_range_misses(
            later_comp_exp_maxsz,
            None,
            None,
            Some(600),
            Some(400)
        ));
        assert!(companion_expanded_size_range_misses(
            later_comp_exp_maxsz,
            None,
            None,
            Some(400),
            None
        ));
        assert!(companion_expanded_size_range_misses(
            later_comp_exp_maxsz,
            None,
            None,
            None,
            Some(200)
        ));
    }
}
