# Option C: AMA as adapter, not the exclusive mechanism

Working-group comment draft. Not a standard. Defaults are provisional until worksheets close.

Creative Readiness is a checkable evidence object (policy, technical, operational, identity, expiry). The Ad Management API 1.1 is one ingest and audit transport that can fill parts of that object.

Recommendation:

- Keep AMA 1.1 as the reference adapter onto a CapabilitySurface. Proprietary ingest APIs implement the same surface.
- Do not require AMA to claim Creative Readiness interoperability.
- Map AdCOM audit status codes onto lifecycle and the policy dimension. Do not treat AMA approval as technical or operational readiness.
- Treat known-creative lookup, publisher id, and deal id as additive AMA 1.2 candidates, not as a v2 rewrite.
- Leave serve / fallback / reject with the seller. The module records the facts; it does not choose disposition.

Reference probe and local identity lookup stay outside AMA. They bind to the same evidence object.
