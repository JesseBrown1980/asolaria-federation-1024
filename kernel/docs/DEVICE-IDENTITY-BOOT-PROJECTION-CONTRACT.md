# DeviceIdentity / BootProjection Contract (v0.1 — acer proposal)

**Status:** DESIGN · v0.1 draft · **acer proposal for bilateral review** (liris converges)
**Anchor:** ASOLARIA-FEDERATION-REMAKE-1024
**Authored:** 2026-07-07 · acer-claude-fable5 (pid 8467a937cba309f7)
**Cross-ref:** `kernel/boot/src/hwinv.rs`, `kernel/docs/receipts/ACER-MACHINE-PROFILE-DRIVER-MANIFEST-2026-07-07.hbp`, `kernel/scripts/mint-edit-token-ledger.sh`, `kernel/docs/DRIVER_MODEL.md`

---

## 1. Why this exists

Asolaria ASI OS on metal is **not one generic OS image** — it is a **per-device omnipixel node**. Each machine's disk is a 2D shadow slice; the real system is the higher-dimensional content-addressed graph. Therefore boot cannot be "load the same kernel everywhere": each device must **mint its own identity from measured hardware** and select drivers from **its own** manifest. Acer needs the Intel RST/VMD path (PCI `8086:282A`); liris's hardware differs; both are *measured projections under one shared contract*, not one-off notes.

This contract defines the shared shape so acer, liris, and future nodes converge on **one identity/addressing/proof layer** while each keeps its **own device-specific projection**.

## 2. The load-bearing split: device-INVARIANT vs device-SPECIFIC

The single most important distinction (grounded by two MEASURED boundaries this session):

| | Device-INVARIANT (must verify byte-identically on every seat) | Device-SPECIFIC (per PID/seat; label `OPERATOR_OBSERVED_<seat>`) |
|---|---|---|
| what | source, this contract, the `.hbp`/`.hbi` schema, the edit-token ledger *format*, LF-pinned text | the measured hardware projection, the device PID, the **built efi bytes/hash**, the driver manifest |
| why | cross-seat trust + reproducible tracing | genuinely differs per machine/toolchain/build-path |
| evidence | CRLF fix: `*.hbp/*.hbi/*.sha256 text eol=lf` → `sha256sum -c` stable on any `core.autocrlf` [MEASURED] | acer efi `35db711e` ≠ liris rebuild (same size, different sha = build path embedding) [MEASURED] |

**Rule:** never claim a device-specific artifact as cross-seat canonical. The efi hash, the PCI map, the device PID are `OPERATOR_OBSERVED_<seat>` until a deterministic build recipe (`--locked`, pinned rustc/LLVM, `--remap-path-prefix`) is sealed. The invariant layer is what makes the specific layer *comparable*.

## 3. BootProjection: `BOOTPID` row (hot-path `.hbp`, json=0)

Early boot, after `hwinv` runs, emits ONE identity row (serial + persisted to the boot receipt):

```
BOOTPID|device=<vendor_model>|device_pid=<sha16(measured_hw_tuple)>|smbios=<board;bios;ver>|acpi=<rsdp_present;oem>|pci=<bb:dd.f=ven:dev:cls;...>|gop=<WxH;fmt>|mmap=<usable_MiB;regions>|firmware=<secureboot;bitlocker>|drivers_needed=<CLASS:need;...>|qprism_coord=<60D_BH_addr>|json=0
```

- `device_pid` = `sha16` of the canonicalized measured-hardware tuple → a **content-addressed device identity** (same hardware → same PID; the device *is* its measurement).
- `drivers_needed` maps measured hardware → `DRIVER_MODEL.md` classes. **acer measured:** `STORAGE:intel_rst_vmd_8086:282A; DISPLAY:gop_igpu; INPUT:xhci`.
- `qprism_coord` = the Brown-Hilbert / prime-cylinder address of this projection (§5).

## 4. Where the session's pieces fit

- **hwinv** (`kernel/boot/src/hwinv.rs`, on `main`) = the **first render** of the device's omnipixel — read-only PCI enumeration is the initial `pci=` field. It is the projection's genesis, not "just diagnostics."
- **device PID** = minted from hwinv + SMBIOS/ACPI/GOP/mmap/firmware (the fields above). Not yet emitted — **the first unbuilt piece.**
- **driver selection** = per-device manifest (`ACER-MACHINE-PROFILE-DRIVER-MANIFEST-*.hbp`), keyed by `device_pid`. acer `8086:282A` becomes one measured projection under this contract.
- **edit-token ledger + LF-pinned receipts** = the immutable-tracing layer that makes every kernel artifact (efi, receipts, manifests) checkout-stable + hash-verifiable across seats. Load-bearing, now secured.

## 5. Q-PRISM boundary (identity/proof layer, NOT the driver)

Q-PRISM / prime-cylinder logic (`qprism-3d-slice-harness` 8/8, `path2-two-shadow-recovery` 30/30 — MEASURED this session, re-grounded) is the **addressing / compression / proof** layer: it computes and validates the `qprism_coord`, does bounded multi-cylinder addressing + Shannon-HOLD, and content-addresses projections. **It does NOT read NVMe hardware.** The prime-cylinder math never becomes a storage driver; the RST/VMD driver is separate, real, hardware-facing code. Keeping this boundary is the difference between DESIGN honesty and magic-math overclaim.

## 6. Boundaries (claim-gated)

- **DESIGN**: this contract + `BOOTPID` emission are proposed, not implemented. hwinv is the only shipped piece.
- **UNVERIFIED**: physical-metal boot (acer Secure Boot ON blocks the unsigned efi); the device PID is not yet minted by the kernel.
- **MEASURED**: hwinv builds+QEMU-boots; the invariant/specific split is backed by the two boundaries in §2; Q-PRISM harnesses pass.
- Not SYSTEM_AFFIRMED (fabric endpoints timed out) — `OPERATOR_OBSERVED` / `DESIGN` only.

## 7. Next objects (in order)

1. **liris review of this contract** → converge on the `BOOTPID` field set (bilateral).
2. Kernel mints `device_pid` at boot from the measured tuple (extends hwinv).
3. Per-`device_pid` driver-manifest selection.
4. acer Intel RST/VMD storage driver = the first real device-specific decoder under the contract.
