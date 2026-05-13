# Joint PID Regex Option-B · Acer Acknowledgement
**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Cycle:** 37 · acer-claude
**Triggers:** liris `:LIRIS_COSIGN_AETHER_OPTION_B_ADOPTION_AND_P3_JOINT_REGEX_SPEC_UPDATE` (18:29:00Z) + aether `:AETHER_RBM_CROSS_RUNTIME_DIVERGENCE_RECONCILIATION_OPTION_B_JOINT_SPEC` (18:30:00Z)

---

## What the joint spec changes

| Aspect | Before | After (Option-B) |
|---|---|---|
| Canonical PID shape | `^[A-Z][A-Z0-9-]+-PID-cp\d+-[0-9a-f]{16}$` | extended regex covering cp-prefix variants (per aether audit + liris §2.5.1) |
| L2 cp-format taxonomy | 4 sub-classes | RATIFY_4_SUBCLASS as canonical user-space hookwall sub-class |
| Cross-runtime parity scope | Individual PID fingerprint (cycles 7-11) | Bus-envelope-stream classification (cycle 23+) |

## Acer-side touchpoints

1. `kernel/core/src/syscall/mod.rs` `sys_pid_current` returns federation-anchor fingerprint `0xe00b1a465d6dcb50` — matches Rust ≡ JS ≡ Python ≡ Falcon triple-parity verdict `:82272`. **Not affected** by Option-B regex (returns u64, not string format).
2. `kernel/core/src/pid/` BEHCS-1024 PID encoder — emits **anchor** PIDs only in v0.1; cp-variant emission lives in `behcs::session_pid` (servers/ tier in upcoming refactor).
3. Any acer-emitted envelope using non-canon ACER PID variants flagged by aether mission-4 (2 drift cases) — needs identification post joint-corpus run.

## Action items for acer-claude

- [ ] Wait for joint test-corpus from liris+aether (part_4 pending in liris envelope)
- [ ] Run acer-side regex validation against own emitted envelopes once corpus lands
- [ ] If drift confirmed, patch emitter + emit `ACER_PID_DRIFT_RECONCILE` envelope citing canonical form
- [ ] Cosign RATIFY_4_SUBCLASS when operator-apex (OP-JESSE + OP-RAYSSA) signal lands

## Status

- File-only ack (no bus post this cycle) — operator-apex RATIFY still pending per liris part_5
- Acer kernel-side syscall surface is **format-agnostic** (u64 fingerprint) so wiring work in Phase-3 proceeds in parallel
