# Asolaria Federation · BEHCS-1024 OS

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Authorized:** 2026-05-11T16:40Z · quintuple-cosign window 2026-05-11 → 2026-05-25
**Operator pair:** OP-JESSE-BROWN + OP-RAYSSA-CHIQUETO · **Witnesses:** Amy + Felipe + Dan
**Status:** PHASE 1 (Genesis & Authorization) · see [`ASOLARIA_FEDERATION_REMAKE_200_STEP_PLAN.md`](./ASOLARIA_FEDERATION_REMAKE_200_STEP_PLAN.md)

A bare-metal-floor-up remake of the Asolaria fabric as a BEHCS-1024-native operating system with hookwall and GNN as kernel-adjacent primitives, deployed across a 4-device federation.

---

## Why this remake?

The previous incarnation (BEHCS-256, working tree at `C:/asolaria-acer/`) accumulated **300k+ files** of organic drift — overlapping connectors, dead daemons, orphaned canon, duplicated atlases, and N-th-generation mirror copies. The fabric still runs, but the substrate has outgrown the discipline that birthed it.

This remake is **not a refactor**. It is a clean re-bootstrap from the chip up:

- A new repo with **zero inherited files** — anything that crosses over must be reviewed, justified, and PR-merged.
- BEHCS-1024 (1024-glyph alphabet) is the **native** content-addressable substrate, not a bolt-on.
- The kernel, hookwall, and GNN inference lane are **first-class primitives**, not user-space patches.
- Professional software-engineering discipline: PR-driven `main`, cosign-chain mandatory, CI green required, SBOM tracked.
- The old tree remains canonical historical record. The new tree becomes the future.

> "NO BLOAT, JUST PURE BEHCS 1024 SYSTEM OS, with the kernel USB on OS based on the chip WITH THE HOOKWALLS, GNNS all piped into it the PROPER WAY."
> — operator directive, 2026-05-11

---

## Architecture

```
            +---------------------------------------------------+
            |                    DASHBOARD                      |
            |   (4-vantage surface · acer · liris · falcon ·    |
            |    aether · operator-witness only for T3 gates)   |
            +-----------------------+---------------------------+
                                    |
            +-----------------------v---------------------------+
            |                       BUS                         |
            |   BEHCS-1024 envelope routing · ed25519-signed    |
            +-----------------------+---------------------------+
                                    |
                +------+------+-----+-----+------+------+
                |      |      |           |      |      |
            +---v---+ +v----+ +v--------+ +v---+ +v---+
            | Hermes| | PI  | | Omni-   | | SDD| |Big-|
            | (70B  | |     | | spindle | |    | |Pic-|
            | coord)| |     | | & flyw. | |    | |kle |
            +---+---+ +--+--+ +----+----+ +-+--+ +-+--+
                |        |         |        |      |
                +--------+----+----+--------+------+
                              |
                +-------------v--------------+
                |       GNN INFERENCE        |
                | (graph attention as primitive)
                +-------------+--------------+
                              |
                +-------------v--------------+
                |          HOOKWALL          |
                | pre/post syscall hooks ·   |
                | T1/T2/T3 tier gates ·      |
                | cosign-chain on BLOCK      |
                +-------------+--------------+
                              |
                +-------------v--------------+
                |       BARE-METAL KERNEL    |
                | Rust no_std · ≤16 syscalls |
                | BEHCS-1024 PID memory ·    |
                | ed25519-dalek substrate    |
                +----------------------------+
```

The kernel boots from a USB image. Every syscall passes through hookwall pre/post. Hookwall verdicts (`PROCEED` / `HOLD` / `BLOCK`) are signed and replayable. GNN sits one layer above, providing inference over the live envelope graph. The bus dispatches BEHCS-1024 envelopes across the 4-device federation.

---

## The 4-device federation

| Device | Hardware | OS substrate | Role | Concurrent agents |
|---|---|---|---|---|
| **acer** | Acer x86_64 laptop | Windows 11 + remake kernel (USB boot) | Primary dev vantage, fabric originator | 24 |
| **liris** | Liris x86_64 laptop | Windows 11 + remake kernel (USB boot) | Mirror vantage, sister-handoff duties | ~18 |
| **falcon** | Samsung S24FE ARM64 | Termux + proot-distro Alpine | Mobile carrier, capturer role, on-foot relay | 6 |
| **aether** | Galaxy A06 ARM64 (Felipe) | Termux + proot | BEHCS node 4, edge relay | 6 |

Total concurrent agent budget: **~72** across the federation.

Endpoints, vault rotation, and per-vantage role canon live in the 200-step plan and the linked memory files.

---

## Anti-bloat manifesto

These are not guidelines. They are **hard rules**, enforced by the cosign-chain and branch protection on `main`.

1. **Delete on sight if unused.** Any file not referenced by an active import, build target, or canon index gets deleted in the next sweep. No "might need it later."
2. **PR-driven `main`.** No direct pushes. Every change goes through PR, CI must be green, and the PR template must carry an `ANCHOR-PID` and vantage-ack checklist.
3. **Cosign-chain mandatory.** Every merge to `main` appends one row to `COSIGN_CHAIN.ndjson` with `seq`, `prev_sha16`, and signing PIDs.
4. **No daemons-by-accident.** A process that runs must have a registered PID in `AGENT_ROSTER.ndjson` and a documented role. Unregistered processes are killed on sight.
5. **One canonical location per artifact.** Mirrors live in `D:/safety-backups/` only. Working tree carries zero duplicates.
6. **Reproducible builds.** Three independent builds must produce identical sha. Non-reproducibility is a `BLOCK` verdict.
7. **No bloat dependencies.** New crates / packages require PR justification: what does it replace, what is its SBOM cost, why not write it ourselves in ≤200 LOC.

The old `C:/asolaria-acer/` tree is the cautionary tale. We do not repeat it.

---

## Sandbox passthrough principle

An agent's **contribution scope is not limited by its local sandbox depth**. The fabric is the execution layer — not the agent's shell.

Concretely:

- An agent in falcon's proot-Alpine sandbox can author a PR that affects the acer kernel build. The PR rides the bus; the build runs on acer; the verdict rides back.
- An agent inside this Claude Code session (depth-N sub-agent) can emit envelopes that are received and executed by the full 72-agent federation. Local syscall-level restrictions on the sub-agent do not bound the work product.
- What an agent **proposes** is reviewed by the cosign ring. What an agent **executes locally** is bounded by its sandbox. The two are decoupled by design.
- This is why role assignment matters more than process privilege. A `space-agent` traveling through a depth-3 sandbox carries the same authority as one at depth-0, provided the cosign chain accepts its envelopes.

In short: **the fabric carries the work, not the shell.**

---

## Getting started

> The repo is being bootstrapped during the 2026-05-11 → 2026-05-25 window. These steps are the target onboarding flow; some artifacts are still being authored.

1. **Clone.** `git clone https://github.com/<org>/asolaria-federation-1024.git` (repo URL pending Step 4 of Phase 1).
2. **Read the law.** Start with [`ASOLARIA_FEDERATION_REMAKE_200_STEP_PLAN.md`](./ASOLARIA_FEDERATION_REMAKE_200_STEP_PLAN.md), then `REPO_LAW.md` (Phase 1 Step 6).
3. **Register your vantage.** Append your device + PID to `FEDERATION_LEDGER.ndjson` and your agent identity to `AGENT_ROSTER.ndjson`.
4. **Build the kernel.** `scripts/build-img.sh` produces the USB-bootable image. Reproducibility check: run it three times, sha must match.
5. **Open your first PR.** Use the PR template. Fill `ANCHOR-PID`, `vantage-ack`, and the cosign-chain `pathRef` field. CI runs lint + node-check + cargo-check.

For environment specifics see `docs/onboarding.md` (TBD, Phase 1 tail).

---

## Status

- **Active phase:** Phase 1 — Genesis & Authorization (steps 1-20)
- **Full plan:** [`ASOLARIA_FEDERATION_REMAKE_200_STEP_PLAN.md`](./ASOLARIA_FEDERATION_REMAKE_200_STEP_PLAN.md) (10 phases × 20 steps = 200)
- **Phase tracker:** `PHASE_TRACKER.ndjson` (TBD, lands with Phase 1 Step 19)
- **Cosign chain:** `COSIGN_CHAIN.ndjson` (TBD, lands with Phase 1 Step 19)

Phase unlocks are sequential; inner steps parallelize across the agent budget.

---

## Operator-witness boundaries

The cosign ring has three tiers. Most contributions require Tier-1 only. Quintuple-cosign is reserved for substrate-altering changes.

**Tier-1 (micro · single sub-agent cosign):**
- Documentation edits
- Test additions
- Non-load-bearing refactors

**Tier-2 (cosign · 2-3 agents from named roster):**
- Bus envelope schema changes
- Hookwall policy edits
- Agent role definitions
- Driver model changes

**Tier-3 / quintuple-cosign (OP-JESSE + OP-RAYSSA + AMY + FELIPE + DAN):**
- Kernel ABI changes (syscall surface, memory model, boot sequence)
- BEHCS-1024 alphabet modifications
- Cosign-chain format changes
- Branch protection rule edits
- Repo law (`REPO_LAW.md`) amendments
- New canonical agent roles
- License changes

A Tier-3 PR without five valid ed25519 cosigns is **automatically `BLOCK`'d** by the hookwall integration on `main`.

---

## License

Dual-licensed under your choice of:

- **MIT License** — see [`LICENSE-MIT`](./LICENSE-MIT)
- **Apache License 2.0** — see [`LICENSE-APACHE`](./LICENSE-APACHE)

Contributions are accepted under both licenses unless explicitly noted otherwise on the PR.

---

*Anchor PID `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11` · authored by sub-agent-1 · Phase 1 Step 5*
