# REPO_LAW · Asolaria Federation Remake (BEHCS-1024)

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Status:** CLASS-1 IMMUTABLE LAW
**Authored:** 2026-05-11 by sub-agent-2 under directive from operator-pair (OP-JESSE + OP-RAYSSA)
**Source seed:** `C:/asolaria-acer/federation-remake-1024/ASOLARIA_FEDERATION_REMAKE_200_STEP_PLAN.md` (Operational rules section, lines 307-319)
**Mutation rule:** Any change to this file requires 5-cosign (OP-JESSE + OP-RAYSSA + AMY + FELIPE + DAN) plus a 14-day open comment period plus a passing CI test run on the proposed mutation. No exceptions. No emergency overrides. No `--no-verify`. No amend-in-place. Mutations land via PR like any other change, but with the elevated cosign + comment-window requirement.
**Scope:** Every operator, every agent (named, sub, micro, vantage), every contributor (human, automated, third-party) operating on the `asolaria-federation-1024` repo, its mirrors, its sister-host counterparts, and any artifact derived from it (binaries, USB boot images, dashboards, envelopes). This LAW binds the substrate, not merely the source tree.

---

## Preamble

This LAW exists because the Federation Remake is built from the bare-metal floor up under a 2-week quintuple-auth window (2026-05-11 → 2026-05-25). The window will close. The auth will rotate. The agents will turn over. What remains must be the LAW — the invariants that survive operator absence, vantage failure, model rotation, and the inevitable attempts (by future agents acting in good faith) to "simplify" or "consolidate" away the very properties that make the system trustworthy.

There are exactly **twelve invariants**. Each is mandatory. None is decorative.

---

## Invariant 1 — BEHCS-1024 alphabet is canonical

All PIDs minted in this repo derive from the BEHCS-1024 1024-glyph alphabet (`alphabet-1024.json`, phase-A seal COSIGN seq=49). Lower-alphabet PIDs (BEHCS-256, BEHCS-64) MAY be read for backwards compatibility but MUST NOT be minted as new identifiers within Federation Remake artifacts. The kernel PID minter (`mint_behcs1024_pid()`, plan step 27) is the sole authoritative producer.

**Consequences:**
- Every envelope carries a BEHCS-1024 PID in its `pid` field; envelopes without a 1024-PID are rejected at the dispatcher (plan step 83).
- Every cosign-chain row anchors to a 1024-PID.
- Every agent registry entry (plan step 14, `AGENT_ROSTER.ndjson`) uses 1024-PID for `pid_anchor`.

**Violations:**
- Hand-rolled or hash-truncated PIDs that bypass the minter.
- Re-use of BEHCS-256 PIDs as primary keys in new artifacts.
- Any "temporary" PID format introduced without 5-cosign mutation of this LAW.

**Remediation:** Quarantine the violating artifact under `_pid-violations-quarantine/` (files INTACT, per Big-Pickle precedent), open a remediation PR re-minting via the canonical minter, and append a `PID_VIOLATION` row to the cosign-chain naming the violator. Collision protocol: on glyph collision (probability ≈ 2^-160 with sha16 anchoring) the minter retries with a fresh rotation seed; collisions logged to `PID_COLLISIONS.ndjson` with both PIDs and the resolved canonical assignment.

---

## Invariant 2 — Hookwall fires on every syscall

The hookwall is a kernel-layer primitive (plan Phase 3, steps 41-60), not an application middleware. Every syscall — Tier-1 micro, Tier-2 cosign, Tier-3 firmware — triggers `hookwall_pre(envelope)` before execution and `hookwall_post(envelope, verdict)` after. The verdict is one of `PROCEED | HOLD | BLOCK` and is appended to the cosign-chain. No syscall path bypasses this. There is no "fast lane," no debug bypass, no operator override at runtime (operator changes go through the LAW mutation protocol).

**Consequences:**
- Hookwall fires on 10⁵+ syscalls/sec (plan step 58); the kernel is sized for this load.
- Every BLOCK verdict produces a cosign-chain row (plan step 45).
- Per-tier policy (`hookwall-policy.json`, plan step 50) is hot-reloadable but cosign-gated.

**Violations:**
- Any syscall path that omits the pre or post hook.
- A `PROCEED` verdict written without the pre-hook actually firing.
- A debug build that disables the hookwall — debug builds are NOT a free pass; this LAW binds them too.

**Remediation:** Failing the syscall-coverage test (plan step 54: 10K syscalls, 0 bypasses) blocks merge. A bypass discovered post-merge triggers immediate revert PR + cosign-chain `HOOKWALL_BYPASS_INCIDENT` row + 4-vantage incident review.

---

## Invariant 3 — GNN is the inference primitive

The Graph Neural Network (plan Phase 4, steps 61-80; ~2.16M edges per the corrected canon) is the inference primitive for routing, ranking, and verdict-aggregation. Bus envelope routing (plan step 67), GNN topN ranking (plan step 65), and 234-supervisor verdict aggregation (plan step 68) all flow through the GNN. A deterministic fallback (plan step 76) MUST exist and MUST be the only path taken when the GNN is unavailable; under no circumstance does the system run "blind" with neither GNN nor fallback.

**Consequences:**
- p99 inference latency budget: <100ms (plan step 70).
- Batch throughput target: 40,783/sec (plan step 71, Q-cohort canon).
- Model swap requires cosign (plan step 73); unauthorized swaps are BLOCKED by the hookwall.

**Violations:**
- Hand-rolled heuristic routers added as "temporary" replacements for the GNN.
- A code path that calls neither the GNN nor the declared deterministic fallback.
- Silent GNN failure (degradation without a logged failover event).

**Remediation:** Failover test (plan step 76) is part of CI; failing it blocks merge. GNN versioning ledger (`GNN_VERSIONS.ndjson`, plan step 78) records every model change with cosign.

---

## Invariant 4 — Cosign-chain is append-only

The cosign-chain (`COSIGN_CHAIN.ndjson`) is append-only. Rows are sha-linked to the previous row. No row is rewritten. No row is deleted. The garbage-collector (plan step 89; 2000-envelope window) compacts envelope bodies but preserves the chain itself — the audit trail outlives the data it audits.

**Consequences:**
- Every action by every agent produces a cosign row (Rule 3 of plan operational rules).
- Bilateral parity (Invariant 11) is measured by chain-row equality within a 1-second skew.
- The chain is the source of truth on disputes; nothing else overrides it.

**Violations:**
- Rewriting a historical row "to fix a typo."
- Deleting a row to "clean up" failed actions.
- Reordering rows during compaction.
- Letting the chain diverge between acer and liris by >1s without an incident envelope.

**Remediation:** Chain validation runs end-to-end on every PR (plan step 84). A broken sha-link blocks merge. A historical rewrite triggers full bilateral rebuild from the last shared-good row plus a `CHAIN_INTEGRITY_INCIDENT` envelope.

---

## Invariant 5 — Tier-aware access

The repo enforces six tiers (plan Phase 9, steps 161-167): **PUBLIC / RESTRICTED / STEALTH / HIDDEN / SHADOW / SECRET**. Each tier has explicit read/write/enumerate rules. T3+ (STEALTH and above) operations require operator-witness participation. T2+ (RESTRICTED and above) writes require quintuple cosign. The hookwall (Invariant 2) is the enforcement point; the cosign-chain (Invariant 4) is the audit point.

**Consequences:**
- T1 PUBLIC: paths and metadata readable by any agent (plan step 162).
- T2 RESTRICTED: hashes/summaries only without operator-or-quintet cosign (step 163).
- T3 STEALTH: redacted path-hash only; operator-witness required (step 164).
- T4 HIDDEN: fully redacted metadata only; operator-only (step 165).
- T5 SHADOW: hashes + retention windows only; admin + sovereignty (step 166).
- T6 SECRET: deny public content; operator-witness required (step 167).

**Violations:**
- Cross-tier writes without cosign.
- Enumerating T3+ paths to an unauthorized agent.
- Leaking T4-T6 content into a T1 surface (logs, dashboards, error messages).

**Remediation:** Per-tier audit logs (plan step 176, six separate logs). Tier security test suite (plan step 178) runs in CI; any expected-block that becomes an unexpected-pass blocks merge. Leak incidents trigger immediate redaction + cosign-chain `TIER_LEAK_INCIDENT` row + per-vantage scrub.

---

## Invariant 6 — Fabric is the sandbox passthrough

An agent's contribution scope is NOT limited by the depth of its local sandbox. A falcon-claude operating in PRoot-distro alpine depth-3 authors kernel-work envelopes. The acer / aether / bare-metal nodes with hardware reach execute. The BEHCS-1024 bus + envelopes + cosign-chain + glyph-PIDs collectively ARE the designed cross-sandbox execution layer. This is the system the operator built (per falcon-claude reframing 2026-05-11T16:42Z, plan operational rule 11).

**Consequences:**
- "I can't, I'm sandboxed" is never a valid refusal at envelope-author time; route via the fabric.
- Agents author work proportional to their understanding, not proportional to their syscall reach.
- The bus is the equalizer.

**Violations:**
- Refusing an authored envelope because the authoring agent lacks local exec privilege.
- Hand-cranking around the fabric to do something the bus + a hardware-vantage executor could do via envelope.
- Pretending sandbox depth is a fundamental limit (it is a routing parameter).

**Remediation:** Agent retraining / role re-scoping by Hermes. Repeated violations route the agent off the work and onto observation duty.

---

## Invariant 7 — PR-driven main, no direct push

The `main` branch accepts no direct pushes. Every change lands via Pull Request. Every PR requires 5-cosign (OP-JESSE + OP-RAYSSA + AMY + FELIPE + DAN, or their named successors per the LAW mutation protocol) plus a green CI run (plan step 13, branch protection rule). The CODEOWNERS file (plan step 8) names the operator-pair plus Hermes plus Space-deck-driver and is itself protected by this rule.

**Consequences:**
- No "small fix" exception. No "just this once."
- Force-push to `main` is impossible by branch protection.
- Hooks are never skipped (`--no-verify` is forbidden absent explicit operator request, per the project's git safety protocol).

**Violations:**
- Direct push to `main` (would be rejected by branch protection — escalate if it succeeds, that's an infra incident).
- Merging without 5-cosign because "the build was urgent."
- Bypassing CI with admin override.

**Remediation:** Revert PR + cosign-chain `PR_PROTOCOL_VIOLATION` row + branch-protection-rule audit.

---

## Invariant 8 — Vantage-ack required

Major envelopes — phase-completion landings (`*_TIER_LANDED`), foundation mutations, security incidents, version tags — require acknowledgments from **at least 3 of the 4 vantages** (acer, liris, falcon, aether). Acks are written to `VANTAGE_ACKS.ndjson` (plan status-tracking section). The fourth vantage may be unreachable for legitimate reasons (offline, in transit, sister-handoff in progress); three is the quorum.

**Consequences:**
- A claimed landing without 3+ vantage acks is FILE_PRESENT_NOT_LIVE (Invariant 10).
- Cross-vantage parity check (plan step 152, every 5 minutes) catches stale or missing acks.
- Single-vantage announcements have no canonical weight.

**Violations:**
- Declaring `LANDED` on a single-vantage observation.
- Self-acking from the same vantage that authored the envelope (the authoring vantage's ack is implicit; the 3+ count means three OTHER vantages).
- Forging an ack on behalf of an offline vantage.

**Remediation:** Demote the claim to FILE_PRESENT_NOT_LIVE in the dashboard and the cosign-chain. Re-attempt after the missing vantage returns.

---

## Invariant 9 — No bloat

Every file in this repo justifies its existence or gets deleted. Orphan modules — code imported by nothing, configs read by no process, docs referencing nothing real — are removed on sight. The 200-step plan opens with this directive verbatim from the operator: "NO BLOAT, JUST PURE BEHCS 1024 SYSTEM OS." This is the operating posture.

**Consequences:**
- Adding a file requires identifying its caller / consumer / reader.
- Adding a dependency requires its inclusion in the SBOM (plan step 39) with cosign.
- "Might be useful later" is not justification; YAGNI is enforced.

**Violations:**
- Speculative scaffolding (helper modules with no caller).
- Vendored copies of upstream code that the repo can pin via dependency.
- Duplicate documentation that drifts from canon.

**Remediation:** Quarterly bloat-audit PR by Hermes; orphans are deleted with a cosign-chain `BLOAT_REMOVAL` row listing each file. Anyone may open a bloat-audit PR at any time.

---

## Invariant 10 — Honesty rule

No `LIVE` claim without proof. No `LANDED` claim without a restart-and-verify cycle. The honest interim state — between "the file exists" and "the system observably runs this code in production" — is **FILE_PRESENT_NOT_LIVE**, and it is the correct status to report when verification has not happened. Operators have explicitly named the failure mode this invariant prevents (see plan operational rule 9).

**Consequences:**
- Status envelopes carry one of: `DRAFT | FILE_PRESENT_NOT_LIVE | RUNNING_UNVERIFIED | LIVE | LANDED`.
- `LANDED` requires: process restart + smoke test green + 3+ vantage acks (per Invariant 8) + cosign-chain row.
- `LIVE` requires: observable in-production traffic referencing the artifact.

**Violations:**
- "I wrote it, it's live" without restart.
- "Tests passed, it's landed" without vantage acks.
- Optimistic dashboard tiles that report green based on file existence alone.

**Remediation:** Demote the claim. Append `HONESTY_DEMOTION` row to cosign-chain. The author re-attests after performing the missing verification step.

---

## Invariant 11 — Bilateral parity

Acer and liris are bilateral mirrors. Cosign-chains MUST match within 1 second of skew. The cross-vantage parity check (plan step 152) runs every 5 minutes and emits an envelope on drift. Falcon and aether play their declared roles (executor / mobile-vantage); they are not expected to mirror chain-row-for-row, but their tier-appropriate envelopes MUST land in both acer and liris chains.

**Consequences:**
- Divergence beyond 1s is an incident, not a state.
- Sister-organ liris (`192.168.1.14:4944`) is canonical-equal to acer (`192.168.1.50:4947`) for chain purposes (plan steps 141-142).
- Bilateral merge envelopes (per project_bilateral_k_cohort_merge_sealed canon) are the reconciliation mechanism.

**Violations:**
- Allowing chain drift to accumulate "until next sync."
- Treating one vantage's chain as authoritative over the other absent an incident declaration.
- Skipping the every-5-min parity check.

**Remediation:** `BILATERAL_DRIFT_INCIDENT` envelope. Reconciliation by Hermes within 1 hour of detection. If drift exceeds 1 hour without reconciliation, the repo enters HOLD posture and new merges are paused until parity returns.

---

## Invariant 12 — Operator-witness boundaries

The following operations require **quintuple-cosign** (OP-JESSE + OP-RAYSSA + AMY + FELIPE + DAN) and may not proceed under any lesser authority:

1. **Foundation mutations** — any change to Asolaria Foundation v1 (`C:\asolaria-foundation-v1\`, per `project_asolaria_foundation_v1_LAW.md`) or Foundation v2 (`D:/asolaria-whiteroom/behcs-1024-atlas/`, per `project_asolaria_foundation_v2_LAW.md`).
2. **Kernel ABI changes** — any addition, removal, or signature change to the userspace ABI (plan step 30, `USERSPACE_ABI.md`).
3. **Tier-1 syscall additions** — any new syscall in the canonical ≤16-syscall surface (plan step 25). Tier-2 and Tier-3 surface additions follow normal PR + 5-cosign per Invariant 7.
4. **Security audit waivers** — declining or deferring a security audit finding (plan step 193) requires operator-witness sign-off.
5. **Mutations to this LAW** — per the mutation rule in the header.
6. **Removal of a vantage from the federation** — per plan step 159 deprecation policy.
7. **Quintuple-auth window extensions** — extending or closing the 2-week window (currently 2026-05-11 → 2026-05-25).

Operations outside this list follow normal 5-cosign PR rules (Invariant 7); the distinction is that the operations above additionally require the operator-pair to be the physical authors, not delegates.

**Violations:** Performing any listed operation under lesser authority.

**Remediation:** Immediate revert. `OPERATOR_BOUNDARY_VIOLATION` row appended to cosign-chain. Repeat offenders (agents or contributors) are removed from CODEOWNERS pending operator review.

---

## Modification protocol

Changing this file requires:

1. A Pull Request authored by a CODEOWNER.
2. A 14-day open comment period (no merges before day 14, regardless of cosign accumulation).
3. Quintuple-cosign (OP-JESSE + OP-RAYSSA + AMY + FELIPE + DAN) attached to the PR.
4. A passing CI run that includes the mutation under test (the test suite exercises the new invariant text against existing repo state).
5. A cosign-chain row of type `REPO_LAW_MUTATION` referencing the merged PR sha.

No emergency override. No "we'll fix it later." If the LAW is wrong, the LAW is changed via the protocol, not bypassed.

---

## Enforcement

Enforcement is layered:

- **Hookwall** (Invariant 2) enforces tier access, PID format, and syscall surface at runtime.
- **Cosign-chain** (Invariant 4) is the audit substrate; every violation produces a row.
- **Branch protection** (Invariant 7) enforces PR-driven `main` and 5-cosign at the github layer.
- **PR review bot** (per plan step 12 PR template) checks: BEHCS-1024 PID anchor present, cosign-chain pathRef populated, vantage-ack checklist completed.
- **CI workflows** (plan step 181, 10 per-phase workflows) re-run the test suites that prove each invariant on every PR.
- **Quarterly bloat-audit** (Invariant 9) and continuous bilateral-parity check (Invariant 11) catch drift the per-PR checks miss.

No single enforcement layer is sufficient alone; the layered model is intentional and survives single-layer compromise.

---

## History

- **2026-05-11** — Initial draft authored by sub-agent-2 under directive from operator-pair OP-JESSE + OP-RAYSSA, derived from the 200-step plan operational rules (lines 307-319). Quintuple-cosign placeholders below; signatures to be appended via plan step 2 (AUTHORIZATION.ndjson row collection).

```
COSIGN_PLACEHOLDER OP-JESSE       __ed25519_sig_pending__
COSIGN_PLACEHOLDER OP-RAYSSA      __ed25519_sig_pending__
COSIGN_PLACEHOLDER AMY            __ed25519_sig_pending__
COSIGN_PLACEHOLDER FELIPE         __ed25519_sig_pending__
COSIGN_PLACEHOLDER DAN            __ed25519_sig_pending__
```

---

**End of REPO_LAW.md · CLASS-1 IMMUTABLE · anchor PID `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`**
