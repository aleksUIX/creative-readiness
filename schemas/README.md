# Schemas

JSON Schema 2020-12 for every interchange object. Shared types live in `defs.json`. Object files `$ref` those `$defs` by `$id`.

Members can validate with any 2020-12 implementation if they resolve `https://creative-readiness.dev/schemas/*.json` to this directory.

## Catalog

Shared:

- `defs.json` (dimension, lifecycle, finding, identity, facts, claims, ladder, overlay, opportunity, auction context)

Outputs:

- `readiness-evidence.json`
- `eligibility.json`
- `bid-gate.json`
- `readiness-summary.json`
- `environment-result.json`
- `diff-result.json`
- `finding.json`
- `media-facts.json`
- `capability-surface.json`

Inputs:

- `policy-document.json`
- `policy-overlay.json`
- `transcode-ladder.json`
- `observation.json`
- `opportunity.json`
- `operational-claims.json`
- `prepared-media.json`
- `environment-spec.json`
- `openrtb-lite.json` (extra fields allowed)
- `evaluate-request.json` (MCP/CLI/Go evaluate contract)
- `authority-spec.json` (multi-authority evaluate)

Packs in `packs/` are `policy-document.json` instances. Builtin versions in this tree: `live-ssai`, `live-sgai`, `live-csai`, and `live-hybrid` are `0.2.11` (AdServingId required; Mezzanine URLs must be HTTPS). How to write a pack: [docs/pack-authoring.md](../docs/pack-authoring.md). Option C surface: [docs/capability-surface.md](../docs/capability-surface.md).

Identity and authority:

- `creative-identity.json`
- `validation-authority.json`
- `delivery-environment.json`
- `auction-context.json`
