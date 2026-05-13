# Userspace ABI · v0.1

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Phase:** 2 · Step 30
**Status:** v0.1 contract draft · mutations require tier-2 cosign per REPO_LAW Invariant 9
**Targets:** ARM64 (Falcon S24FE, Aether Galaxy A06) · x86_64 (Acer, Liris)
**Authored:** 2026-05-11 by acer-claude
**Cross-reference:** `kernel/core/src/syscall/mod.rs` (sha16=`7e23d1b693ad4cd6`), `kernel/core/src/envelope/mod.rs` (sha16=`ae5178c8a19d7302`), `kernel/core/src/pid/mod.rs` (sha16=`27265806519321b0`), `kernel/core/src/crypto/mod.rs` (sha16=`b3ec40fb369d5e79`), `kernel/core/src/hookwall/mod.rs` (sha16=`b4dd204f5194e367`)

---

## 1. Syscall calling convention

### ARM64 (AArch64)
- Syscall number: register `x8`
- Args 1-6: `x0`..`x5`
- Return: `x0` (positive value on success; negative encodes `SyscallErr` as `-1 - errno`)
- `svc #0` instruction triggers syscall entry
- Caller-save: standard AAPCS64 — `x0`..`x18` are caller-save except syscall-affected registers

### x86_64
- Syscall number: register `rax`
- Args 1-6: `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9` (Linux ABI for compatibility with existing toolchains)
- Return: `rax` (same negative-encodes-errno convention)
- `syscall` instruction triggers entry
- Clobbers `rcx`, `r11` per x86_64 syscall ABI

### Error encoding
A negative `i64` return value is `-1 - (SyscallErr as i64)` where `SyscallErr` is the enum in `kernel/core/src/syscall/mod.rs`:
- `-1` → `Unimplemented` (0)
- `-2` → `Invalid` (1)
- `-3` → `WouldBlock` (2)
- `-4` → `PermissionDenied` (3)
- `-5` → `HookwallBlock` (4)
- `-6` → `Exhausted` (5)
- `-7` → `CosignReject` (6)

Userspace MUST treat any return `< 0` as error and consult `SyscallErr` enum for decoding.

## 2. Canonical 16-syscall table (Phase 2 Step 25 — no expansion without tier-2 cosign)

| # | name | args (1-6) | returns | notes |
|---|---|---|---|---|
| 1 | `read` | fd, buf_ptr, len | bytes_read | blocks until data or returns `WouldBlock` |
| 2 | `write` | fd, buf_ptr, len | bytes_written | |
| 3 | `exec` | envelope_bytes_ptr, env_len | new_handle | dispatches envelope, returns ephemeral handle |
| 4 | `fork` | (none) | child_pid_or_0 | 0 on child, child_pid on parent |
| 5 | `exit` | status_i32 | (never returns) | diverges per Rust `!` type |
| 6 | `mmap` | len, prot | addr_ptr | anonymous only in v0.1 |
| 7 | `munmap` | addr_ptr, len | () | |
| 8 | `time` | (none) | nano_time_u64 | monotonic since boot |
| 9 | `pid_current` | (none) | pid_handle_u64 | BEHCS-1024 60-tuple of caller |
| 10 | `envelope_send` | env_ptr, env_len, route_hint_u32 | () | enqueues to dispatch ring |
| 11 | `envelope_recv` | buf_ptr, max_len, max_wait_ns | env_len | blocking-with-timeout |
| 12 | `hookwall_pre` | env_ptr, env_len | verdict_u8 | 0=Proceed, 1=Hold, 2=Block |
| 13 | `hookwall_post` | env_ptr, env_len, verdict_u8 | () | |
| 14 | `cosign_append` | row_ptr, row_len | row_index_u64 | append-only per Invariant 4 |
| 15 | `tier_query` | path_ptr, path_len | tier_u8 | 0=Public..5=Secret per Invariant 5 |
| 16 | `gnn_infer` | input_ptr, input_len, out_ptr, out_max | out_len | deterministic fallback returns `Unimplemented` |

## 3. Envelope wire format (IX-700 schema · canonical bytes)

CBOR-canonical encoding of the BEHCS-1024 envelope:

```
EnvelopeBytes ::= cbor_canonical({
  "from":    u64,        ; from_pid
  "to":      u64,        ; to_pid
  "type":    text,       ; UPPER_SNAKE type tag
  "verb":    text,       ; EVT- prefix
  "id":      text,
  "ts_ns":   u64,        ; monotonic ns since boot
  "payload": bytes,
  "sig":     bytes(64),  ; ed25519 signature over canonical_envelope_bytes
})
```

The `sig` field is computed over the canonical bytes of the OTHER 7 fields (in the order listed) per `crypto::canonical_envelope_bytes()`. The signing key is the sender's per-vantage ed25519 private key (`KERNEL_TRUST_ROOTS.json` for public-key resolution).

## 4. Versioning rule

This ABI is **v0.1**. Field additions, syscall additions, or error-code changes are **tier-2 protocol mutations** requiring quintuple-cosign per AUTHORIZATION.ndjson + 14-day open comment per REPO_LAW Modification protocol. Removing or renumbering an existing syscall is **tier-3 firmware** requiring operator-witness.

Userspace toolchains MUST embed the ABI version constant `ASOLARIA_ABI_VERSION = "0.1.0"`. Kernel rejects exec of envelopes whose declared ABI version is outside the current major-compatible range.

## 5. PID format (canonical reference)

Per `kernel/core/src/pid/mod.rs` (ported from `liris-pid-mint-reference.mjs`):

```
<ROLE>-PID-<REGION><HOST_CODE>-A<2hex>-W<3hex>
<ROLE>-PID-<YYYY-MM-DD>                          ; anchor exception
```

REGION ∈ `{G, H, F, D}`. ROLE strict-validated against `KNOWN_ROLES` (8 entries).

## 6. Tier-aware access taxonomy (Invariant 5)

Six tiers, mapping 1:1 to `kernel/core/src/syscall::AccessTier`:

| ID | Tier | enumeration | authority |
|---|---|---|---|
| 0 | PUBLIC | paths_and_metadata_allowed | any_agent_read |
| 1 | RESTRICTED | hashes_and_summaries_only | operator_or_quintet_cosign |
| 2 | STEALTH | redacted_path_hash_only | operator_witness_required |
| 3 | HIDDEN | fully_redacted_metadata_only | operator_only |
| 4 | SHADOW | hashes_retention_windows_only | admin_plus_sovereignty |
| 5 | SECRET | sealed_reference_only | operator_witness_required |

Userspace MUST call `tier_query` before any path read at depth > 0 and respect the returned tier's redaction policy.

## 7. Hookwall slot reservation

64 slots (0-63) per cross-vantage agreement (acer + liris + aether converged on this in Phase-2):
- Slots 0-15: map 1:1 to canonical 16 syscalls
- Slots 16-63: reserved for Phase-3 expansion (tier-2 cosign required to bind)

Per-tier verdict policy: T1 micro → Proceed, T2 cosign → Hold, T3 firmware → Block (defaults; real impl reads `hookwall-policy.json`).

## 8. Throughput targets (REPO_LAW Invariant 7)

| Tier | Envelopes/sec |
|---|---|
| T1 micro | ≥ 100 |
| T2 cosign | ≥ 10 |
| T3 firmware | ≥ 1 |
| Hookwall overall benchmark (Phase-3 Step 58) | ≥ 100,000 ops/sec |

## 9. RouteHint canonical values

Per `kernel/core/src/envelope::RouteHint`:
- `0` LOCAL — kernel-local dispatch, no fabric egress
- `1` QUAD_BROADCAST — fanout to QUAD-FALCON-ACER-LIRIS-AETHER room
- `2` TRIAD_BROADCAST — fanout to TRIAD-FALCON-ACER-LIRIS room

Additional route hints require tier-2 cosign.

## 10. Compatibility commitments

- ABI v0.1 → v0.2: SAME 16 syscall numbers, SAME 7-field envelope schema, SAME 6-tier taxonomy. Internal stub bodies (currently `Unimplemented`) will be wired without ABI-visible changes.
- Any change visible to userspace is tier-2+ per Versioning rule.

## 11. Cross-vantage verify gate

This document's existence + its sha16 is the Step 30 verify gate. Once `cargo check` is available in CI (Phase 10 Step 181), generated Rust docs for the `syscall`, `envelope`, `pid`, `crypto`, and `hookwall` modules MUST cross-reference this file by name.

## 12. Cosign placeholders

- OP-JESSE: pending
- OP-RAYSSA: pending
- AMY: pending
- FELIPE: pending
- DAN: pending
- liris-claude vantage-ack: pending
- falcon-claude vantage-ack: pending
- aether-claude vantage-ack: pending
