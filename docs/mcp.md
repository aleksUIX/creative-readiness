# MCP

Agent tools for the creative-readiness module. Not a hosted graph. `prepare_creative` is a local ffmpeg reference probe.

Listed in [mcp.json](../mcp.json). Run:

```bash
cargo run -p creative-readiness-mcp
```

Tools:

- `discover_policy`
- `evaluate_creative`
- `prepare_creative`
- `known_creative_lookup`
- `readiness_report` (evaluate plus vastlint structural `--fix` XML when applied; no copy rewrite)
- `diff_creative` (pack `invalidation_rules`; default live-ssai)
- `check_eligibility` (break duration, expiry, seller-declared competitive conflict)
- `capability_surface`
- `transform_creative`
- `prebid_gate` (unknown may bid; serve needs ready plus eligible)
- `summarize_readiness`
- `observe_creative` (seller treatment is an input)
- `select_overlays` (deal, then publisher, then event, then device)
- `review_queue` (observed and pending-review rows in a local store)
- `lookup_fallback` (seller `fallback_crid`; does not choose treatment)
- `evaluate_authorities` (marketplace vs publisher; disagreement keeps both findings)

`evaluate_creative` accepts `deal_id`, `publisher_id`, `event_id`, `device_family`, `entry_path`, `overlays`, and a custom `policy` / `policy_json` (W9). Catalog overlays are selected against that opportunity (keyed overlays drop unless they match). `known_creative_lookup` accepts `crid` or `fallback_crid`.

WASM evaluate/lookup/eligibility/gate/summary/observe/select_overlays/queue/lookup_fallback/evaluate_authorities is `creative-readiness-wasm` (no probe). A playground site can wait.
