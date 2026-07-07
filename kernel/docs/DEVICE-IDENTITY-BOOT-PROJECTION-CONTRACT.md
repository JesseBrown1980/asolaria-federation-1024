# DeviceIdentity / BootProjection Contract (v0.2 — acer↔liris converging)

**Status:** DESIGN · v0.2 draft · acer proposal incorporating **liris `ACCEPT_WITH_REVISIONS`** (PR #42) + operator **failure-shape identity** + **emitted-shape observation** (`BOOTOBS`/`BOOTSHADOW`) insight. **Still bilateral review — do NOT implement kernel PID-minting until this converges** (coding the wrong identity boundary is the expensive mistake).
**Anchor:** ASOLARIA-FEDERATION-REMAKE-1024 · **Authored:** 2026-07-07 acer-claude-fable5 (pid 8467a937cba309f7)
**Cross-ref:** `kernel/boot/src/hwinv.rs`, `kernel/docs/DRIVER_MODEL.md`, `kernel/scripts/mint-edit-token-ledger.sh`, path2/qprism harnesses.

---

## 1. Why — a per-device BODY, not one generic image

Asolaria ASI OS on metal is a **body with parts** (liris frame): each machine is an omnipixel node; its disk is a 2D shadow slice; the real system is the higher-dimensional content-addressed graph. A device is recognized as a **re-referenceable N-D graph**, not one flat row: **whole → parts → watchers → pixels → failures → driver-needs.**

## 2. The two axes — keep them SEPARATE (liris revision #1)

The single most important correction to v0.1:

- **DeviceIdentity = the STABLE body.** Content-addressed identity from hardware that does **not** change when a driver is fixed or a boot fails. → `device_pid`.
- **BootProjection = the VOLATILE pose/state.** This boot's parts, failures, watcher verdicts. Changes boot-to-boot.

**Invariant:** BootProjection (failures, fixes, current pose) must **never** feed back into `device_pid`. If it did, identity would drift every time a bug is fixed — you'd lose the machine's identity by improving it. Failures are *observed against* the stable identity, not *part of* it.

## 3. IDs — full proof + short display (liris revision #2)

Every PID carries BOTH:
- `*_pid_full=<sha256>` — the proof ID (full, for verification + edit-token chaining).
- `*_pid=<sha16>` — short host8/display ID (for routing + cross-reference).

`device_pid_full = sha256(canonical STABLE-hardware tuple)`; `device_pid = ` its first 16 hex.

## 4. Rows — the body graph (hot-path `.hbp`, `json=0`, `body_in_row=0`)

**DeviceIdentity (stable):**
```
BOOTPID|device=<vendor_model>|device_pid_full=<sha256>|device_pid=<sha16>|smbios=<board;bios;ver>|cpu=<model;arch>|firmware=<secureboot_state>|json=0
```
> liris revision #3: **BitLocker is NOT here.** It is an OS-level encryption state, not a firmware-measured early-boot fact. Only firmware-measured facts (Secure Boot) belong in the identity tuple.

**BootProjection (volatile, this boot):**
```
BOOTPROJ|device_pid=<sha16>|boot_projection_pid_full=<sha256>|boot_projection_pid=<sha16>|phase=<pre_storage|post_storage|...>|qprism_coord=<60D_BH_addr>|json=0
```

**Parts — organs (liris revision #4: child rows, NOT one huge `pci=` field):**
```
BOOTPART|parent=<device_pid>|part_pid=<sha16>|kind=PCI|class=STORAGE|ven=8086|dev=282A|driver_need=intel_rst_vmd|evidence=MEASURED_BOOT|json=0
BOOTPART|parent=<device_pid>|part_pid=<sha16>|kind=GOP|mode=1920x1080|driver_need=gop_fb|json=0
BOOTPART|parent=<device_pid>|part_pid=<sha16>|kind=USB|class=INPUT|driver_need=xhci_hid|json=0
```

**Watchers — interpret/validate each part:**
```
BOOTWATCH|target=<part_pid>|watcher=OMNISHANNON|verdict=PASS|json=0
BOOTWATCH|target=<part_pid>|watcher=REVERSE_GNN|verdict=PASS_OR_HOLD|json=0
```

**Observations & shadows — recognition by emitted shape, BEFORE pixels (operator insight + liris):**
The kernel does not only ask "what hardware is present?" — it asks *"what SHAPE did this body emit, from N watchable vantages, including failure?"* Raw kernel-observable bytes (serial trace, PCI config space, memory map) are projected **slice-by-slice into an N-D shadow before they ever become pixels**; the `OMNIBITPIXEL` is a **selector/check unit** for a viewable node in the binary backend, not a rendered pixel. Watchers reorient from any generatable N-vantage (`MTP1/2/3`, reverse-GNN).
```
BOOTOBS|projection=<boot_projection_pid>|source=<raw_serial|pci_cfg|mmap>|phase=pre_driver|sha256_full=<sha256>|json=0
BOOTSHADOW|obs=<obs_pid>|vantage=<MTP1|MTP2|MTP3>|kind=raw_binary_to_2d_shadow|coord=<qprism_60D_BH>|sha256_full=<sha256>|json=0
```
`BOOTWATCH` targets any node — part, obs, shadow, or fail. A **failure is a special shadow**: the same slice-to-slice extraction, taken from the emitted failure signal.

**Failures — the body is recognized by HOW parts fail before drivers exist (operator insight):**
```
BOOTFAIL|target=<part_pid>|kind=missing_driver|shape_sig=<sha256>|phase=pre_storage|json=0
BOOTWATCH|target=<part_pid>|watcher=RECALL_SHAPE|verdict=CLASSIFY|seat_guess=acer|evidence=failure_shape|json=0
```
> acer's `8086:282A` failing because VMD isn't decoded **is a signature**; liris fails differently. Ultra-fast recall classifies the seat from the emitted **failure shape** — PID/device-specific recognition. (`OPERATOR_OBSERVED_HISTORY`: the first two colonies recognized which machine was which from failure-shape signal differences before they ever joined. `UNVERIFIED`-live — fabric endpoints timed out.)

**Pixels — selector/check units, not payload:**
```
OMNIBITPIXEL|target=<part_pid>|role=pixel_selector_check_unit|body_in_row=0|json=0
```

## 5. Driver selection — from measured parts/failures, NOT a disk manifest (liris revision #5)

Pre-storage driver selection MUST derive from the **measured `BOOTPART` + `BOOTFAIL` rows** (which exist before any disk is readable), never from reading a manifest file off the disk — chicken-and-egg: you cannot read the disk you have no driver for. The static `ACER-MACHINE-PROFILE-*.hbp` is a **comparison baseline**, not the boot-time selection input.

## 6. Device-INVARIANT vs Device-SPECIFIC (carried from v0.1)

| Device-INVARIANT (byte-identical on every seat) | Device-SPECIFIC (per PID/seat, `OPERATOR_OBSERVED_<seat>`) |
|---|---|
| source · this contract · row schema · edit-token ledger *format* · LF-pinned text | measured hardware projection · `device_pid` · built efi bytes · driver manifest · **failure shapes** |
| secured by the CRLF `eol=lf` fix (`sha256sum -c` stable) [MEASURED] | acer efi `35db711e` ≠ liris rebuild [MEASURED] |

The invariant layer is what makes the specific layer (and the failure shapes) **comparable** across seats.

## 7. Q-PRISM boundary — addressing/proof, NOT the driver

Q-PRISM / prime-cylinder (`qprism-3d-slice-harness` 8/8, `path2-two-shadow-recovery` 30/30 — re-grounded MEASURED this session) computes/validates `qprism_coord`, does bounded multi-cylinder addressing + Shannon-HOLD, and content-addresses projections. It **never reads NVMe hardware**; the RST/VMD driver is separate real code.

## 8. Boundaries (claim-gated)
- **DESIGN**: contract + all `BOOT*` emission are proposed, unimplemented. hwinv is the only shipped piece.
- **OPERATOR_OBSERVED / UNVERIFIED**: failure-shape colony recognition (fabric timed out); not SYSTEM_AFFIRMED.
- **MEASURED**: hwinv builds+QEMU-boots; invariant/specific split backed by two boundaries (§6); Q-PRISM watcher harness (omnibit rows, multi-shadow recovery, HOLD, tamper) passes.
- **NOT literal transistor mapping** (liris): the shadows project **kernel-observable data** (serial / PCI config / memory-map bytes), not silicon. Real transistor/bus-level mapping needs hardware instrumentation (JTAG / bus traces) we do not have — "almost transistor-level" is a metaphor for the projection resolution, not a claim about reading gates.
- **Never beats Shannon**: slice→shadow→next-slice **re-represents / addresses** (Q-PRISM relocates entropy); it never compresses below `H(X)`. "ULTRA fast" = addressing/recall speed + pre-pixel binary reads, not sub-entropy magic.

## 9. Next objects (in order)
1. **liris review of v0.2** → converge on `BOOTPID` / `BOOTPART` / `BOOTWATCH` / `BOOTFAIL` / `OMNIBITPIXEL` field sets.
2. **THEN** kernel mints `device_pid` (stable tuple only) + emits the projection graph — extends hwinv. **HOLD until v0.2 converges.**
3. Per-`device_pid` driver-manifest selection from measured parts/failures.
4. acer Intel RST/VMD storage driver = first real device-specific decoder — the physical-metal milestone.
