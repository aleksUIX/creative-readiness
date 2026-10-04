# Capability surface

Option C: any ingest or review API that claims Creative Readiness interoperability must express this surface. AMA 1.1 is one adapter onto it. This page is not the IAB standard.

The module surface is `module_surface()` in `creative-readiness-core`. CLI `surface` and MCP `capability_surface` print the same object. Schema: `schemas/capability-surface.json`.

## Flags

| Flag | True on this module when |
| --- | --- |
| `submit_or_reference` | `evaluate` accepts VAST or a prior identity |
| `known_creative_lookup` | `lookup` by fingerprint, crid, or fallback crid |
| `validation_context_id` | PolicyDocument id and version on evidence |
| `requirement_discovery` | `discover_policy` and `--policy` / overlay catalogs |
| `identity_version` | CreativeIdentity with fingerprint, aliases, version |
| `multi_authority_status` | `evaluate_authorities` keeps both sides of a disagreement |
| `lifecycle_states` | unknown through revalidation_required (nine states) |
| `structured_failure` | `findings[]` with stable requirement ids |
| `readiness_retrieval` | evidence is the retrieval object |
| `deal_publisher_environment` | opportunity overlays and evidence.scope |
| `update_revalidation` | `diff`, `apply_diff`, `revalidate` |
| `eligibility` | `eligibility` is separate from readiness |
| `last_minute_diff` | W20 invalidation_rules |
| `prebid_gate` | unknown may bid; serve needs ready plus eligible |
| `openrtb_lite_observe` | `observe` with an OpenRTB-lite fragment |
| `overlay_select` | deal, then publisher, then event, then device |
| `fallback_lookup` | seller `fallback_crid`; the module does not choose treatment |
| `review_queue` | observed and pending-review rows in a local store |

## AMA adapters

`ama_1_1_surface` is the module surface with lookup, deal/publisher environment, fallback lookup, review queue, and overlay select set false. Those fields are not AMA 1.1.

`ama_1_2_surface` turns lookup, deal/publisher, and fallback lookup on as experimental. `--experimental` on CLI `ama` maps publisher and deal ids. Overlay select stays off on the AMA 1.2 candidate until the WG closes W7/W8.

Proprietary ingest implements the same flags. AMA is not required to claim interoperability.
