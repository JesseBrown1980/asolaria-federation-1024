# KERNEL_TARGETS · Phase-2 Substrate Decision Document

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Phase:** 2 · Steps 21-40 · Kernel Substrate
**Lead vantage:** acer (Windows x86_64, full hardware reach, WSL2 + native build capability)
**Co-authors (via Rule-11 fabric-passthrough):** falcon-claude (PRoot-authored kernel envelopes), liris-claude (sister-build target + drift-watch)
**Authored:** 2026-05-11T17:02:00Z
**Status:** decisions documented, awaiting cosign

---

## Step 21 · Target architecture matrix

| Arch | Devices in federation | Build priority | Rationale |
|---|---|---|---|
| **ARM64** | Falcon S24FE (SM-S721U1) · Aether Galaxy A06 (SM-A065M) · 2 of 4 fabric peers | **1st** | Mobile peers are bandwidth-limited and PRoot-sandboxed; native kernel reach matters more there |
| **x86_64** | Acer (Windows 11) · Liris (Windows 11) · 2 of 4 fabric peers | **2nd** | Desktop peers already have full host reach via WSL2; less urgent |
| RISC-V | none currently | future | Reserved for bare-node addition phase (Phase 8 mention) |

**Decision: ARM64 first, x86_64 cross-compile to follow within Phase 2.**

## Step 22 · Kernel language

**Decision: Rust (no_std, edition 2021)**

Rationale:
- Memory safety at kernel layer (no buffer overflows in syscall handlers)
- `bootloader` crate v0.11+ gives UEFI minimal stub out of the box
- `ed25519-dalek` no_std support → invariant 4 (ed25519 substrate at kernel) trivial
- Cargo workspace structure cleanly maps to phase-step subdirectories
- Reproducible builds via `cargo --locked` + lockfile commit
- Wide ARM64+x86_64 LLVM backend support

Rejected alternatives:
- **Zig** — smaller ecosystem; no_std story less mature for cryptographic primitives
- **C** — no_std is the default but ed25519 + envelope dispatch reimplementation = bloat per Invariant 9
- **D / Nim** — too niche; team familiarity low; cosign + audit tooling sparse

## Step 23 · Boot loader

**Decision: `bootloader` crate v0.11.7 with UEFI minimal stub**

Targets a single `.efi` artifact that:
- Loads via UEFI on x86_64 hardware (Acer + Liris testbeds)
- Loads via UEFI on ARM64 (Falcon-class hardware once rooted; QEMU emulation otherwise)
- Hands off to Rust `_start` with framebuffer + memory map + ACPI info
- ≤ 50 lines of bootloader-glue Rust

Build artifact path: `federation-remake-1024/kernel/target/<arch>-unknown-uefi/release/asolaria-os.efi`

## Step 24 · USB-bootable image build

**Decision: QEMU first, physical USB second**

Order:
1. `scripts/qemu-test.sh` — boots .efi in QEMU x86_64 (acer dev loop, ~3s boot)
2. `scripts/qemu-test-arm.sh` — boots .efi in QEMU AArch64 (validates ARM64 cross-compile)
3. `scripts/build-usb-img.sh` — produces `.img` for `dd` to physical USB stick (acer + liris testbeds)
4. Physical USB validation acer first (acer-claude has hardware reach), then liris

USB stick canonical: 4GB minimum. Sovlinux 2TB USB on liris is too large for kernel-only testbed; reserve it for full-system images later.

## Step 25 · Initial syscall surface

**Decision: 16 syscalls max for v0.1.0**

Canonical 16:
1. `read(fd, buf, len)`
2. `write(fd, buf, len)`
3. `exec(envelope)` — executes a BEHCS-1024 envelope (NOT path; envelopes are the dispatch unit)
4. `fork()` — process clone
5. `exit(status)`
6. `mmap(addr, len, prot)` — anonymous map only in v0.1
7. `munmap(addr, len)`
8. `time()` — monotonic nanoseconds since boot
9. `pid_current()` — returns BEHCS-1024 PID of caller
10. `envelope_send(env, route)` — send envelope to dispatcher
11. `envelope_recv(buf, max_wait_ns)` — blocking-with-timeout recv
12. `hookwall_pre(env)` — pre-exec hook (invariant 2)
13. `hookwall_post(env, verdict)` — post-exec hook
14. `cosign_append(row)` — append-only cosign-chain row
15. `tier_query(path)` — returns 1-6 access-tier per invariant 5
16. `gnn_infer(input)` — GNN inference invariant 3 (deterministic fallback if model unavailable)

**No other syscalls in v0.1.** Per Invariant 9 (no bloat). Any addition is a tier-2 cosign-required protocol change.

## Step 26 · Memory model: PIDs as content-addressable addresses

Each kernel-allocated memory region is addressed by its BEHCS-1024 PID. Lookup is O(K) prefix-tree walk per `reference_brown_hilbert_cube_of_cubes_port_division.md` (K=20 typical → 20 hops to any address in 256^20 ≈ 10^48 space).

This breaks the classic virtual-memory page-table abstraction. Instead:
- Allocation returns a PID
- Read/write syscalls take PID instead of pointer
- MMU is repurposed as PID-cache (recently-resolved PID→physical-page mappings)
- Page fault becomes "PID-cache miss" → walk prefix-tree

Acer-claude commits to writing the design RFC for this as Step 26's deliverable. Falcon-claude (via fabric-passthrough) is invited to author the security review.

## Step 27 · Kernel PID minter

**Port `tools/behcs/brown-hilbert-pid-cell-minter.mjs` from liris → `kernel/src/pid/mint.rs`**

Function signature:
```rust
pub fn mint_behcs1024_pid_60(content: &[u8], context: &MintContext) -> Pid1024;
```

Output must match the JS reference byte-for-byte. Test vectors derived from existing minter output (sample 100 known content→PID pairs and verify Rust output identical).

## Step 28 · ed25519 substrate at kernel layer

**Decision: `ed25519-dalek = "2.1.1"` (no_std + `zeroize`)**

Used at kernel for:
- Signing envelope dispatch decisions (cosign-chain append per invariant 4)
- Verifying inbound envelopes from peer vantages (per Rule 6 fabric-passthrough)
- Per-tier key derivation (ed25519 + HKDF per Phase 9)

Key storage: kernel maintains per-vantage public keys in `KERNEL_TRUST_ROOTS.json` (build-time injected); private key lives in TPM where available or `/sealed/` partition where not.

## Step 29 · Atomic envelope dispatch primitive

**Decision: lock-free MPMC ring buffer (capacity 16384 envelopes)**

Targets:
- ≥ 10⁵ envelope dispatches/sec (per Invariant 2 hookwall benchmark)
- < 1μs p99 enqueue/dequeue
- Backpressure via gc-trigger at 75% full (matches BEHCS 2000-msg gulp canon scaled to kernel)

Reference impl: `crossbeam-queue::ArrayQueue` adapted for no_std.

## Step 30 · Userspace ABI spec

Write `USERSPACE_ABI.md` with:
- Syscall calling convention per arch (ARM64 + x86_64)
- Envelope wire format (CBOR-encoded, IX-700 schema)
- Per-syscall error codes (mapped to envelope `payload.err` strings)
- Versioning rule: ABI changes = quintuple-cosign tier-2

Owner: acer-claude. Reviewer: falcon-claude (fabric-passthrough).

## Steps 31-40 · Driver model, USB enumeration, NIC, storage, SBOM, build-img, KERNEL_TIER_LANDED

These steps are well-defined in the 200-step plan. Each gets a sub-document in `federation-remake-1024/kernel/docs/`:

| Step | Sub-doc | Owner |
|---|---|---|
| 31 | `init-system-spec.md` | acer-claude |
| 32 | `driver-model.md` | acer-claude |
| 33 | `usb-fabric-mapping.md` | falcon-claude (mobile USB expertise) |
| 34 | `nic-bus-native.md` | liris-claude (acer↔liris ethernet testbed) |
| 35 | `storage-behcs256-fs.md` | acer-claude |
| 36 | `tier1-syscall-review.md` | operator-witness required |
| 37 | `build-img-script.md` | sub-agent |
| 38 | `repro-builds.md` | sub-agent |
| 39 | `sbom.md` | sub-agent |
| 40 | `KERNEL_TIER_LANDED.envelope.json` | hermes |

## Kernel directory structure (initial)

```
federation-remake-1024/
├── kernel/
│   ├── Cargo.toml                    (workspace root)
│   ├── boot/                         (UEFI stub)
│   │   └── Cargo.toml
│   ├── core/                         (no_std kernel crate)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── pid/
│   │       │   ├── mod.rs
│   │       │   └── mint.rs           (Step 27)
│   │       ├── envelope/
│   │       │   ├── mod.rs
│   │       │   └── dispatch.rs       (Step 29)
│   │       ├── crypto/
│   │       │   └── mod.rs            (Step 28 ed25519)
│   │       ├── hookwall/
│   │       │   └── mod.rs            (Phase 3 hooks)
│   │       └── syscall/
│   │           └── mod.rs            (Step 25, 16 syscalls)
│   ├── docs/                         (Steps 31-39 sub-docs)
│   ├── scripts/
│   │   ├── qemu-test.sh
│   │   ├── qemu-test-arm.sh
│   │   ├── build-usb-img.sh
│   │   └── sbom.sh
│   └── tests/
│       └── pid-mint-vectors.rs       (Step 27 verification)
└── KERNEL_TARGETS.md                 (this file)
```

## Anti-bloat enforcement for Phase 2

Per Invariant 9 (no bloat) and Invariant 10 (honesty):
- Every file in `kernel/` must be referenced by at least one Cargo dependency OR one phase-step doc — orphans get deleted at end of Phase 2
- No "future-use" placeholder files
- No commented-out code blocks > 5 lines (move to design doc instead)
- No copy-pasted modules from old BEHCS-256 repo without an explicit "absorbed-from" comment header

## Cosign placeholders

- OP-JESSE: pending
- OP-RAYSSA: pending
- AMY: pending
- FELIPE: pending
- DAN: pending
- liris-claude vantage-ack: pending (already cosigned ACER_PHASE_1_CONTRIBUTION at :TBD)
- falcon-claude vantage-ack: pending (fabric-passthrough role)
- aether-claude vantage-ack: pending

---

**This document unblocks Steps 21-40. Next acer-side action: scaffold the `kernel/` Cargo workspace per the directory structure above.**
