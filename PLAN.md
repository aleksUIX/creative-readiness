# Creative Readiness: scope

Product scope for the open-source module. Engineering sequence is [ROADMAP.md](ROADMAP.md). Draft source: IAB Tech Lab Advanced TV v0.02 (`proposal.md`). Worksheets 2026-08-27. All 20 Final Decisions TBD.

Tech Lab asked for an OSS module. That is the product: a library members embed. Not a hosted approval desk, not MediaTailor, not an SSP.

## One sentence

A creative is ready for a named live environment when policy, technical, and operational requirements are satisfied under a named authority, with identity and evidence that have a scope, a version, and an expiry.

## What this is

A checkable live-execution contract: not only "does the XML parse" but "does this creative play in this environment, under this authority, with evidence that expires." It is not an IAB standard by itself, and it is not a transcode farm.

## In the module

Everything the draft lists as in scope, implemented as **data + evaluate + reference probe**:

- Live CTV / streaming policies and evaluation
- Programmatic and direct-sold context on the same object
- Proactive registration path (evaluate before auction)
- Auction-triggered path (observe unknown creative, emit lifecycle, do not decide serve vs fallback)
- Creative ingest/review **semantics** (Option C capability surface)
- AMA 1.1 adapter; AMA 1.2 publisher/deal fields as experimental
- SSAI, SGAI, CSAI, hybrid as environment types
- Validation, approval **state**, operational **claims and probe**
- Distribution / cache **claims**, bound to media identity
- Readiness vs eligibility as separate objects
- Unknown / pending-review states
- Structured findings (stable ids)
- Multi-authority evidence (one creative, many authorities)
- Identity, versioning, reuse
- Last-minute diff: which change invalidates which dimension
- Agent consumption (MCP), not agent architecture

## Out of the module

Matches the draft out-of-scope list, plus things that would recast the product:

| Out | Why |
| --- | --- |
| Publisher content-moderation classifiers | Draft out of scope. Policy pack may *declare* category rules; we do not score creative content. |
| Measurement / attribution standards | pixellint already covers that hop. |
| Player UX | Draft out of scope. |
| Creative-agent internals | Draft out of scope. We expose MCP. We do not design their planner. |
| Campaign decisioning / pacing | Draft out of scope. |
| A specific transcode or CDN vendor | Draft out of scope. We own the ladder and a reference probe. |
| Serve / fallback / reject as our policy | Draft forbids prescribing it. We emit the facts a seller uses. |
| Creative generation / narrative rewrite | Workshop: no. |
| Shared multi-SSP known-creative graph | Identity is local/embeddable. |
| GPU farm, origin, CDN, DRM, stitcher, live burst fill | Members operate that. IAB will not host it. |
| AMA v2 as if consensus existed | Experimental fields only until the WG closes W7/W8. |

## Objects

These are the module. Every API reads or writes one of them.

### PolicyDocument

Machine-readable requirements for one or more delivery environments.

- `id`, `version`, `expires_at`
- `authority` (who publishes this policy)
- `environments[]` (`ssai` / `sgai` / `csai` / `hybrid`)
- `overlays[]` keyed by publisher, deal, event, device family
- `requirements[]` (Worksheet 10): media type, codec, bitrate, resolution, aspect, duration, audio, container, VAST constraints, identifiers, tracking, privacy macros, SSAI constraints, SGAI constraints, event/content/category **declarations**, device limits, `proactive_required`, `unknown_creative_treatment` (informational)
- `transcode_ladder` (renditions, packaging HLS/DASH, mezzanine rules)
- `transformation_rights` (which downstream encodes inherit policy vs mint a new identity)
- `invalidation_rules` (Worksheet 20 defaults until the WG votes)

### DeliveryEnvironment

Named execution context. Not "the internet."

- `id`, `architecture`, streaming platform, SSAI/SGAI vendor slot, device family, manifest protocol, event infrastructure flags

### ValidationAuthority

Who may assert a dimension.

- `id`, `role` (ssp, marketplace, publisher, streaming_platform, ad_server, ssai, sgai, other)
- `acts_on_behalf_of[]` (marketplace aggregating publishers)

### CreativeIdentity

How we know it is the same asset.

- DSP/SSP ids, `crid`, `mid`, `universal_ad_id`, media URL, content hash, mezzanine id
- `version`
- Fingerprint for "have we seen this media under another system id"

### ReadinessEvidence

The output. Workshop sentence: evidence, scope, authority, version, expiration. Not a permanent approved bit.

- `authority`, `scope` (publisher / deal / event / environment)
- `version`, `issued_at`, `expires_at`
- `identity`
- `dimensions`: `policy`, `technical`, `operational`, `contextual` (contextual may stay on Eligibility; default: separate)
- `lifecycle`: unknown, observed, pending_review, approved, rejected, technically_ready, operationally_ready, expired, revalidation_required
- `environments[]`: per-path results (Worksheet 15). Not one global bit.
- `findings[]`: requirement id, severity, remediable, affected environment / publisher / deal, timestamp
- `operational_claims?` and/or `probe` (rendition hashes, packaging, cache assertion)
- `entry_path`: `proactive` | `auction_triggered`

### Eligibility

Separate from readiness (Worksheet 2, Option B/C). Same creative can be operationally ready and ineligible for this break, publisher, or event.

- `ready_ref`
- `context` (break duration, event, program, competitive separation, pacing, supply path)
- `eligible`: boolean + reasons

### Observation (auction-triggered)

Input when a creative is first seen on a bid, not a registration.

- `creative`, `identity`, `auction_context`
- Does not include our serve/fallback/reject decision. May include the **seller's** declared treatment as data.

### PreparedMedia

Probe or SSAI output bound to identity.

- source hash, rendition hashes, ladder id, packaging, tracker-preservation flag, `mapped_ad_id`

### CapabilitySurface (Option C)

What any ingest/review API must be able to express, AMA or proprietary:

submit or reference; known-creative lookup; validation-context id; requirement discovery; identity/version; multi-authority status; unknown/pending/approved/rejected; structured failure; readiness retrieval; deal/publisher/environment association; update/revalidation; versioning.

AMA is the reference adapter onto this surface, not the only one.

## APIs

```
discover_policy(context) -> PolicyDocument
evaluate(creative, policy, context, operational?) -> ReadinessEvidence
prepare(mezzanine, ladder) -> PreparedMedia + ReadinessEvidence   // reference probe
lookup(identity) -> known | unknown + prior evidence
lookup_fallback(store, auction_context) -> known | unknown   // seller fallback_crid; do not choose treatment
select_overlays(catalog, opportunity) -> PolicyOverlay[]   // deal → publisher → event → device
review_queue(store) -> Evidence[]   // observed | pending_review
diff(old_identity, new_creative) -> invalidation set
adapt_ama(Ad | Audit) -> Evidence | CapabilitySurface fields
observe(Observation) -> Evidence (lifecycle starts)
eligibility(evidence, opportunity) -> Eligibility
prebid_gate(evidence, opportunity?) -> BidGate   // unknown may bid; serve needs ready + eligible
summarize(evidence, opportunity?) -> ReadinessSummary
```

Deterministic given inputs. No network except optional fetch of a mezzanine for `prepare` / hash, behind an explicit flag.

## Worksheet map

Every worksheet has a module object. Final Decision TBD means we ship a default, labeled `provisional`, not "the standard."

| W | Decision | Module default until WG closes |
| --- | --- | --- |
| 1 | What readiness means | Four dimensions. Lifecycle includes unknown/pending, not only the terminal state. |
| 2 | Ready vs eligible | Separate objects. No intrinsic `LIVE_ELIGIBLE`. |
| 3 | Who may assert | Multi-authority. Marketplace may aggregate. Buyer need not know every downstream hop if the aggregator signs. |
| 4 | Delivery environment | Persistent `environment_id`. One platform may expose many. |
| 5 | Identity | Fingerprint = hash + UniversalAdId + URL. System `crid`s are aliases. Media identity ≠ advertising-creative identity. |
| 6 | AMA vs proprietary | Option C. CapabilitySurface + AMA adapter. |
| 7 | AMA v1.2 | Publisher id + deal id on audit. Known-creative lookup as experimental, additive. |
| 8 | AMA 1.x / v2 | Additive fields only. No mandatory publisher-operated API. |
| 9 | Policy discovery | `PolicyDocument` at a URI; overlays on a base; inventory may declare `proactive_required`. |
| 10 | Requirement schema | Categories listed in the draft, as typed fields, not free text. |
| 11 | Feedback / states | `findings[]` + lifecycle enum above. Names illustrative until WG votes. |
| 12 | Auction-triggered | `observe()`. Patterns A–E are seller policy. We record treatment, we do not choose it. High-concurrency inventory may set `proactive_required`. |
| 13 | SSAI operational | Claims + probe: received, accessible, transcoded, renditions, packaged, cache assertion, media identity, trackers preserved, mapped ad id. |
| 14 | SGAI / hybrid | Split `server`, `signaling`, `client_capability`, `execution`. Server-ready ≠ end-to-end-ready. Probe optional. |
| 15 | Multi-path | `environments[]` on evidence. Ready-for-A is not ready-for-B. |
| 16 | Transformation | Ladder encodes inherit technical/operational. Narrative/media replacement mints a new identity. Policy may override. |
| 17 | Distribution / reuse | Hash match reuses PreparedMedia. We verify claims; we do not push to their CDN. |
| 18 | Deal / supply-path | Overlay resolution: deal → policy set → authorities. Deal may forbid auction-triggered. |
| 19 | Agents | MCP over the APIs. No generation. Remediation = structural/probe, not copy. |
| 20 | Last-minute | `diff()`. Tracking-only: keep transcode. Media URL/hash change: invalidate operational + technical. New wrapper: re-evaluate technical. Emergency pull: expire evidence. |

## Draft in-scope map

| Draft in-scope line | Where it lives |
| --- | --- |
| Live CTV and streaming | packs `live-ssai`, `live-sgai`, `live-csai`, `live-hybrid` |
| Programmatic and direct-sold | `transaction` on evaluate; OpenRTB-lite on observe |
| Proactive registration | `evaluate()` entry_path=proactive |
| Auction-triggered discovery | `observe()` |
| Ingest/review interfaces | CapabilitySurface |
| AMA evolution | `adapt_ama`, experimental 1.2 |
| Proprietary interoperability | same surface, non-AMA adapters later |
| SSAI / SGAI / hybrid | packs + W13/W14 fields; `live-hybrid` is SSAI hop plus SGAI identifiers |
| Creative validation | `evaluate` technical + vastlint |
| Creative approval | `dimensions.policy` + lifecycle, not a classifier |
| Operational preparation | claims + `prepare()` |
| Creative distribution | distribution claims + hash reuse |
| Readiness and eligibility signaling | Evidence + Eligibility |
| Unknown / pending | lifecycle |
| Structured feedback | findings |
| Multi-party validation | authorities[] |
| Identity and version | CreativeIdentity |
| Reuse of prior review | `lookup` + transformation_rights |

## Transcoding

In scope as the **ladder, the probe, and claim verification**. Out of scope as a farm.

1. Policy ladder on PolicyDocument
2. `prepare()`: local ffmpeg against that ladder; renditions + hashes on PreparedMedia
3. SSAI/SGAI may pass operational claims; we check them against the ladder and bind identity
4. Reuse via hash (W16/W17/W20)

SGAI: probe optional. Server-ready is a different dimension.

## Surfaces

| Surface | Role |
| --- | --- |
| `creative-readiness-core` | objects, evaluate, lookup, diff, eligibility |
| `creative-readiness-probe` | ffmpeg prepare(), optional feature |
| `creative-readiness-ama` | AMA 1.1 JSON + experimental 1.2 |
| `creative-readiness-cli` | evaluate / prepare / lookup / apply-diff / revalidate / eligibility / transform / observe / ama / surface / gate / summary / select-overlays / queue / authorities |
| `creative-readiness-mcp` | agent tools including observe_creative, select_overlays, review_queue, lookup_fallback, evaluate_authorities |
| `creative-readiness-wasm` | evaluate, lookup, eligibility, gate, summary, observe, select_overlays, queue, evaluate_authorities; no probe |
| `schemas/` | JSON Schema 2020-12. `defs.json` is the shared catalog; object files `$ref` it. `evaluate-request.json` is the Go/Java/MCP evaluate contract. |
| `packs/` | `live-ssai`, `live-sgai`, `live-csai`, `live-hybrid` plus event overlays |
| `docs/pack-authoring.md` | member PolicyDocument authoring (not the IAB standard) |
| `docs/capability-surface.md` | Option C ingest/review surface; AMA is one adapter |
| `fixtures/` | golden evidence for each worksheet default |

vastlint is a dependency for the VAST technical dimension. pixellint is not in this repo (measurement out of scope). Observation accepts an OpenRTB 2.x bid-response fragment (`imp.video.maxduration`, `crid`, `dealid`, publisher id, `ext.fallback_crid`) without taking rtblint as a dep. That parse is not a pacing engine.

License: Apache 2.0. IAB CLA if they take the repo. vastlint stays a separate project. Confirm license and maintainer with Tech Lab before moving the canonical repo under `IABTechLab/`.
