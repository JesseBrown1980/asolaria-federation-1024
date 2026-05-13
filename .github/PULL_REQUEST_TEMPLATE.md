<!--
Anchor PID: ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11
REPO_LAW invariants apply. Branch protection on `main` requires:
  - 5-cosign quintuple-auth signature OR FULL SYSTEMS auth window active
  - CI green (cargo, node, schema, cosign-chain link, no-bloat)
  - CODEOWNERS approvals per touched paths
-->

## Summary

<!-- 1-3 sentences. Why now. -->

## Plan step(s) implemented or addressed

<!-- e.g. "Phase 1 step 12 (PR template)" or "Phase 2 step 27 (kernel PID minter)" -->

## Cosign-chain pathRef

<!--
Append a new row to COSIGN_CHAIN.ndjson and paste the row sha16 here.
Format: row N, sha16=XXXXXXXX, verb=EVT-<UPPER-KEBAB>
-->
- row: `<n>`
- sha16: `<sha16>`
- verb: `EVT-<UPPER-KEBAB>`

## BEHCS-1024 anchor

- glyph-envelope PID (if applicable): `_______________`
- atlas-cp range touched: `_______________`

## Vantage-ack checklist

Reviewers from each vantage check the box AFTER pulling + verifying locally:

- [ ] acer-vantage ack (operator or acer-Claude)
- [ ] liris-vantage ack (OP-RAYSSA or liris-Claude)
- [ ] falcon-vantage ack (falcon-Claude)
- [ ] aether-vantage ack (aether-Claude)

Multi-vantage changes require ≥3 acks; single-vantage changes require ≥1 ack from the affected vantage.

## REPO_LAW invariants respected

- [ ] BEHCS-1024 native — no regression to BEHCS-256-only paths
- [ ] hookwall fires on every relevant action — no bypass surface added
- [ ] GNN gate consulted where decisions are routed (Phase 4+)
- [ ] no-bloat — no source file >2000 LOC; no premature abstraction; no half-finished features
- [ ] cosign-chain row appended for canon-changing actions
- [ ] honesty (REPO_LAW Invariant 10) — no LIVE claim without proof; demote if claim falsified

## Test plan

- [ ] CI green
- [ ] Local cargo check
- [ ] Local node --check on touched .mjs
- [ ] Manual: <describe>

## Risks / rollback

<!-- What breaks if this lands wrong? How do you reverse it? -->

## Linked issues / decisions

<!-- Closes #N · References FD-XXX in FEDERATION_DECISIONS.md -->
