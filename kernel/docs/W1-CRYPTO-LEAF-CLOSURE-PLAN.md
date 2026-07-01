# W1 Crypto-Leaf Closure PLAN

**Wave**: W1 of the kernel-stub-closure cascade
**Agent**: AGT-C10-ATHENA-W1-PLAN-PID-2026-05-19
**Land-mode**: LAND-AS-IS · descriptor-only (no kernel src edits in this task)
**Anchor (this doc)**: `w1cryptoleafplanlanded`
**Date**: 2026-05-19
**Prior cosign row**: `cosignv02landed175` (Phase-3 cosign_chain v0.2 landing, kernel/core)

---

## §1 Stub inventory — exact grep occurrences

Search: `Unimplemented` across `C:\asolaria-acer\federation-remake-1024\kernel`.
Total: **62 occurrences across 16 files**. Filtering to `Err(_::Unimplemented)` return-sites (i.e., callable stub bodies, excluding enum-variant defs, doc/comment lines, and `assert_eq!` test assertions) yields the closeable-candidate distribution.

Per-module Unimplemented occurrence counts (from grep, all 16 files):

| Module | File | Hits |
|---|---|---|
| boot/init | `kernel/boot/src/init.rs` | 1 |
| docs/USERSPACE_ABI | `kernel/docs/USERSPACE_ABI.md` | 3 |
| docs/PHASE_3_WIRING_STATUS | `kernel/docs/PHASE_3_WIRING_STATUS.md` | 11 |
| docs/TIER_1_SYSCALL_SECURITY_REVIEW | `kernel/docs/TIER_1_SYSCALL_SECURITY_REVIEW.md` | 6 |
| core/agent_runtime | `kernel/core/src/agent_runtime/mod.rs` | 6 |
| core/bus_and_kick | `kernel/core/src/bus_and_kick/mod.rs` | 1 |
| core/bus_fabric | `kernel/core/src/bus_fabric/mod.rs` | 3 |
| core/crypto | `kernel/core/src/crypto/mod.rs` | 5 |
| core/cosign_chain | `kernel/core/src/cosign_chain/mod.rs` | 1 |
| core/gnn | `kernel/core/src/gnn/mod.rs` | 6 |
| core/vfs | `kernel/core/src/vfs/mod.rs` | 1 |
| core/envelope | `kernel/core/src/envelope/mod.rs` | 5 |
| core/highway | `kernel/core/src/highway/mod.rs` | 5 |
| core/syscall | `kernel/core/src/syscall/mod.rs` | 6 |
| core/frame_alloc | `kernel/core/src/frame_alloc/mod.rs` | 1 |
| core/sign_gate | `kernel/core/src/sign_gate/mod.rs` | 1 |

Subset relevant to closure (kernel/core/src modules only, **12 modules**, excluding docs and boot stub-acks): agent_runtime, bus_and_kick, bus_fabric, crypto, cosign_chain, gnn, vfs, envelope, highway, syscall, frame_alloc, sign_gate. Sum of source-side hits = **41** (matches the per-module total once doc/comment lines and assert-eq test-mirrors are excluded).

Anchor count for closure planning: **41 grep occurrences across 12 kernel/core/src modules**.

## §2 True closeable surface

Not every `Unimplemented` is a closeable stub:

1. **Enum-variant declarations** (e.g., `CryptoErr::Unimplemented` at `crypto/mod.rs:34`, `AgentErr::Unimplemented`, etc.): NOT closeable. These are the error vocabulary; they must remain in the type for callers that pattern-match on the variant.
2. **Doc/comment mentions** (e.g., `///` lines explaining what a stub returns): NOT closeable directly.
3. **`assert_eq!(..., Err(_::Unimplemented))` in `#[cfg(test)]`** (e.g., `crypto/mod.rs:147`, `agent_runtime/mod.rs:241`, `highway/mod.rs:115`): NOT independently closeable. These tests will FAIL when their target function gets a real body; they must be re-asserted to the new contract as part of each closure.
4. **`return Err(_::Unimplemented)` / bare `Err(_::Unimplemented)` as final expr in `pub fn` body**: **closeable**. Each is one fn that needs a real implementation.

Distinguishing the two flavors of `Err(...::Unimplemented)`:
- **Return-site stub** (closeable): inside a `pub fn` body, no `assert_eq!` on the line.
- **Test-mirror** (not directly closeable): inside `#[cfg(test)] mod tests`, wrapped in `assert_eq!`.

Per Wave-3 PHANES, the deduplicated true closeable surface (return-site stubs only, no enum decls, no doc mentions, no test mirrors) is **25–30 fns**, NOT 15 (under-count, ignored re-exports) and NOT 211 (over-count from naive `grep Unimplemented`). The closure cascade plans against this 25–30 surface.

## §3 Dependency DAG

The 25–30 surface partitions into a DAG by what each fn calls. Closure order = leaves first (no inbound edges from other stubs):

```
                    crypto::{sign, verify, derive_public}        ← LEAVES (W1)
                              │
                              ▼
                         bus_fabric                              ← W2
                              │
                              ▼
                envelope (sign-path) + highway (route+verify)    ← W3
                              │
                              ▼
                             gnn                                 ← W4
                              │
                              ▼
                       agent_runtime                             ← W5
                              │
                              ▼
                       syscall (callers)                         ← already Phase-3 wired
```

Justification:
- **crypto** has no kernel dependencies (calls only `ed25519-dalek` + canonical_envelope_bytes). True leaves.
- **bus_fabric** publish/subscribe needs sign-on-publish (per Phase-9 tier security) → depends on crypto.
- **envelope** sign-path + **highway** verify-and-route both call `crypto::sign`/`crypto::verify` to seal/check on the wire.
- **gnn** edge-propagate runs over signed envelopes (post-highway).
- **agent_runtime** spawn/yield consumes the full stack.
- **sys_exit** (in syscall) is by-design diverging (`loop {}`); never closes — see §9.

## §4 W1 scope (THIS WAVE)

**Target functions** (exactly 3 stubs, all leaves of the DAG):

| # | Fn | File:line | Current body | Target contract |
|---|---|---|---|---|
| 1 | `pub fn sign(_canonical_bytes: &[u8], _priv: &PrivateKey) -> Result<Signature, CryptoErr>` | `kernel/core/src/crypto/mod.rs:84-86` | `Err(CryptoErr::Unimplemented)` | ed25519-dalek `SigningKey::from_bytes(priv.0).sign(canonical_bytes)` → wrap into `Signature([u8; 64])`. Reject zeroized/all-zero priv with `KeyMalformed`. |
| 2 | `pub fn verify(_canonical_bytes: &[u8], _sig: &Signature, _pub_key: &PublicKey) -> Result<(), CryptoErr>` | `kernel/core/src/crypto/mod.rs:90-96` | `Err(CryptoErr::Unimplemented)` | ed25519-dalek `VerifyingKey::from_bytes(pub_key.0)?.verify_strict(canonical_bytes, &sig.into())` → `Ok(())` or `SignatureInvalid`/`KeyMalformed`. |
| 3 | `pub fn derive_public(_priv: &PrivateKey) -> Result<PublicKey, CryptoErr>` | `kernel/core/src/crypto/mod.rs:100-102` | `Err(CryptoErr::Unimplemented)` | ed25519-dalek `SigningKey::from_bytes(priv.0).verifying_key().to_bytes()` → wrap into `PublicKey([u8; 32])`. |

Key citations:
- `crypto/mod.rs:84-86` — `sign` stub return-site
- `crypto/mod.rs:90-96` — `verify` stub return-site (multi-line signature, `Err(...)` at line 95)
- `crypto/mod.rs:100-102` — `derive_public` stub return-site

Out-of-scope for W1 (kept Unimplemented this wave): `CryptoErr::Unimplemented` enum variant at `crypto/mod.rs:34` STAYS (preserves `#[non_exhaustive]` future-extensibility); test mirror at `crypto/mod.rs:147` will be REPLACED, not deleted, with positive-path assertions (round-trip sign→verify, derive→verify).

## §5 Approach (per Phase-3 landing pattern from cosign row 175)

Mirror the cosign_chain v0.2 landing playbook:

1. **Scaffold** — add `ed25519-dalek = { version = "2.1", default-features = false }` to `kernel/core/Cargo.toml`. Confirm no_std compatibility.
2. **Wire** — replace the three stub bodies with real ed25519 calls; introduce a small internal helper `signing_key_from_seed(&PrivateKey) -> Result<SigningKey, CryptoErr>` shared by `sign` + `derive_public` for KeyMalformed mapping.
3. **Test** — add ≥6 new tests under existing `#[cfg(test)] mod tests`:
   - `round_trip_sign_then_verify_succeeds`
   - `verify_rejects_tampered_canonical_bytes` → `SignatureInvalid`
   - `verify_rejects_wrong_public_key` → `SignatureInvalid`
   - `derive_public_is_deterministic` (same seed → same pubkey)
   - `derive_public_matches_signing_key_pubkey` (consistency with sign)
   - `key_malformed_rejected` (all-zero or short seed where dalek refuses)
   - REPLACE `stub_sign_returns_unimplemented` with positive assertion.
4. **cargo test --lib** — must report `EXIT=0`, **≥208 passed** (target: 208 + new ≥6 = 214 passed).
5. **Cosign row** — append a row to the chain (see §7 format).

Mirrors Phase-3 row 175 exactly: scaffold-→-wire-→-test → row.

## §6 Ratchet floor

**Invariant**: `cargo test --lib` (run from `federation-remake-1024/kernel/core`) must report `EXIT=0` with `passed ≥ 208` at every checkpoint of W1.

- Baseline before W1: 208 passed (207 parallel + 1 #[ignore] serial), per cosign row 175.
- Target after W1: 214+ passed (208 baseline + ≥6 new crypto round-trip tests). Replaced stub-test does not change the count (1 deleted, ≥1 positive added).
- Ratchet rule: if any intermediate test run drops below 208, REVERT and re-plan. No row appended until floor holds AND new count ≥ baseline + (new tests added − tests replaced).

## §7 Cosign row format

Each closure within W1 (we will treat the full crypto-leaf set as a single row since they ship as one Cargo.toml + mod.rs landing — atomic from the chain's perspective) emits one cosign row:

```
row=N+1                                  (next after 175)
ts=<ns-unix>
prev_sha16=<sha16(canonical_bytes_of_row 175)>
kind=cryptosignv02landed176
payload_sha16=<sha16 of payload>
sig=<deferred to v0.3 ed25519, per Phase-3 doctrine>
payload= {
  eventId: "EVT-KERNEL-CORE-CRYPTO-W1-LANDED-2026-05-19",
  file: "kernel/core/src/crypto/mod.rs",
  fns_closed: ["sign", "verify", "derive_public"],
  tests_added: 6,
  tests_replaced: 1,
  cargo_test_lib_passed: 214,
  baseline_passed: 208,
  prior_anchor: "cosignv02landed175"
}
```

Anchor naming: per Phase-3 convention `<kind><pad><row>`, here `cryptosignv02landed176`. Subsequent W1 sub-closures (if W1 is later sub-divided) follow `cryptosignv02landed{NNN}` with monotonic NNN.

`prev_hash` chains directly off `cosignv02landed175` (cosign_chain v0.2 row).

## §8 Atlas cp 64 = `legacy_subset_256` domain

PHANES correction (from Wave-3): atlas cp 64 is the `legacy_subset_256` codepoint domain. The kernel does **NOT** directly own cp 64 — kernel inherits this slot via the `subset_embedding_cp` indirection, i.e., kernel-side code that needs cp-64 semantics goes through the embedding wrapper, not bare cp ownership.

Implication for W1: crypto closure does **not** touch atlas cp 64 directly. The ed25519 KEY (seed bytes) is a sealed-storage artifact (`/sealed/` partition or TPM per Phase-9), not an atlas codepoint. No atlas reservation needed for this wave. Any future wave that wants to mint a per-key codepoint (e.g., one cp per trusted-root pubkey) MUST go through `subset_embedding_cp`, not punch a kernel-owned slot.

## §9 W2–W5 sketch (brief)

- **W2 — bus_fabric** (`kernel/core/src/bus_fabric/mod.rs:85,91`): publish/subscribe with sign-on-publish via W1's `crypto::sign`. 2 closeable stubs.
- **W3 — envelope + highway**: envelope sign-path at `envelope/mod.rs:91,98` (2 stubs) + highway verify-and-route at `highway/mod.rs:79` (1 stub). 3 closeable stubs. Consumes W1 `verify`.
- **W4 — gnn** (`kernel/core/src/gnn/mod.rs:120,128,136,148`): edge-propagate over signed envelopes. 4 closeable stubs. Consumes W3 highway routing.
- **W5 — agent_runtime** (`kernel/core/src/agent_runtime/mod.rs:143,148,153`): spawn/yield/exit-child wiring. 3 closeable stubs. Consumes W4 gnn.
- **NEVER-CLOSE — `sys_exit`**: by-design diverging spin-loop stub per `PHASE_3_WIRING_STATUS.md` line 64. NOT a closure target — it ships forever as `loop {}`. Counted out of the 25–30 surface.

Approximate running total: W1 (3) + W2 (2) + W3 (3) + W4 (4) + W5 (3) = **15 high-confidence closures** on the strict-fn-body definition. The remaining 10–15 (to reach PHANES's 25–30 ceiling) come from cosign_chain v0.3 signature path, sign_gate, frame_alloc register_frame_region, bus_and_kick, vfs path extensions, and any newly-uncovered return-sites — all sequenced AFTER W5 in waves W6+.

---

**END W1-CRYPTO-LEAF-CLOSURE-PLAN.md**
**Status**: PLAN ONLY · no kernel src edits this task · descriptor file only.
**Next action (NOT this task)**: a separate landing-task with cargo-test gate will execute §5 and emit the row described in §7.
