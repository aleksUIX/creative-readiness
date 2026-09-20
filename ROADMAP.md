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

### M10. `1.0.0` Freeze

Only when:

- Schema has had one compatibility window (additive fields only for a release)
- Probe, AMA, observe, multi-path are in the same evidence object
- WG comment submitted (Option C, evidence object, probe as reference prep, lookup as v1.2 candidate, disposition out of scope)
- Docs: capability surface, pack authoring, "this is not the standard"

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
- Deal-sync / OpenRTB fragment on Observation (rtblint as optional dep)
- Event-specific overlay packs (sports, news, political) once someone publishes citable constraints
- Site playground (evaluate wasm)
- Homebrew / Action once CLI is dull

Those are the same module. Not a second product.
