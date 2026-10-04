# Watchdog

Refine-readiness loop. The old 2h campaign ended. Do not re-arm it. Do not mint `1.0.0`.

**Loop name:** `refine-readiness`
**Wake:** 240s after each turn, or sooner if tests fail

## On each wake

1. Review remaining defects in core, CLI, MCP, wasm, probe, AMA, schemas, packs, and docs.
2. Fix the cluster. If the cluster is thin, deepen one PLAN gap that members would actually embed.
3. `cd` into this directory (the path has a space) and run `cargo test --workspace`.
4. Update this checklist. Re-arm `refine-readiness` unless the user said stop.

## Bounds

Stay inside PLAN/ROADMAP. No transcode farm, CDN, stitcher, hosted graph, AMA v2-as-standard, NIW/EB2/petition comments. Nested git. No remote unless asked. URI fetch stays out (no network except explicit `prepare`).

## Stop

User says stop, remaining work is 1.0 freeze only, or the wall clock is after **2026-09-20T18:10:00-07:00**. Then do not re-arm.

The 18:10 PDT wall passed. Loop PID 35521 and sleeper 41114 are dead. Do not re-arm until asked.

## Checklist

- [x] M0 repo
- [x] M1 / 0.0.1: workspace, evaluate, schema, CLI, two fixtures, `cargo test` (2026-09-19)
- [x] M1 / 0.1.0: identity parse (media_url), pack files on disk, schema validation test, more findings mapping
- [x] M2 policy packs
- [x] M3 probe: `creative-readiness-probe` ffmpeg `prepare()`, claims-only operational evaluate, `--prepared`, stable hashes when ffmpeg is present (0.3.0)
- [x] M4 identity store: fingerprint, local JSON store, same-media two crids inherit, new-media same crid miss (0.4.0)
- [x] M5 AMA: 1.1 Ad/Audit parse, status → lifecycle/policy, spec fixtures, 1.2 publisher/deal behind `--experimental`, Option C note in `docs/wg-note-option-c.md` (0.5.0)
- [x] M6 observe: auction-triggered `observe()`, patterns A–E fixtures, seller treatment echoed, `proactive_required` on live-ssai (0.6.0)
- [x] M7 multi-path: environments[] (SSAI-A ready, SSAI-B not, SGAI server-ready only), deal overlay, aggregator vs publisher conflict keeps both findings (0.7.0)
- [x] M8 CLI/MCP/wasm: `diff` CLI, MCP tools in `docs/mcp.md` + `mcp.json`, wasm evaluate/lookup no probe, readiness_report maps vastlint structural fix (0.8.0)
- [x] M9 diff/expiry: pack `invalidation_rules`, last-minute fixtures, clock + pull fail closed, CLI `revalidate` (0.9.0)
- [x] M9.1 embed deepen: eligibility, W10 media facts, live-csai, SGAI split, transform, apply_diff (0.9.1)
- [x] M9.2 bid gate, hybrid, W10/W13 deepen, OpenRTB-lite observe (0.9.2)
- [x] M9.3 overlay select, pack expiry, fallback lookup, review queue, custom policy (0.9.3)
- [x] M9.4 review-fix: observe auction context, reuse policy, device selector, store first-wins, fail-closed HTTPS/expiry, UAdId fingerprint, body diff, hybrid SGAI, probe packaging, evaluate_authorities, CI (0.9.4)

## Last turn

Stopped: 18:10 PDT wall reached. Resumed on 2026-10-03 for Companion asset and NonLinear expanded maximum size checks. Added four overlay fixtures, schema entries, policy checks, and authoring docs. 244 core tests and the full workspace pass. Still 0.9.4. Next: inspect remaining W10 and docs gaps. Do not mint 1.0.0. Do not re-arm until asked.
