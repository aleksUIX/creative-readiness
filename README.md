# Creative Readiness

This repository is a reference implementation of the IAB Tech Lab Advanced TV Live Event Ad Creative Readiness draft v0.02. It is not the IAB standard.

Open-source module that evaluates whether a creative is ready for a named live environment (policy, technical, operational), with identity and evidence that have a scope, a version, and an expiry.

Not an SSP, not an SSAI, not a transcode farm. Members embed the library.

## 0.9.4

```bash
cargo test --workspace
cargo run -p creative-readiness-cli -- evaluate --file fixtures/ok-https-mp4.xml --pack none --overlay fixtures/overlays/catalog.json --event game-1
cargo run -p creative-readiness-cli -- authorities --file fixtures/ok-https-mp4.xml --spec fixtures/authorities/marketplace-publisher.json
```

Review-fix release. `observe` evaluates as auction-triggered and carries deal/publisher overlays. `apply_reuse` will not mark ready when policy failed. Device overlays use the request device, not the overlay selector as a claimed device. Same-media two crids keep the first identity. Missing MediaFile HTTPS and unparseable pack `expires_at` fail closed. Fingerprint is `hash:` or `url:` plus `|uad:` when UniversalAdId is present. Duration and other body edits are not tracking-only. `live-hybrid` records an SGAI split. Probe packaging comes from the ladder (mp4, not hls). MCP `evaluate_creative` selects overlays from the catalog. CLI `--overlay` accepts a catalog array. `authorities` / MCP `evaluate_authorities` keep both sides of a marketplace vs publisher disagreement.

`--policy` loads a PolicyDocument and wins over builtin pack requirements. Pack `expires_at` fails closed. `--deal`, `--publisher`, `--event`, and `--device` select overlays (deal first) and set evidence scope. `--auction-triggered` records auction entry; a deal overlay may forbid it. `select-overlays` prints the matching overlay set without evaluating. `lookup --crid` / `--fallback-crid` are identity lookups; fallback does not choose seller treatment. `queue` lists observed and pending-review rows in a local store.

`--pack live-ssai` (pack 0.2.11) requires mezzanine and UniversalAdId, forbids VPAID, and requires HTTPS, quartile tracking, CACHEBUSTING and TIMESTAMP, declared AVC/AAC, mp4 container, 16:9 size of at least 640x360, Linear duration 6–60 seconds, and a MediaFile bitrate between 1000 and 20000 kbps. `--pack live-sgai` (0.2.11) forbids VPAID and requires UniversalAdId rather than mezzanine, plus the same HTTPS, quartiles, macros, AVC/AAC, mp4, size, duration, and bitrate checks; evidence.sgai splits server / signaling / client / execution. `--pack live-csai` (0.2.11) is the client path with no ladder and the same identifier and media floor as live-sgai. `--pack live-hybrid` (0.2.11) needs mezzanine plus UniversalAdId and that shared media floor. All four packs require AdServingId, declare W20 `invalidation_rules`, and echo `unknown_creative_treatment` as informational on evidence (not as a gate). Empty Mezzanine text fails; an HTTP Mezzanine URL fails `media_https`. `--pack none` is vastlint findings only. A bitrate-only VAST edit is a body change, not tracking-only. `prepare` is a local ffmpeg reference probe, not a transcode farm. Operational claims JSON can be passed to `evaluate --claims` (packaged/cached/trackers when a ladder is present). Probe output can be passed to `evaluate --prepared`. `--store` is a local JSON identity file (same media hash + new crid reuses technical/operational; new media with the same crid does not). `--paths` evaluates one creative across named environments. `--overlay` applies deal/publisher/event/device policy overlays (`event-sports.json`, `event-news.json`, `privacy-macros.json`, `audio-aac.json`, `device-ctv.json`). `diff` uses pack `invalidation_rules` (tracking-only keeps transcode; media change does not). `apply-diff` writes those invalidations onto stored evidence. `revalidate` applies the expiry clock; `--pull` expires immediately. `eligibility` is contextual: a 30s creative can be operationally ready and still miss a 15s break, or miss a deal/event/publisher scope. `gate` is the DSP/SSP cut: unknown may bid; serve needs ready plus eligible. `summary` is the ad-ops view. `transform --kind encode` inherits technical readiness onto new renditions; `--kind media_replacement` mints a new identity. `surface` prints the Option C capability surface. `ama` maps AMA 1.1 Ad/Audit JSON onto evidence; `--experimental` maps publisher/deal ids. `observe` records auction-triggered sightings and echoes seller treatment; it does not choose serve vs fallback. Observation JSON may include `openrtb_json` for an OpenRTB-lite fragment.

MCP tools are listed in [docs/mcp.md](docs/mcp.md). WASM evaluate/lookup/eligibility/gate/summary/observe/select_overlays/queue/evaluate_authorities is `creative-readiness-wasm` (no probe).

## Build and releases

The workspace uses the published `vastlint-core` 0.14.0 crate. A standalone clone can run `cargo test --workspace --locked` and build the CLI and MCP server with `cargo build --release --locked -p creative-readiness-cli -p creative-readiness-mcp`.

Pushing a `vX.Y.Z` tag that matches the workspace version runs the [release workflow](.github/workflows/release.yml). It tests the workspace, builds native Linux, macOS, and Windows binaries, and attaches archives plus SHA-256 checksums to a GitHub prerelease. Each archive includes both binaries, the policy packs, schemas, README, and license.

## Docs

- [PLAN.md](PLAN.md): objects, APIs, worksheet map, in/out of scope
- [ROADMAP.md](ROADMAP.md): milestones `0.1` through `1.0`
- [docs/pack-authoring.md](docs/pack-authoring.md): how members write a PolicyDocument
- [docs/capability-surface.md](docs/capability-surface.md): Option C ingest/review surface (AMA is one adapter)
- [proposal.md](proposal.md): converted working-group draft (v0.02 + worksheets)

## License

Apache License 2.0.
