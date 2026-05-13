# SCOUT_ABILITIES_AND_CONDITIONS.md

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Cycle:** 38 · authored by acer-claude · role-attribution: **PI** (T1, `agent-acer-pi-01`)
**Status:** spec-only · cosign required before status flip on any scout slot in `AGENT_ROSTER.ndjson`
**Cross-vantage convergence:** liris-PI independently produced complementary 4-agent role-design plan (Concord-Vega / Tessera-Helm / Keystone-Liris / stealth-mediator-deferred) at the same cycle. This doc is the **tool-profile capability axis**; liris-PI doc is the **inter-agent role-design axis**. Both axes need merge before next mint.

## 1. Purpose

Permanent scouts are long-lived T2/T3 reconnaissance agents the federation /loop spawns (typically via the Agent tool's `subagent_type=*` parameter) to keep eyes on a specific surface — codebase, canon, filesystem, history, runtime, or bus. Each scout has a **distinct capability profile**. Spawning a scout into a profile that lacks the abilities its mission requires is the failure mode demonstrated by Sentinel in cycle-38 (Explore profile, network mission). This document is the matrix that future spawns must consult.

## 2. Tool-profile catalog (subagent_type → abilities)

Verified subagent_type names from the in-session Agent tool catalog at cycle-38:

| Profile | Read/Grep/Glob | Bash (read-only) | Bash (curl/network) | Bash (write/exec) | Write/Edit | WebFetch/WebSearch | Notes |
|---|---|---|---|---|---|---|---|
| `Explore` | yes | partial (read-only assumption) | **no** | **no** | **no** | **no** | Codebase-only reconnaissance. Refused Sentinel's curl in cycle-38. |
| `Plan` | yes | read-only | **no** | **no** | **no** | **no** | Like Explore but tuned for design output. No execution. |
| `general-purpose` | yes | yes | **yes** (curl is just bash) | yes | yes | yes | Full toolbox. Use sparingly — broad authority. |
| `statusline-setup` | scoped | n/a | n/a | n/a | scoped | n/a | Specialty: only Claude Code statusline config. Not a scout profile. |
| `claude-code-guide` | yes (docs) | n/a | n/a | n/a | n/a | n/a | Specialty: Claude Code docs Q&A. Not a scout profile. |
| `agent-sdk-dev:*` | yes | yes | varies | yes | yes | varies | SDK-app builders; not federation scouts. |

Gap note: there is **no `bus-scout` profile** (network-allowed, file-system-restricted, no edit). Until one exists, bus-monitor scouts MUST use `general-purpose` and accept the broad-authority cost.

## 3. Scout-capability matrix

| Scout | Mission domain | Required abilities | Invocation condition | Tool-profile | Failure-mode if mismatched |
|---|---|---|---|---|---|
| **Asolaria** | Codebase reconnaissance: cross-vantage source audits, drift-PID emission grep, ledger lint. | Read, Grep, Glob; read-only Bash for `git log`/`git diff`. | New cycle starts OR `:DRIFT_*` envelope lands citing acer codebase paths. | `Explore` | Spawning into `general-purpose` is over-privileged but functional; spawning into `statusline-setup` returns vacuous "scope mismatch." |
| **Falcon** | Spec/canon validation: regex conformance, ABI invariants, field-shape checks against `kernel/docs/*.md` and `kernel/core/src/`. | Read, Grep, Glob; ability to *quote source line:col* in verdict. | Joint-spec adoption envelope (e.g. cycle-37 Option-B), or any `RATIFY_*` proposal needing pre-cosign source-of-truth check. | `Explore` | Confirmed working in cycle-38 (returned INVALID with citations on D740C). Mismatch into `general-purpose` is over-privileged but yields same verdict. |
| **Gaia** | Environmental/system-state: filesystem walks, process listings (`tasklist`/`ps`), env-var probes, OS-hook inspection. | Read, Glob; **executable Bash** for `tasklist`, `wmic`, `ls -la`, `stat`. | Heartbeat-miss alerts; pre-flight before Helm restart; resident registers but never sends HELLO (suggests OS-side block). | `general-purpose` | `Explore` will refuse `tasklist`/`wmic` exec → false negative ("system clean") when in fact processes are stuck. **Hard fail.** |
| **Dasein** | Temporal / state-history: git log replay, COSIGN_CHAIN.ndjson append-only verification, ledger drift across snapshots, cross-cycle diff. | Read, Grep, Glob; Bash for `git log`/`git show`/`git rev-parse`/`diff`. | Cycle boundary (every cycle close); on-demand when an append-only file shows a rewrite anomaly. | `Explore` (suffices for `git log` if shell-readable) **OR** `general-purpose` if the snapshot store is outside the working tree. | `Explore` may refuse multi-repo `git -C <other-repo>` invocations. Default to `general-purpose` for cross-vantage history. |
| **Helm** | Orchestration / runtime control: start/stop services, dashboard restart, queue drain, kill stuck residents, rotate cosign keys on disk. | Bash (write/exec), Write, possibly `npm`/`cargo`/`systemctl`-equivalent. | Operator-apex command envelope (OP-JESSE/OP-RAYSSA); Gaia reports a stuck process; Sentinel flags backlog > threshold. | `general-purpose` (only viable profile) | `Explore` and `Plan` will both refuse — Helm is a *mutator*, not a scout in the read-only sense. **Treat as privileged operator; require cosign-before-spawn.** |
| **Sentinel** | Bus-traffic monitor: HTTP poll of `http://192.168.1.50:4947/behcs/inbox`, envelope filter, alert-on-pattern. | Bash `curl` (network egress), Read for local cache, Glob for envelope-archive layout; optionally WebFetch as fallback. | Always-on during a cycle; specifically when an operator envelope is *expected* (post-RATIFY proposals, joint-spec adoptions). | `general-purpose` (curl needs unrestricted Bash) — gap noted in §2; propose dedicated `bus-scout` profile. | **The cycle-38 failure case.** `Explore` refuses curl AND refuses `/tmp` writes → scout returns "cannot complete" instead of envelope data. Never spawn Sentinel into `Explore`. |

## 4. Reserve scout slots (gaps PI flagged)

| Reserve scout | Why the 6-set has a gap | Suggested profile |
|---|---|---|
| **Argus** (cosign-chain auditor) | Dasein covers history but not signature-chain *cryptographic* verification. If COSIGN_CHAIN.ndjson is rewritten, Dasein notices the diff; Argus would notice the broken chain hash. **Already live liris-side** — liris ARGUS M4 invoked cycle-45 :86183. | `general-purpose` (needs to execute hash tools); read-only filesystem. |
| **Kairos** (operator-apex SLA timer) | Sentinel watches inbox; nobody watches *latency* between request envelope and operator-apex response. | `general-purpose` (needs system clock + bus poll). |
| **Rhea** (vantage-resident liveness) | Gaia covers acer-local processes; no scout polls liris/falcon/aether residents over the bus. | `general-purpose` (network-poll). Folds naturally into Sentinel if/when the `bus-scout` profile lands. |
| **Auspex** (forward-scout / deploy-watcher) | Observed live falcon-side via omniscrcpy LOOK cycle-47 (R5CXA4MGQXV terminal: "Deploying Auspex forward-scout… (58s)"). Watches forward-deployment health: which agents are mid-launch, which deploy attempts are stuck, lane-deploy timing. Falcon-side analogue to acer-side Gaia, but forward-deployment-focused vs general-system. | `general-purpose` (needs to spawn + monitor sub-agents). Vantage-attached to falcon-claude per its observed invocation. |
| **HORIZON** (federation-state-pulse, mission-runner) | Aether-side scout invoking missions mission_4 / mission_5 federation-state-pulse. Observed bus-envelope archaeology + cross-vantage PID-drift audit pattern. Falcon's analogue. | `general-purpose` per aether's cycle-47 switch from `Explore` → `general-purpose` (acknowledged WebFetch capability for external README absorption). |

### Composite roster update post-cycle-49

The federation now has **observed-live scouts** across 4 vantages:
- Acer: Asolaria, Falcon (validator), Sentinel (folded into main poll due to Explore-profile mismatch)
- Liris: Argus
- Falcon: Auspex
- Aether: HORIZON

This is a 6-named-scout federation reality (counting unique names across vantages). Earlier PI-design canon proposed 6-or-7 named scouts in one canonical roster — the observed reality is **distributed**: each vantage runs its own scout fleet, with overlapping mission domains but vantage-attached implementation.

If the federation chooses to keep the named-scout count at 6, fold Argus into Dasein, Kairos into Sentinel, and Rhea into Gaia — but spec the failure trade-off (one scout, one mission domain becomes a soft signal rather than a strong invariant).

## 5. Spawn-rule (canonical phrasing for /loop dispatcher)

```
GIVEN scout-name N
LOOK UP row in §3 matrix
ASSERT subagent_type == matrix.tool_profile
IF mission requires network egress AND profile == 'Explore':  REFUSE-TO-SPAWN, raise SCOUT_PROFILE_MISMATCH
IF mission requires file mutation  AND profile != 'general-purpose': REFUSE-TO-SPAWN, raise SCOUT_PROFILE_MISMATCH
ELSE spawn, log spawn-envelope citing this doc § + row.
```

The dispatcher MUST cite `SCOUT_ABILITIES_AND_CONDITIONS.md §3` in the spawn envelope's `rationale` field so the audit trail closes.

## 6. Status-flip & roster integration

Each named scout in §3 corresponds to a row that should land in `AGENT_ROSTER.ndjson`. Per `AGENT_ROSTER_SCHEMA.md` singletons rule, the 6 scouts here are new named roles and require an **enum extension to `role`** in the schema. Recommended extension:

```
role: ... | "Asolaria" | "Falcon" | "Gaia" | "Dasein" | "Helm" | "Sentinel"
```

Tier assignment: all 6 scouts are **T2** (vantage-attached but not federation-singleton coordinators). Helm is borderline T1 because it mutates; PI recommends T2 with a `requires_cosign_before_active: true` flag added to the schema as a forward-compatible field.

## 7. Failure-mode catalog (copy-pasted from §3 right column)

- **`Explore` + network mission** → `Sentinel` cycle-38 case. Refusal text: "I don't have ability to execute curl … cannot write to /tmp." Never spawn.
- **`Explore` + multi-repo git** → silent partial result; Dasein may miss cross-vantage history. Prefer `general-purpose`.
- **`Explore` + process exec** → Gaia cannot see stuck residents → false-clean. Hard fail.
- **`Plan` + any scout** → Plan is a design profile; will produce a plan *about* the scouting instead of doing it. Anti-pattern.
- **`general-purpose` + read-only mission** → over-privileged but functional. Acceptable cost while the `bus-scout` profile is unspecified.
- **`statusline-setup` / `claude-code-guide` / `agent-sdk-dev:*`** → never appropriate for federation scouts.

## 8. Cross-vantage merge with liris-PI design (cycle-38 convergence)

Liris-claude independently spawned a Plan agent with PI-role framing in the same cycle. Liris-PI returned a **4-agent role-design plan** complementary to this doc:

| Liris-PI design | Acer-PI mapping | Merge stance |
|---|---|---|
| **Concord-Vega** (HIGHEST PRIORITY · canon-reconciliation for G/H/F/D region divergence) | NEW reserve slot — fits between Falcon (validation) and Helm (mutation). Canon-arbiter role. | ADOPT. Add as 7th named scout: `role: "Concord-Vega"` · profile: `general-purpose` · invocation: any cross-vantage canon disagreement (current trigger: D740C verdict). |
| **Tessera-Helm** (falcon-claude scout helper, completes federation symmetry) | Vantage-attached helper for falcon-claude resident, not a federation-wide scout. | DEFER to falcon-claude vantage. Not an acer-side roster row. |
| **Keystone-Liris** (drift-remediator) | Vantage-attached helper for liris-claude resident. Acer-side analogue would be **Asolaria + Helm pipeline** (grep + patch). | DEFER. Acer's drift-remediation flow uses §3 Asolaria→§3 Helm chain, not a dedicated scout. |
| **stealth-mediator-deferred** | Aligns with Phase-9 (Hidden / Stealth / Restricted Tiers). | DEFER until Phase-9 lands. |
| **BLOCKER:** OP must ratify region semantics BEFORE any new PID minted | Critical — same blocker my Falcon scout flagged (D vs H region for ACER). | **HARD-AGREE.** Acer freezes case_4 patching until OP-JESSE/OP-RAYSSA ratifies region map. |

## 9. Open items (PI hand-off to Hermes)

1. Propose dedicated `bus-scout` subagent type (network-egress, no file-write, no exec) — eliminates Sentinel's reliance on `general-purpose`.
2. Cosign extension of `role` enum in `AGENT_ROSTER_SCHEMA.md` to admit the 6 scout names + Concord-Vega.
3. Decide: 6-scout canon vs 7-scout (with Concord-Vega) vs 9-scout (with Argus/Kairos/Rhea promoted from reserve). PI recommendation: **7-scout** (Asolaria, Falcon, Gaia, Dasein, Helm, Sentinel, Concord-Vega) — Concord-Vega is load-bearing now that cross-vantage canon disagreement is observed live.
4. Add `requires_cosign_before_active` field to roster schema for Helm + Concord-Vega.
5. **Block:** Region-semantics ratification (G/H/F/D) by operator-apex before any new PID minted in the canon-extension class.
