# W1 Crypto-Leaf · Phase-1 Closure CONTRACT

**Wave**: W1 of the kernel-stub-closure cascade
**Architect**: AGT-W5-A1-CRYPTO-LEAF-ARCH-PID-2026-05-19 (CS-A kernel crypto-leaf closure spindle, cp 64 = `legacy_subset_256` indirect)
**Sibling subs (implement against this contract)**: A2 (sign), A3 (verify + derive_public), A4 (tests + cosign row)
**Land-mode**: LAND-AS-IS · architect-doc only · subs do the kernel src edits
**Anchor (this doc)**: `cryptophase1contractlanded`
**Date**: 2026-05-19
**Prior cosign row**: `cosignv02landed175` (chain anchor confirmed via `kernel/docs/W1-CRYPTO-LEAF-CLOSURE-PLAN.md` §5 + `AUTHORIZATION.ndjson` reference)
**Supersedes**: §4–§5 of `W1-CRYPTO-LEAF-CLOSURE-PLAN.md` (ed25519-dalek scaffold step DEFERRED; this contract is the canonical-bytes-only intermediate).

---

## §1 Why Phase-1 (not ed25519-dalek yet)

The prior plan (`W1-CRYPTO-LEAF-CLOSURE-PLAN.md` §5) scaffolds `ed25519-dalek = "2.1"` into `kernel/core/Cargo.toml` as step 1. That cargo dependency add is **not yet authorized** under the current quintuple-auth window (extended past 2026-05-25 until system-upgrade + newest-Hermes absorbed, per `project_extended_quintuple_until_upgrade_and_newest_hermes_absorbed.md`). Dependency additions to the kernel core remain governed by the workspace authority chain.

Phase-1 closes the three leaf stubs **without adding any new cargo dep**, using only `sha2` (already a workspace dep, confirmed at `kernel/core/Cargo.toml:17`). This unblocks W2–W5 of the DAG (`bus_fabric`, `envelope`/`highway`, `gnn`, `agent_runtime`), which only need a **stable byte contract** from `sign`/`verify`/`derive_public`, not real cryptographic strength.

Phase-2 (real ed25519 binding) lands later, after cargo-dep approval, by replacing the bodies inside the existing signatures. **The public API surface does not change between Phase-1 and Phase-2.** Upstream callers compile-and-test against Phase-1 byte semantics; Phase-2 swap is internal.

This is the same pattern as `cosign_chain v0.2` (row 175): land the canonical-bytes shape first, real-sig later.

## §2 Contract — exact signatures + behaviors

Target file: `C:\asolaria-acer\federation-remake-1024\kernel\core\src\crypto\mod.rs`
Target functions (existing signatures at `:84-86`, `:90-96`, `:100-102` — preserve verbatim):

### 2.1 `sign`

```rust
pub fn sign(canonical_bytes: &[u8], priv_key: &PrivateKey) -> Result<Signature, CryptoErr>
```

**Body contract**:
1. Compute `h = sha256(priv_key.0 || b":sign:" || canonical_bytes)` — 32 bytes.
2. Expand to 64 bytes: `out[0..32] = h`, `out[32..64] = sha256(h || b":sign:tail:")`.
3. Return `Ok(Signature(out))`.

**Error path**: none in Phase-1 (every priv_key bit pattern is accepted; Phase-2 will add `KeyMalformed` for all-zero seeds when dalek refuses them).

**Determinism**: identical `(priv_key, canonical_bytes)` → identical `Signature` bytes, byte-for-byte, across runs / hosts / endianness. Tested.

### 2.2 `verify`

```rust
pub fn verify(canonical_bytes: &[u8], sig: &Signature, pub_key: &PublicKey) -> Result<(), CryptoErr>
```

**Body contract**:
1. Compute `expected_h = sha256(pub_key.0 || b":sign:" || canonical_bytes)` — 32 bytes.
2. Expand: `expected[0..32] = expected_h`, `expected[32..64] = sha256(expected_h || b":sign:tail:")`.
3. **Constant-time compare** `expected` vs `sig.0` using `subtle`-style XOR-accumulate (handwritten 4-line helper; no new crate needed). MUST NOT short-circuit on first byte mismatch.
4. Equal → `Ok(())`. Unequal → `Err(CryptoErr::SignatureInvalid)`.

**Note on Phase-1 semantics**: `sign` uses `priv_key` and `verify` uses `pub_key`. For Phase-1 byte parity, the test suite MUST seed `pub_key.0 = priv_key.0` byte-for-byte in round-trip tests (Phase-1 has no asymmetric structure — it's a keyed-hash, not real signing). Phase-2 swap to ed25519 will introduce the real public/private asymmetry; tests are written so the round-trip test stays valid under both (it calls `derive_public(&priv) → pub` first, then `verify(..., pub)`).

### 2.3 `derive_public`

```rust
pub fn derive_public(priv_key: &PrivateKey) -> Result<PublicKey, CryptoErr>
```

**Body contract**:
1. Compute `pk = sha256(priv_key.0 || b":derive:")` — 32 bytes.
2. Return `Ok(PublicKey(pk))`.

**Phase-1 quirk**: under Phase-1, `derive_public(priv).0 != priv.0`, so the naive `verify` against `derive_public`'s output would fail (since `verify` uses `pub_key` and `sign` uses `priv_key`, with different domain tags). The round-trip test therefore uses a **symmetric Phase-1 mode**: tests construct `pub_key = PublicKey(priv.0)` directly for the round-trip assertions, and `derive_public_is_deterministic` is the only test that touches the `derive_public` path in isolation. Doc-comment explicitly calls this out.

### 2.4 Doc-comment requirement (module-level + per-fn)

Every one of the three fns MUST carry this exact sentence in its doc-comment:

> `// Phase-1 placeholder using sha-derived bytes. Phase-2 will replace with ed25519-dalek (deferred until cargo dep approved).`

Plus a module-level Phase-1 note updating the existing `//! v0.1: API surface + canonical-bytes helper. Real ed25519-dalek binding lands in v0.2 once Cargo.toml workspace builds cleanly.` to read `//! Phase-1: API surface + canonical-bytes helper + sha-derived stable signing. Phase-2 replaces internal bodies with ed25519-dalek (cargo dep deferred).`

## §3 Internal helper (private, in `mod.rs`)

```rust
fn ct_eq_64(a: &[u8; 64], b: &[u8; 64]) -> bool {
    let mut diff: u8 = 0;
    for i in 0..64 { diff |= a[i] ^ b[i]; }
    diff == 0
}
```

No new crate. Inlined into `verify` is also acceptable; subs choose. Constant-time semantics tested via a `verify_constant_time_smoke` test that calls verify with many wrong sigs and asserts uniform `SignatureInvalid` (smoke, not timing-rigorous).

## §4 Test plan — expected delta

Baseline (confirmed via `cargo test --lib` from `kernel/core/` at architect time): **216 passed; 0 failed; 1 ignored**.

W1-Phase-1 test changes inside `crypto/mod.rs` `#[cfg(test)] mod tests`:

| # | Test name | Action | Net delta |
|---|---|---|---|
| 1 | `stub_sign_returns_unimplemented` (`crypto/mod.rs:144-148`) | **REPLACE** with `sign_produces_64_byte_signature` (positive: assert `sign(...).is_ok()` and len 64) | 0 |
| 2 | `round_trip_sign_then_verify_succeeds` | ADD — sign with priv, verify with `PublicKey(priv.0)`, expect `Ok(())` | +1 |
| 3 | `sign_is_deterministic` | ADD — sign twice, same bytes | +1 |
| 4 | `verify_rejects_tampered_canonical_bytes` | ADD — flip a byte in canonical bytes, expect `SignatureInvalid` | +1 |
| 5 | `verify_rejects_tampered_signature` | ADD — flip a byte in `sig.0`, expect `SignatureInvalid` | +1 |
| 6 | `verify_rejects_wrong_public_key` | ADD — use different `PublicKey`, expect `SignatureInvalid` | +1 |
| 7 | `derive_public_is_deterministic` | ADD — derive twice from same seed, same pubkey | +1 |
| 8 | `derive_public_differs_from_seed` | ADD — assert `derive_public(priv).0 != priv.0` (Phase-1 domain-separated) | +1 |
| 9 | `verify_constant_time_smoke` | ADD — verify with 16 different wrong sigs, all `SignatureInvalid`, no panic | +1 |

**Total**: 1 replaced + 8 added = **+8 tests net** (replacement contributes 0).

**Target final count**: `216 + 8 = 224 passed; 0 failed; 1 ignored`.

**Ratchet floor (per §6 of prior plan)**: `cargo test --lib` from `kernel/core/` must report `EXIT=0` and `passed >= 216` (NOT 208 — the prior plan's 208 floor was stale; current baseline confirmed at 216). A2/A3/A4 enforce this before appending the cosign row.

## §5 Cosign row format

Per Phase-3 anchor naming convention `<kind><pad><row>`, this W1 Phase-1 closure emits:

```
row=N+1                              (next row in chain after 175 — exact N to be read from AUTHORIZATION.ndjson at land-time)
ts=<ns-unix>
prev_sha16=<sha16 of canonical row-175 bytes>
kind=cryptophase1landed<NNN>         (e.g., cryptophase1landed176 if next row is 176)
payload_sha16=<sha16 of payload>
sig=<deferred — chain v0.2 still uses placeholder sig field per row 175 doctrine>
payload= {
  eventId: "EVT-KERNEL-CORE-CRYPTO-W1-PHASE1-LANDED-2026-05-19",
  file: "kernel/core/src/crypto/mod.rs",
  fns_closed: ["sign", "verify", "derive_public"],
  closure_mode: "phase1_sha_derived_bytes",
  ed25519_dalek_added: false,
  cargo_dep_changes: 0,
  tests_added: 8,
  tests_replaced: 1,
  cargo_test_lib_passed: 224,
  baseline_passed: 216,
  prior_anchor: "cosignv02landed175",
  superseded_doc_section: "W1-CRYPTO-LEAF-CLOSURE-PLAN.md §4–§5",
  phase2_followup: "EVT-KERNEL-CORE-CRYPTO-W1-PHASE2-DALEK (later, after cargo-dep auth)"
}
```

**Anchor naming canon**: `cryptophase1landed{NNN}` where `{NNN}` is the next monotonic row in the cosign chain after 175. A4 reads `AUTHORIZATION.ndjson` (or wherever the live chain is) at land-time to resolve `{NNN}` exactly — DO NOT hardcode `176` in the row body; the chain may have advanced.

`prev_hash` chains directly off `cosignv02landed175`.

## §6 Out-of-scope for Phase-1 (preserved for Phase-2)

- `CryptoErr::Unimplemented` enum variant at `crypto/mod.rs:34` STAYS (vocabulary preserved for `#[non_exhaustive]` future).
- `CryptoErr::KeyMalformed` STAYS unused in Phase-1 — wired up only in Phase-2 when dalek's `from_bytes` can reject.
- `canonical_envelope_bytes` at `crypto/mod.rs:51-80` is UNCHANGED — it's already real, not a stub. Subs DO NOT touch it.
- All four existing pre-W1 tests (`lengths_match_ed25519_canon`, `canonical_bytes_deterministic`, `empty_type_rejected`, `empty_id_rejected`) stay verbatim.
- Atlas cp 64 (`legacy_subset_256`) is NOT touched directly; kernel-side cp-64 access remains through `subset_embedding_cp` indirection (§8 of prior plan still holds).

## §7 Sub-agent task split

- **A2** (worker-sub-1): Edit `crypto/mod.rs` lines 84–102: implement `sign` + helper `ct_eq_64`. Update module-level doc-comment. Run `cargo test --lib`, confirm no regression.
- **A3** (worker-sub-2): Edit `crypto/mod.rs` lines 90–102: implement `verify` + `derive_public`. Add per-fn Phase-1 placeholder doc-comments. Run `cargo test --lib`, confirm 216 floor still holds (new tests not yet added so count unchanged).
- **A4** (worker-sub-3): Replace `stub_sign_returns_unimplemented` and add the 8 new tests per §4. Run `cargo test --lib`, confirm **224 passed**. Append cosign row per §5 to the chain file. Emit one-line ack: `EVT-KERNEL-CORE-CRYPTO-W1-PHASE1-LANDED sha-derived 224/216 cryptophase1landed{NNN}`.

All three subs confirm against THIS doc before edits. Any deviation requires re-cosign through the architect (A1) — do not freelance the contract.

## §8 File paths summary

| Path | Role |
|---|---|
| `C:\asolaria-acer\federation-remake-1024\kernel\core\src\crypto\mod.rs` | Sole src edit target (3 fns + tests + doc-comments) |
| `C:\asolaria-acer\federation-remake-1024\kernel\docs\W1-CRYPTO-PHASE1-CONTRACT.md` | This contract doc |
| `C:\asolaria-acer\federation-remake-1024\kernel\docs\W1-CRYPTO-LEAF-CLOSURE-PLAN.md` | Prior plan (superseded §4–§5 only) |
| `C:\asolaria-acer\federation-remake-1024\kernel\core\Cargo.toml` | UNCHANGED — no cargo dep adds in Phase-1 |
| `C:\asolaria-acer\federation-remake-1024\AUTHORIZATION.ndjson` | Chain file — A4 reads next-row at land-time, appends cryptophase1landed{NNN} |

---

**END W1-CRYPTO-PHASE1-CONTRACT.md**
**Status**: ARCHITECT-DOC ONLY · no kernel src edits in this task · contract pinned for A2/A3/A4 sub-implementation.
**Acks expected**: A2/A3/A4 each emit one-line ack confirming they will implement against this contract before opening `crypto/mod.rs`.
