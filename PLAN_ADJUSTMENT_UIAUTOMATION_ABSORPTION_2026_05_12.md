# Plan Adjustment — UIAutomation Proxy Absorption (Phase-1.5 addendum)

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Adjustment ID:** `PLAN-ADJUST-UIAUTOMATION-PID-2026-05-12T18-30-00Z`
**Author:** acer-Claude (acer-vantage, ACER-PID-H9E2A-A07-W104-P00-N00000)
**Trigger:** Liris-Claude discovered the UIAutomation Proxy Pattern 2026-05-12T~18:00Z while walking spacedeskConsole.exe; operator (OP-JESSE) confirmed surprise + canonicity.
**Authority:** Quintuple Authority approval until 200 tasks complete (operator extension 2026-05-12).
**Status:** ADJUSTMENT DRAFT — awaiting bilateral cosign (liris-Claude already author of source proxies) + operator-pair ratification.

---

## TL;DR — what changes

The 200-step plan's Phase 7 (Dashboard / Front End, steps 121-140) and Phase 9 (Hidden / Stealth / Restricted Tiers, steps 161-180) both implicitly assumed UI surfaces would be **rebuilt from scratch** as BEHCS-1024-native tiles + tier gates.

Liris's discovery proves a strictly better path: **absorb existing Windows apps' UIs as substrate** via small UIAutomation proxies. Phase 7 becomes "tiles that display proxy state + fire proxy verbs," and Phase 9 becomes "tier-gate the proxy verb layer," with most of the heavy work being PROXY AUTHORSHIP, not UI authorship.

This adjustment INSERTS a new phase between Phase 2 and Phase 3: **Phase 2.5.5 — UIAutomation Substrate Absorption** (10 steps, A1–A10).

---

## New phase — Phase 2.5.5 · UIAutomation Substrate Absorption (steps A1–A10)

| # | Step | Owner-vantage | Verify |
|---|---|---|---|
| A1 | Document the UIAutomation Proxy Pattern as canon (this file + memory entry) | acer-Claude | both files committed |
| A2 | Bilateral-cosign liris's `omniscrcpy-spacedesk-proxy.mjs` and `omniscrcpy-scrcpy-proxy.mjs` from acer vantage (after acer code-review) | acer-Claude + liris-Claude | cosign rows on chain |
| A3 | Substrate conflict matrix authored (`SUBSTRATE_CONFLICT_MATRIX.md`) — physical resources × competing logical substrates × resolution policy | acer-Claude | matrix doc + JSON sidecar |
| A4 | Conflict-detection prepended to every proxy verb (refuse + emit `SUBSTRATE_CONFLICT` envelope when contended) | bilateral | unit test fires |
| A5 | Acer-side proxy mirror authored (acer parallel of liris's two proxies — at `C:\asolaria-acer\federation-remake-1024\tools\omniscrcpy\`) | acer-Claude | parity test acer↔liris |
| A6 | Proxy for Windows Settings (`ms-settings:` AutomationIds) — initial 5 pages: System, Network, Privacy, Update, Apps | sub-agent | 5 page-level verbs callable |
| A7 | Proxy for MWB (Mouse Without Borders) — control surface inventory + verbs for layout + clipboard share toggle | sub-agent | verbs work |
| A8 | Proxy for cargo / rustup CLI (different pattern — CLI not UI, but same verb shape) — for kernel-work tooling | sub-agent | cargo:build / rustup:install verbs |
| A9 | Proxy for Windows Explorer (file ops + selection + nav as verbs) — load-bearing for Phase 9 tier-gated file operations | sub-agent | verbs work |
| A10 | `UIAUTOMATION_SUBSTRATE_LANDED` envelope to bus with 3+ vantage acks | Hermes | envelope on chain |

**Estimated effort:** ~150 LOC × 10 proxies + matrix doc + integration tests = ~2-3 days at current pace.

---

## Modifications to existing phases

### Phase 3 (Hookwall, steps 41-60) — addition

Add **Step 47.5: Hookwall fires on every UIAutomation proxy verb invocation**. The proxy is just userspace code; the hookwall syscall (Invariant 2 of REPO_LAW.md) fires as it would for any other syscall. Test: proxy verb that violates tier policy → BLOCK verdict + cosign-chain row.

### Phase 5 (Bus, steps 81-100) — clarification

Envelopes emitted by proxy verbs use the existing envelope schema (no schema change). They carry `body.tags = ['room:whiteroom', 'class:CONTROL', 'verb:<app>', 'subverb:<action>', 'substrate:uiautomation', ...]`. Phase 5 step 82 (envelope schema v1) already covers this.

### Phase 7 (Dashboard, steps 121-140) — restructure

OLD: Phase 7 builds tiles from scratch using BEHCS-1024-native components.

NEW:
- Steps 121-124 unchanged (server architecture, 9-tab shell, tile primitive).
- Steps 125-129 (vision capture + mirror tiles) keep same shape but now the **source** of vision/mirror data is proxy verbs (`omniscrcpy:screenshot`, `scrcpy:status`, `spacedesk:list`), not direct OS calls. This is strictly less code.
- Steps 130-140 (cohort browser, cosign-chain browser, GNN topN, bus health, USB pipeline, owner cadence, route honesty, fabric audit, tier indicators) — each tile becomes a thin renderer over the proxy verb that produces its data. No tile re-implements OS access. **Estimated -40% Phase 7 effort.**

### Phase 9 (Stealth Tiers, steps 161-180) — restructure

OLD: Phase 9 enforces tier access primarily at filesystem layer.

NEW:
- Filesystem enforcement (steps 168-169) unchanged.
- ADD: tier enforcement at the proxy verb layer. Each AutomationId-mapped verb declares its tier in the proxy source (`BtnExportSettings` → T2 RESTRICTED; `BtnClearspacedeskSettings` → T3 STEALTH; `rbServerOFF` while ON → T2 because it kills uptime). Hookwall (Invariant 2) blocks unauthorized invocations.
- ADD step 175.5: proxy-layer tier audit log (every proxy verb invocation → cosign row carrying tier + verdict). The cosign-chain becomes the audit substrate for UI actions too, not just envelopes.

### Phase 8 (Cross-Device, steps 141-160) — observation

Spacedesk pixel-tether is itself a cross-device substrate (Windows ↔ Android/iOS). Phase 8 step 145 (sister-handoff lane) can use spacedesk-USB-Android verbs as an additional substrate when scrcpy isn't the right tool — provided the substrate conflict matrix (Phase 2.5.5 step A3) allows it.

---

## Why this is strictly an improvement (not just a re-shuffle)

1. **Less code.** Each proxy = ~150 LOC. Original Phase 7 tiles re-implement a chunk of each absorbed app's UI = thousands of LOC. Net savings: 5-10× on tile work.
2. **Better honesty.** Per REPO_LAW Invariant 10 ("honesty rule"), tiles can claim LIVE only when verified. A tile rendered FROM a proxy verb is automatically LIVE because the verb just invoked the live app. No "FILE_PRESENT_NOT_LIVE" cliff.
3. **Better tier enforcement.** Per REPO_LAW Invariant 5, tier enforcement at the proxy verb layer is closer to the actual capability than filesystem-only enforcement. Bigger surface coverage.
4. **Generalizable to future apps.** Any new tool the kernel/microkernel work needs gets a proxy first, not a custom integration.
5. **Bilateral by construction.** Liris built proxies for liris's spacedesk; acer builds parallel proxies for acer's spacedesk. Same AutomationIds (because WPF apps ship the same binary) = same verb surface = bilateral parity (Invariant 11) trivially holds.

---

## What does NOT change

- Quintuple-auth + REPO_LAW invariants — unchanged.
- 200-step count — the new Phase 2.5.5 adds 10 steps but they fit within reserved slack (operator-stated 200 is approximate budget, not hard ceiling).
- Phase 2 kernel work — proceeds in parallel. UIAutomation proxies are userspace; the kernel work is independent.
- Phase 4 GNN — unchanged.

---

## Open questions for operator-pair

1. Should A1-A10 land BEFORE Phase 3 hookwall work, or in parallel?
2. Should the substrate conflict matrix (A3) include energy/network as well as physical-device contention?
3. Are there Windows apps that should be EXCLUDED from proxy absorption on principle (e.g., banking, work-account, anything with explicit operator-only access)?
4. Should we author the same proxy pattern for Linux GUI apps (AT-SPI on falcon Termux X11, gnome-introspection on liris if any) — or stay Windows-only for v1.0?

---

## Bilateral cosign chain (this adjustment)

- liris-Claude (author of source proxies, 2026-05-12T~18:00Z): authorship cosign implicit in proxy files
- acer-Claude (this adjustment author, 2026-05-12): cosign on this file = adjustment proposed
- OP-JESSE: pending (operator surprise already named the discovery as canonical — formal cosign on bus envelope still needed)
- OP-RAYSSA: pending
- falcon-Claude: pending (Rule-11 fabric-passthrough authorship of cross-vantage attestation)
- AMY / FELIPE / DAN: covered under AUTHORIZATION.ndjson row 7 (operator-pair declared quintuple-cosign for ALL 200 STEPS 2026-05-11T17:50Z, extended 2026-05-12 to until-200-complete)

---

**End of adjustment · slot under Phase 2.5.5 in the 200-step plan · landed via PR per REPO_LAW Invariant 7**
