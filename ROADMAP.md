# Creative Readiness: roadmap

Engineering sequence for the module scoped in [PLAN.md](PLAN.md). Not a promise that the WG will adopt a given version. `0.x` until worksheets close; `1.0` only after the object model is boring.

Working repo name: `creative-readiness`. Rename if Tech Lab already has a slug.

## Constraints

- Rust core, same family as vastlint. JSON Schema is the interchange.
- Depend on `vastlint-core` for VAST technical findings. Do not fork rule ids; map them onto `findings[].requirement_id`.
- Probe is an optional crate/feature. WASM and default CI evaluate without ffmpeg.
- Every object in PLAN has fixtures before code that consumes it.
- Defaults are `provisional` (worksheet TBD). Shipping a default is allowed. Calling it the IAB decision is not.
- No hosted graph, no CDN, no stitcher.

## Repo layout

```
creative-readiness/
  Cargo.toml                 workspace
  schemas/                   PolicyDocument, Evidence, Identity, Eligibility, Observation, PreparedMedia, CapabilitySurface
  packs/                     live-ssai, live-sgai
  fixtures/                  one directory per worksheet default
  crates/
    creative-readiness-core
    creative-readiness-probe
    creative-readiness-ama
    creative-readiness-cli
    creative-readiness-mcp
    creative-readiness-wasm
  docs/
```

## Milestones

### M0. Repo

**Done when:** git repo exists; Apache 2.0; `schemas/` stubbed; Tech Lab confirms they want an OSS module, the license, and who maintains it.

### M1. `0.1.0` Evidence + evaluate (technical)

W1, W2, W4, W11 core.

- JSON Schema: ReadinessEvidence, Eligibility, DeliveryEnvironment, ValidationAuthority
- `evaluate(creative, policy, context)` with an empty/minimal policy still runs vastlint and fills `dimensions.technical` + `findings[]`
- Lifecycle enum on the object; no `LIVE_ELIGIBLE` bit
- Fixtures: spec-valid but VPAID; missing mezzanine; expired evidence; policy-approved with empty operational

**Done when:** crate on crates.io or unpublished workspace with tests; `evaluate` golden fixtures pass; schema published in-repo.

### M2. `0.2.0` Policy documents + packs

W9, W10.

- PolicyDocument schema and loader
- `discover_policy(context)` from local packs + overlay files (no live policy directory)
- Packs: `live-ssai`, `live-sgai` covering duration, codec/MIME, mezzanine, VPAID forbidden, HTTPS media, bitrate, width/height, UniversalAdId, audio, `proactive_required` flag
- Overlay example: event-stricter duration

**Done when:** `evaluate` against `live-ssai` is the default CLI path; pack docs cite the draft requirement categories.

Optional: run a live VAST corpus under `live-ssai` and record fail-class counts. Selection bias applies; it is not a census.

### M3. `0.3.0` Transcode probe

W13 (preparability), W16 inherit-on-encode.

- `creative-readiness-probe`: ffmpeg against `transcode_ladder`
- `prepare(mezzanine, ladder) -> PreparedMedia`
- Evidence.operational from probe output (renditions, hashes, packaging)
- Claim path: SSAI JSON assertions checked against the same ladder without ffmpeg
- Feature flag; CLI `prepare` documented as reference, not production

**Done when:** one mezzanine fixture produces the ladder; hash is stable; claim-only fixture (no ffmpeg) still evaluates operational. Docs call this a reference probe, not production transcode.

### M4. `0.4.0` Identity, lookup, reuse

W5, W17.

- CreativeIdentity + fingerprint
- Local `lookup(identity)` store (embedded, file, or sqlite; not a network)
- Same hash + new `crid`: inherit technical/operational; keep policy independent
- Media hash change: miss

**Done when:** fixture "same media, two crids" inherits; fixture "new media, same crid" does not.

### M5. `0.5.0` AMA adapter

W6, W7, W8.

- Parse AMA 1.1 Ad / Audit JSON (public spec examples)
- Map audit status onto lifecycle + policy dimension
- Experimental 1.2: publisher id, deal id, distinct exchange vs publisher audit
- Known-creative lookup exposed on CapabilitySurface, not required for 1.1 round-trip
- Additive only; unknown fields preserved

**Done when:** spec examples round-trip; 1.2 fields sit behind `--experimental` / crate feature. WG note draft ready: Option C, lookup as v1.2 candidate.

### M6. `0.6.0` Auction-triggered observe

W12.

- `observe(Observation) -> Evidence` with `entry_path=auction_triggered`
- Lifecycle: unknown → observed → pending_review (and stop)
- Seller treatment (`serve` | `fallback` | `reject` | `defer`) as an input field we echo, never compute
- Inventory flag `proactive_required` from policy: observe still records, evaluate marks operational incomplete if they try to skip prep

**Done when:** five fixtures named for patterns A–E, each with treatment supplied by the fixture, not by us.

### M7. `0.7.0` Multi-authority, deals, multi-path

W3, W15, W18.

- Evidence.environments[] (SSAI-A ready, SSAI-B not, SGAI server-ready only)
- Overlay resolution: deal → publishers → policy set
- `acts_on_behalf_of` on authority
- Deal may set `auction_triggered: forbidden`

**Done when:** one creative, three environment results, one deal overlay, tests for aggregator vs publisher conflict (conflict = two findings, not a silent win).

### M8. `0.8.0` CLI + MCP + wasm

W19.

CLI: `evaluate`, `prepare`, `lookup`, `diff`, `observe`

MCP:

- `discover_policy`
- `evaluate_creative`
- `prepare_creative`
- `known_creative_lookup`
- `readiness_report`
- `diff_creative`

WASM: evaluate + lookup, no probe.

**Done when:** MCP listed in-repo; wasm playground optional (can wait for a site). Remediation stays structural (map vastlint `--fix` where it already exists). No copy rewrite.

### M9. `0.9.0` Diff, expiry, last-minute

W20, remaining W11 (EXPIRED, REVALIDATION_REQUIRED).

- `diff(old, new) -> invalidation set` using PolicyDocument.invalidation_rules
- Defaults: tracking-only keep transcode; media hash change invalidate technical+operational; new wrapper re-evaluate technical; pull → expire
- Clock: evidence past `expires_at` is expired even if dimensions were green

**Done when:** the four PLAN fixtures for last-minute pass; expiry fixture fails closed.

Shipped as `0.9.0`: pack `invalidation_rules`, `diff_vast_with_policy`, clock + emergency pull fail closed, CLI `revalidate --pull`.

### M9.1. `0.9.1` Embed-useful deepen

Stay on `0.9.x`. Do not mint `1.0.0`.

- `eligibility(evidence, opportunity)`: ready is not eligible-for-this-break
- Typed W10 pack checks: duration, MIME, min width/height, aspect
- Pack `live-csai`; event overlay `fixtures/overlays/event-sports.json`
- SGAI split: server / signaling / client_capability / execution
- `transform(encode | media_replacement)` from `transformation_rights`
- `apply_diff` onto stored evidence
- Pack findings land on `dimensions.policy`; vastlint stays `technical`
- MCP: `check_eligibility`, `capability_surface`, `transform_creative`

**Done when:** 30s creative is operationally ready and ineligible for a 15s break; live-csai evaluates without a mezzanine; SGAI execution stays not-evaluated until `client_ready`; `cargo test --workspace` is green.

### M9.2. `0.9.2` Bid gate, hybrid, W10/W13 deepen

Stay on `0.9.x`. Do not mint `1.0.0`.

- `prebid_gate`: unknown/pending may bid; serve needs ready + eligible. Does not choose seller treatment.
- `summarize`: remediable finding ids, per-path ready/not-ready
- OpenRTB-lite on `observe()` (maxduration, crid, deal, publisher, fallback_crid)
- Pack `live-hybrid`; overlays for news categories, privacy macros, declared audio/codec/container
- W10 kinds: tracking events, privacy macros, category declarations, codec, audio codec, container
- W13 claims: packaged, cached, received, accessible, trackers_preserved, mapped_ad_id
- Identity aliases: `mid` from AdServingId, `mezzanine_url`, dsp/ssp slots
- MCP: `prebid_gate`, `summarize_readiness`; CLI `gate` / `summary`; wasm gate/summary

**Done when:** 15s break on a 30s SSAI-ready creative may_bid and not may_serve; live-hybrid without mezzanine is policy not-ready; `cargo test --workspace` is green.

### M9.3. `0.9.3` Overlay select, pack expiry, fallback, review queue

Stay on `0.9.x`. Do not mint `1.0.0`.

- `select_overlays`: keyed AND-match; rank deal → publisher → event → device; unkeyed always apply
- Pack `overlays[]` auto-selected from evaluate context (`deal_id` / `publisher_id` / `event_id` / `device_family`)
- W9: `EvaluateRequest.policy` loads a PolicyDocument from disk; pack `expires_at` fails closed
- Evidence `scope` is deal, then event, then publisher, else environment
- Eligibility mismatches deal / event / publisher scope
- `device_family` requirement fails closed when the device is unknown
- `lookup_crid` / `lookup_fallback` (echo `fallback_crid`; do not choose treatment)
- `review_queue`: local store rows in `observed` or `pending_review`
- CLI: `--policy`, `--deal/--publisher/--event/--device`, `--auction-triggered`, `select-overlays`, `queue`, `lookup --crid/--fallback-crid`
- MCP: `observe_creative`, `select_overlays`, `review_queue`, `lookup_fallback`; evaluate accepts custom policy and overlay context
- WASM: observe / select_overlays / queue / lookup_fallback
- Schema: `evaluate-request.json`; `require_macros` must declare `value` or `values`

**Done when:** sports overlay caps a 30s creative; expired pack fails closed; deal-scoped evidence misses a different deal; `cargo test --workspace` is green.

### M9.4. `0.9.4` Review-fix

Stay on `0.9.x`. Do not mint `1.0.0`.

- `observe` uses `entry_path=auction_triggered` and deal/publisher from OpenRTB-lite; overlays are selected against that context
- `apply_reuse` keeps policy independent: not-ready policy does not become technically/operationally ready
- Device overlay `device_family` is a selector; evaluate fail-closed uses the request device
- Identity store first-wins on fingerprint; later crids alias `by_crid`
- Fingerprint is `hash:` or `url:` plus `|uad:` when UniversalAdId is present
- Missing MediaFile HTTPS and unparseable pack `expires_at` fail closed
- Diff: duration/codec/category/size is `diff.creative-body-changed`, not tracking-only
- `live-hybrid` emits SGAI split; probe packaging from ladder; transform encode checks the ladder
- CLI `--overlay` parses a catalog array and always selects; `--paths` forwards deal/event/device/policy
- MCP/CLI/wasm `evaluate_authorities`; AMA 1.1 surface does not claim lookup or overlay select
- Packs declare `authority`, `environments[]`, and ladder `packaging`
- Nested-repo CI checks out sibling `vastlint`
- Refine: live packs `0.2.10` require TIMESTAMP with CACHEBUSTING; `unknown_creative_treatment` is informational on all four packs and is echoed on evidence
- Refine: identity blanks (empty mid, crid, UAdId) become None; `media_https` rejects ftp and protocol-relative MediaFile URLs
- Refine: leftover empty `deal_id` is not a deal key on overlay select or path scope; `require_macros` matches IAB tokens including inner whitespace, not title prose
- Refine: `require_macros` scans Impression, Tracking, ClickTracking, and Error URLs only; BidGate leftover empty `deal_id` does not block serve
- Refine: `require_macros` also scans ViewableImpression URLs (Viewable, NotViewable, ViewUndetermined); leftover empty overlay `publisher_id` is not stamped on findings
- Refine: `require_macros` scans Icon and Companion tracking URLs; leftover empty `event_id` is not an event key on overlay rank
- Refine: `require_macros` scans NonLinearClickTracking and ClickThrough URLs; leftover empty `device_family` is not a device key, and evaluate trims the request device
- Refine: `require_macros` scans Survey, Extension, and AdVerification JavaScriptResource URLs
- Refine: media facts prefer vastlint `inspect_document` for duration, size, MIME, bitrate, and MediaFile URL, with string-scan fallback; leftover empty `require_element` names are a warning
- Refine: leftover empty `require_tracking_events` names are dropped; codecs and maxBitrate are read from a later MediaFile when the first has none
- Refine: leftover empty codec and container values are dropped; `media_https` and `media_url` prefer a later HTTPS MediaFile over an earlier HTTP one
- Refine: leftover empty `category_forbidden` values are dropped; `media_https` and `mezzanine_url` prefer a later HTTPS Mezzanine over an earlier HTTP one
- Refine: leftover empty `media_type` and `device_family` values are dropped
- Refine: leftover empty `require_macros` values warn instead of falling back to privacy macros; leftover empty `aspect_ratio` does not default to 16:9
- Refine: leftover empty `min`/`max` on duration, bitrate, and size warn; `media_type` passes when a later MediaFile MIME matches
- Refine: `container`, `min_width`, `min_height`, and `aspect_ratio` pass when a later MediaFile matches
- Refine: `min_bitrate`, `max_bitrate`, `codec`, and `audio_codec` pass when a later MediaFile matches
- Refine: min and max bitrate require one MediaFile inside both bounds; MediaFacts follow the preferred HTTPS file
- Refine: min width and min height require one MediaFile; codec and audio_codec require one MediaFile
- Refine: aspect_ratio joins size mins on one MediaFile; media_type and container join on one MediaFile
- Refine: min bitrate and codec require one MediaFile; media_url follows the preferred HTTPS file used by MediaFacts
- Refine: size, bitrate, codec, and MIME join on one MediaFile when those kinds share an overlay
- Refine: MediaFile size, MIME, and bitrate fill from vastlint inspect when the string scanner misses attributes
- Refine: `delivery` is a W10 kind; progressive vs streaming joins MIME, container, size, bitrate, and codec on one MediaFile
- Refine: `require_tracking_events` reads Linear Tracking only; Companion events do not count; ClosedCaptionFile is a sample overlay
- Refine: `forbid_vpaid` reads later InteractiveCreativeFile and NonLinear; `max_skipoffset_seconds` fails closed on percent; first nonempty Creative@id is crid
- Refine: duration min/max read every Linear Duration (facts keep the longest); skipoffset allows spaces around `=`; `require_macros` scans IconClickThrough, StaticResource, and IFrameResource
- Refine: unparseable Duration fails closed even when a later Linear parses; `require_macros` scans HTMLResource, FlashResource, and AdParameters
- Refine: `caption_language` matches ClosedCaptionFile@language; `require_macros` scans IconClickFallbackImage and VerificationParameters
- Refine: `caption_type` matches ClosedCaptionFile@type; `id_registry` matches UniversalAdId@idRegistry; skipoffset also reads NonLinear
- Refine: UniversalAdId `idValue` fills identity and require_element; `icon_program` matches Icon@program; `min_suggested_duration_seconds` reads NonLinear
- Refine: `ad_system_version` matches AdSystem@version; `min_icon_duration_seconds` and `max_icon_offset_seconds` read Icon; `min_companion_width` requires a Companion size
- Refine: `min_companion_height` joins width on one Companion; `icon_x_position` and `icon_y_position` match Icon placement; `min_ad_serving_id_length` reads first nonempty AdServingId
- Refine: `vast_version` matches VAST@version; `creative_sequence` AND-matches Creative@sequence; `max_expires_seconds` and `min_expires_seconds` read every Expires clock
- Refine: `ad_sequence` AND-matches Ad@sequence; `wrapper_https` requires HTTPS VASTAdTagURI hops; `verification_vendor` matches Verification@vendor; `pricing_model` matches Pricing@model
- Refine: `pricing_currency` matches Pricing@currency; `error_https` requires HTTPS Error URLs; `viewable_https` requires ViewableImpression with an HTTPS Viewable URL
- Refine: `impression_https` requires HTTPS Impression URLs; `clickthrough_https` requires HTTPS ClickThrough; `survey_https` requires HTTPS Survey when present
- Refine: `clicktracking_https` requires HTTPS click-tracking URLs when present; `icon_clickthrough_https` requires HTTPS IconClickThrough when present; `category_authority` matches Category@authority
- Refine: `javascript_resource_https` requires HTTPS JavaScriptResource when present; `companion_clickthrough_https` requires HTTPS CompanionClickThrough when present; `advertiser` matches Advertiser text
- Refine: `executable_resource_https` requires HTTPS ExecutableResource when present; `nonlinear_clickthrough_https` requires HTTPS NonLinearClickThrough when present; `min_description_length` reads first nonempty Description
- Refine: `static_resource_https` requires HTTPS StaticResource when present; `iframe_resource_https` requires HTTPS IFrameResource when present; `min_ad_title_length` reads first nonempty AdTitle
- Refine: `html_resource_https` requires HTTPS HTMLResource when present; `flash_resource_https` requires HTTPS FlashResource when present; `min_ad_parameters_length` reads first nonempty AdParameters
- Refine: `closed_caption_https` requires HTTPS ClosedCaptionFile when present; `icon_viewtracking_https` requires HTTPS IconViewTracking when present; `ad_system` matches AdSystem text
- Refine: `icon_clickfallback_https` requires HTTPS IconClickFallbackImage when present; `mezzanine_https` requires HTTPS Mezzanine when present; `category` matches Category text
- Refine: `tracking_https` requires HTTPS Tracking when present; `pricing` matches Pricing text; `extension_https` requires HTTPS Extension when present
- Refine: `interactive_creative_file_https` requires HTTPS InteractiveCreativeFile when present; `universal_ad_id` matches UniversalAdId text or idValue; `api_framework` allow-lists present apiFramework values
- Refine: `creative_adid` matches Creative AdID; `interactive_creative_file_type` matches InteractiveCreativeFile type; `viewable_impression_id` matches ViewableImpression id
- Refine: `companion_type` matches Companion type or StaticResource creativeType; `icon_type` matches Icon type; `ad_id` matches Ad id
- Refine: `mediafile_id` matches MediaFile id; `companion_adslot_id` matches Companion adSlotId; `icon_alt_text` matches Icon altText
- Refine: `companion_id` matches Companion id; `icon_hover_text` matches Icon hoverText; `nonlinear_id` matches NonLinear id
- Refine: `companion_rendering_mode` matches Companion renderingMode; `icon_pxratio` matches Icon pxratio; `nonlinear_scalable` matches NonLinear scalable
- Refine: `min_companion_expanded_width` and `min_companion_expanded_height` join on one Companion; `nonlinear_maintain_aspect_ratio` matches NonLinear maintainAspectRatio
- Refine: `min_companion_asset_width` and `min_companion_asset_height` join on one Companion; `min_nonlinear_expanded_width` and `min_nonlinear_expanded_height` join on one NonLinear
- Refine: `companion_xml_encoded` matches Companion xmlEncoded; `companion_pxratio` matches Companion pxratio; `min_nonlinear_width` and `min_nonlinear_height` join on one NonLinear; `api_framework` also scans Icon and Companion
- Refine: `wrapper_follow_additional_wrappers`, `wrapper_allow_multiple_ads`, and `wrapper_fallback_on_no_ad` match Wrapper flags; `api_framework` also scans Creative
- Refine: `ad_conditional_ad` matches Ad conditionalAd; `mediafile_scalable` matches MediaFile scalable; `mediafile_maintain_aspect_ratio` matches MediaFile maintainAspectRatio
- Refine: `executable_resource_type` matches ExecutableResource type; `javascript_resource_browser_optional` matches JavaScriptResource browserOptional; `extension_type` matches Extension type
- Refine: `mezzanine_type` matches Mezzanine type; `mezzanine_delivery` matches Mezzanine delivery; `ad_parameters_xml_encoded` matches AdParameters xmlEncoded
- Refine: `mezzanine_codec` matches Mezzanine codec or codecs; `min_mezzanine_file_size` reads fileSize; `min_mezzanine_width` and `min_mezzanine_height` join on one Mezzanine
- Refine: `min_mezzanine_bitrate` and `max_mezzanine_bitrate` join on one Mezzanine; `maxBitrate` fills bitrate; `mezzanine_media_type` matches Mezzanine mediaType
- Refine: `mezzanine_scalable` matches Mezzanine scalable; `mezzanine_maintain_aspect_ratio` matches Mezzanine maintainAspectRatio; `mezzanine_id` matches Mezzanine id
- Refine: `api_framework` also scans Mezzanine; `icon_xml_encoded` matches Icon xmlEncoded; `nonlinear_xml_encoded` matches NonLinear xmlEncoded
- Refine: `min_icon_width` and `min_icon_height` join on one Icon; `caption_file_status` matches ClosedCaptionFile fileStatus
- Refine: `clicktracking_https` also scans CustomClick; `min_icon_click_fallback_width` and `min_icon_click_fallback_height` join on one IconClickFallbackImage; `api_framework` also scans Verification
- Refine: `advertiser_id` matches Advertiser id; `companion_alt_text` matches Companion AltText; `verification_parameters_https` requires HTTPS URLs inside VerificationParameters when present
- Refine: `ad_type` matches Ad adType; `impression_id` matches Impression id; `companion_ads_required` matches CompanionAds required
- Refine: `min_mediafile_file_size` reads MediaFile fileSize; `mediafile_media_type` matches MediaFile mediaType; `interactive_creative_file_variable_duration` matches InteractiveCreativeFile variableDuration
- Refine: `creative_id` matches Creative id; `clickthrough_id` matches ClickThrough id; `tracking_offset` matches Tracking offset
- Refine: `clicktracking_id` matches ClickTracking id; `custom_click_id` matches CustomClick id; `icon_clickthrough_id` matches IconClickThrough id
- Refine: `companion_clickthrough_id` matches CompanionClickThrough id; `nonlinear_clickthrough_id` matches NonLinearClickThrough id; `icon_clicktracking_id` matches IconClickTracking id
- Refine: `companion_clicktracking_id` matches CompanionClickTracking id; `nonlinear_clicktracking_id` matches NonLinearClickTracking id; `icon_viewtracking_id` matches IconViewTracking id
- Refine: `tracking_id` matches Tracking id; `error_id` matches Error id; `viewable_id` matches Viewable id
- Refine: `notviewable_id` matches NotViewable id; `viewundetermined_id` matches ViewUndetermined id; `survey_id` matches Survey id
- Refine: `skipoffset` matches Linear or NonLinear skipoffset; `duration` matches Duration text; `expires` matches Expires text
- Refine: `icon_duration` matches Icon duration; `icon_offset` matches Icon offset; `min_suggested_duration` matches NonLinear minSuggestedDuration
- Refine: `ad_serving_id` matches AdServingId text; `ad_title` matches AdTitle text; `description` matches Description text
- Refine: `ad_parameters` matches AdParameters text; `static_resource_creative_type` matches StaticResource creativeType; `flash_resource_creative_type` matches FlashResource creativeType
- Refine: `impression` matches Impression URL; `error` matches Error URL; `clickthrough` matches ClickThrough URL
- Refine: `survey` matches Survey URL; `tracking` matches Tracking URL; `viewable` matches Viewable URL
- Refine: `clicktracking` matches ClickTracking URL; `mezzanine` matches Mezzanine URL; `mediafile` matches MediaFile URL
- Refine: `vast_ad_tag_uri` matches VASTAdTagURI URL; `javascript_resource` matches JavaScriptResource URL; `closed_caption_file` matches ClosedCaptionFile URL
- Refine: `executable_resource` matches ExecutableResource URL; `icon_clickthrough` matches IconClickThrough URL; `companion_clickthrough` matches CompanionClickThrough URL
- Refine: `nonlinear_clickthrough` matches NonLinearClickThrough URL; `icon_viewtracking` matches IconViewTracking URL; `interactive_creative_file` matches InteractiveCreativeFile URL
- Refine: `static_resource` matches StaticResource URL; `iframe_resource` matches IFrameResource URL; `html_resource` matches HTMLResource URL
- Refine: `flash_resource` matches FlashResource URL; `icon_clickfallback` matches IconClickFallbackImage URL; `custom_click` matches CustomClick URL
- Refine: `icon_clicktracking` matches IconClickTracking URL; `companion_clicktracking` matches CompanionClickTracking URL; `nonlinear_clicktracking` matches NonLinearClickTracking URL
- Refine: `notviewable` matches NotViewable URL; `viewundetermined` matches ViewUndetermined URL; `extension` matches Extension URL
- Refine: `verification_parameters` matches VerificationParameters URL; `blocked_ad_categories` matches BlockedAdCategories text; `creative_extension` matches CreativeExtension URL
- Refine: `creative_extension_type` matches CreativeExtension type; `id_value` matches UniversalAdId idValue; `mediafile_codec` matches MediaFile codec or codecs
- Refine: `max_width` and `max_height` pass when one MediaFile sits inside both size caps
- Refine: `creative_extension_https` requires HTTPS CreativeExtension when present; `notviewable_https` requires HTTPS NotViewable when present; `viewundetermined_https` requires HTTPS ViewUndetermined when present
- Refine: `custom_click_https` requires HTTPS CustomClick when present; `icon_clicktracking_https` requires HTTPS IconClickTracking when present; `companion_clicktracking_https` requires HTTPS CompanionClickTracking when present
- Refine: `nonlinear_clicktracking_https` requires HTTPS NonLinearClickTracking when present; `blocked_ad_categories_authority` matches BlockedAdCategories authority; `mediafile_minbitrate` matches MediaFile minBitrate
- Refine: `mediafile_maxbitrate` matches MediaFile maxBitrate; `mezzanine_minbitrate` matches Mezzanine minBitrate; `mezzanine_maxbitrate` matches Mezzanine maxBitrate
- Refine: `mediafile_bitrate` matches MediaFile bitrate; `mezzanine_bitrate` matches Mezzanine bitrate; `mediafile_filesize` matches MediaFile fileSize
- Refine: `mezzanine_filesize` matches Mezzanine fileSize; `mediafile_width` matches MediaFile width; `mediafile_height` matches MediaFile height
- Refine: `mezzanine_width` matches Mezzanine width; `mezzanine_height` matches Mezzanine height; `mediafile_delivery` matches MediaFile delivery
- Refine: `icon_width` matches Icon width; `icon_height` matches Icon height; `companion_width` matches Companion width
- Refine: `companion_height` matches Companion height; `nonlinear_width` matches NonLinear width; `nonlinear_height` matches NonLinear height
- Refine: `companion_expanded_width` matches Companion expandedWidth; `companion_expanded_height` matches Companion expandedHeight; `nonlinear_expanded_width` matches NonLinear expandedWidth
- Refine: `nonlinear_expanded_height` matches NonLinear expandedHeight; `companion_asset_width` matches Companion assetWidth; `companion_asset_height` matches Companion assetHeight
- Refine: `icon_click_fallback_width` matches IconClickFallbackImage width; `icon_click_fallback_height` matches IconClickFallbackImage height; `mediafile_type` matches MediaFile type
- Refine: `max_mezzanine_width` and `max_mezzanine_height` pass when one Mezzanine sits inside both size caps; `max_companion_width` caps Companion width
- Refine: `max_companion_height` caps Companion height; `max_icon_width` and `max_icon_height` pass when one Icon sits inside both size caps
- Refine: `max_nonlinear_width` and `max_nonlinear_height` pass when one NonLinear sits inside both size caps; `max_icon_click_fallback_width` caps IconClickFallbackImage width
- Refine: `max_icon_click_fallback_height` caps IconClickFallbackImage height; `max_companion_expanded_width` and `max_companion_expanded_height` pass when one Companion sits inside both expanded size caps
- Refine: `max_companion_asset_width` and `max_companion_asset_height` join min and max asset size on one Companion; `max_nonlinear_expanded_width` and `max_nonlinear_expanded_height` do the same for one NonLinear

**Done when:** reuse of a policy-not-ready creative stays not-ready; a 30s body edit is not tracking-only; marketplace vs publisher disagreement keeps both findings; `cargo test --workspace` is green.

### M10. `1.0.0` Freeze

Only when:

- Schema has had one compatibility window (additive fields only for a release)
- Probe, AMA, observe, multi-path are in the same evidence object
- WG comment submitted (Option C, evidence object, probe as reference prep, lookup as v1.2 candidate, disposition out of scope)
- Docs: capability surface ([docs/capability-surface.md](docs/capability-surface.md)), pack authoring ([docs/pack-authoring.md](docs/pack-authoring.md)); README states this is not the IAB standard

If worksheets still TBD, stay on `0.9.x` and label `1.0` later.

## Suggested calendar (single maintainer)

Not a commitment. Compress if the written Tech Lab ask has a date.

| When | Milestone |
| --- | --- |
| Week 0 | M0 |
| Weeks 1–3 | M1 |
| Weeks 3–5 | M2 |
| Weeks 5–7 | M3 |
| Weeks 7–8 | M4 |
| Weeks 8–10 | M5 + WG note |
| Weeks 10–11 | M6 |
| Weeks 11–13 | M7 |
| Weeks 13–15 | M8 |
| Weeks 15–16 | M9 |
| After WG motion or 0.9 soak | M10 |

## Tests

- Golden JSON in `fixtures/` per worksheet default
- Schema validation on every object we emit
- vastlint finding map: no orphan rule ids
- Probe: skip on runners without ffmpeg; one recorded hash vector for linux/macos
- AMA: official examples + 1.2 synthetic
- Optional live-VAST corpus script: not a unit test; fail-class counts only

## WG sync

Do not rewrite their Google Doc. After M5, send one comment: Option C, evidence object, probe as reference prep, known-creative lookup as a v1.2 candidate, auction disposition out of scope. After M9, send fixture pack. Stop. If they want the repo under `IABTechLab/`, CLA then move canonical; keep a mirror.

## Later (not 0.x)

- Proprietary SSP adapters beyond AMA (same CapabilitySurface)
- Deal-sync / richer OpenRTB via rtblint as optional dep
- Event-specific overlay packs once someone publishes citable constraints beyond the fixtures
- Site playground (evaluate wasm)
- Homebrew / Action once CLI is dull

Those are the same module. Not a second product.
