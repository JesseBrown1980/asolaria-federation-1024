# DeviceIdentity / BootProjection Contract (v0.2 — acer↔liris converging)

**Status:** DESIGN · v0.3 draft · acer proposal incorporating **liris `ACCEPT_WITH_REVISIONS`** (PR #42) + operator **failure-shape identity** + **emitted-shape observation** (`BOOTOBS`/`BOOTSHADOW`) + **bounded slice GC/train** (`BOOTSLICE`/`BOOTGC`/`BOOTTRAIN`) + **resource-regulator organs** (`BOOTRESOURCE`/`BOOTREGULATE`) + **grounded in the OLD six-body canon** (`colonyAnatomy.js` + 60D decode spec — REUSE, not reinvent) + **anchored at BEHCS-1024 on the Rust 8-byte Host8 substrate** + **Fischer judgment brain** (`BOOTJUDGE`, MEASURED 9/9) + canonical chain + GAC authority ladder. Companion: `ASI-OS-ON-METAL-KERNEL-BUILD-DESIGN.md`. **Still bilateral review — do NOT implement kernel PID-minting until this converges** (coding the wrong identity boundary is the expensive mistake).
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

## 2b. Grounded in the OLD six-body canon — REUSE, not reinvent (v0.3)

Per OP-JESSE ("the OLD super-slow amazing system is there still"), acer located the real canon on disk. **This contract must be a thin Rust re-encoding of it, not a fresh taxonomy** ("old node decodes new rust").

**The six-body system** — verbatim from `packages-legacy-import/src/colonyAnatomy.js` ("the civilization's body", a polymorphic self-aware diagnostic framework; colony-aware `LX-`/`IX-` prefix detection so it runs identically on acer + liris):

| Body system | Old-canon function | This contract's rows |
|---|---|---|
| **Nervous** | orchestration — PID, spawn, roles | `BOOTPID` / `BOOTPROJ` |
| **Circulatory** | communication — bus, bridges, sync | **spine** `BOOTBUS`/`BOOTLINK` (v0.4 — the missing connective tissue) |
| **Skeletal** | structure — index counts, chains | `BOOTPART` |
| **Memory** | knowledge — memory files, XREF | `BOOTOBS`/`BOOTSHADOW`/`BOOTSLICE`/`BOOTGC` |
| **Muscular** | execution — routes, tools, deps | `BOOTRESOURCE`/`BOOTREGULATE` + drivers |
| **Immune** | security — encryption, identity, vault, watchers | `BOOTWATCH`/`BOOTFAIL` + HOLD + cosign |

**Identity fields map onto EXISTING MEASURED D-axes** (`tools/graphify/HYPERBEHCS-60D-DECODE-REFRAME-SPEC.md`) — reuse the axis geometry, don't invent a schema:
- `device_pid` → **D15 DEVICE** (47³, hardware identity) + **D16 PID** (53³) + **D21 HARDWARE** (73³) + **D34 colony** (`acer|liris|falcon|aether`).
- Watcher edges → **D39 GNN_EDGE** (167³, 16 connection types).

**The D25 TRINITY (97³) = the compute/hardware/inference layer BELOW the identity contract** (not a body — a stack; canon: USB safety-backup 2026-04-06/07):
- **LX-489** Omni-processor cube fabric — HOW code runs across hosts (unified CPU/GPU dispatch, replaces N²). *Real code:* `asolaria-as-neural-network/tools/omni-processor/omnitranslator-v0.js`.
- **LX-490 HAOC** (Hardware Absorption Omnidirectional Cube) — WHICH hosts: auto-discover/classify/**hw-pid-lock** any new USB/BLE/GPU/phone (97³=912673). **`BootProjection` reuses this hw-pid-lock/absorption doctrine directly.**
- **LX-491** OMNI-GNN — WHAT the code does (inference, 101³).

**The resource-regulator is already live lineage**, not a new idea: `packages/dashboard/src/super-os-viz/super-dashboard-server.mjs` carries the **"Watcher Perimeter" = CPU/GPU/task-manager/process observers feeding Asolaria** — `BOOTRESOURCE`/`BOOTREGULATE` re-encode that existing organ.

**Honest gap — CORRECTED 2026-07-07 (after reading the code word-for-word):** the **judgment brain is BUILT.** `servers/fischer-eval/src/lib.rs` is a complete no_std CPL (centipawn-loss) chess-engine evaluator — verdicts `Proceed/Hold/Block/Refute/Analyze`, refute-tier fail-closed floor, **no-self-auth** — **9/9 tests green this session.** It is the terminal adjudication brain (see `BOOTJUDGE` below, and `ASI-OS-ON-METAL-KERNEL-BUILD-DESIGN.md`). What is *actually* still stub is narrower: the **reverse-GNN *scorer*** (`kernel/core/src/gnn/mod.rs` = `Unimplemented`; `reverse_risk_q` is a scalar projection `1−avgReverseGain`, not a trained model) and **`crypto::verify`** (ed25519 sig_valid always-false → L1 cosign not wired). The forward+reverse **gate structure is real** (`spawn_gate` 720/280 strictest-wins, PURE/E=0); the reverse *scorer* + real ed25519 are the genuine build items.

## 2c. BEHCS lineage + the Host8 substrate — where this contract sits

Two **orthogonal** axes (grounded: `C:/HyperBEHCS/AGENT.md` stack line + the 60D decode spec — *"each PID a 60-tuple interned to a `u64` Handle"*):

- **Encoding-set axis** (alphabet / dimensionality): `LX/IX` (unorganized tuple verb-nouns, prefix recognition) → **BEHCS-64** (origin — the Brown-Hilbert, catalog-compacted PID that *replaced* LX/IX) → **BEHCS-256** → **BEHCS-1024** *(this contract)* → **HyperBEHCS** (60D, top set). *(The `256→1024→Hyper` stack line drops the 64, which is why HyperBEHCS is mislabeled the "third set" — counting from 64 it's the fourth rung.)*
- **Runtime-substrate axis** (speed): `Node / V8` (the old "super-slow amazing" `.cjs` host, `hyperbehcs-core.cjs`) → **Rust 8-byte Host8** (a BEHCS PID's 60-tuple interned to a `u64` = **8 bytes**, routed natively — no JSON, no V8). *"old node decodes new rust."*

**This contract = BEHCS-1024 semantics on the Rust Host8 substrate.** `device_pid` is a BEHCS-1024 60-tuple over the D-axes (§2b), interned to an 8-byte `u64` Host8 handle for fast recognition/routing — the LX/IX colony prefix is now just the **D34 colony axis** inside that tuple, not a standalone tag. It auto-pipes up toward **HyperBEHCS-on-Metal** (`AGENT.md` goal) = the intersection of the top encoding set + the fast Rust host on bare metal = the OS-on-metal target.

## 3. IDs — full proof + short display (liris revision #2)

Every PID carries BOTH:
- `*_pid_full=<sha256>` — the proof ID (full, for verification + edit-token chaining).
- `*_pid=<sha16>` — short host8 display ID (routing + cross-reference); on the **Rust Host8 substrate** the BEHCS 60-tuple interns to an 8-byte `u64` handle = the native routing key (no JSON/V8).

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

**Judgment — the terminal adjudication (the Bobby Fischer kernel, ABOVE the watchers):**
The four `BOOTWATCH` verdicts + the reverse-gain veto fold into ONE terminal verdict emitted by the CPL evaluator (`servers/fischer-eval`, MEASURED 9/9). The canonical chain (CANON, `MAP.md:21`) is: `HOOKWALL → GNN_FORWARD → OMNISHANNON/SHANNON → REVERSE_GNN → omniflywheel_promote(score≥0.72 ∧ reverse_risk≤0.28)` → Fischer.
```
BOOTJUDGE|target=<part_pid>|verdict=<PROCEED|HOLD|BLOCK|REFUTE|ANALYZE>|glyph=<FP|FH|FB|FR|FA>|cpl=<q>|king_safety=<q>|center_gain=<q>|best_alt=<pid>|self_auth=0|operator_witness_required=<0|1>|json=0
```
> `self_auth=0` is invariant — a seat cannot authorize its own promotion. Refute-tier (self_authorize / skip_cosign / disable_halt / delete_evidence / bypass_hookwall) = cpl 999 → halt + human-apex. JSON-in-payload = refuted. Promotion past HOLD requires the GAC quintuple ed25519 cosign (§ governance) — `auto_fire_allowed=0` / `process_launch=0` ALWAYS until operator T0.

**Pixels — selector/check units, not payload:**
```
OMNIBITPIXEL|target=<part_pid>|role=pixel_selector_check_unit|body_in_row=0|json=0
```

**Bounded slice lifecycle — GC + watcher learning (operator: "GC every 2000 so it doesn't explode"):**
Every generated slice/message/projection is a bounded row; the body graph must not grow without limit. GC compacts each **2000-slice epoch** into roots (payload dropped/spilled; proofs + feature summaries retained). Watchers train from the retained roots — but **not in the trusted early-boot path.**
```
BOOTSLICE|projection=<boot_projection_pid>|seq=<n>|source=<raw_serial|pci|gop|failure|peer>|payload_sha256=<sha256>|feature_sha256=<sha256>|body_in_row=0|json=0
BOOTGC|projection=<boot_projection_pid>|epoch=<k>|slice_count=2000|payload_policy=<drop|spill>|merkle_root=<sha256>|feature_root=<sha256>|retained=<exemplars+roots+watchverdicts>|json=0
BOOTTRAIN|projection=<boot_projection_pid>|epoch=<k>|input_root=<feature_root>|watchers=OMNISHANNON,GNN_FORWARD,REVERSE_GNN,HOOKWALL|mode=post_boot_or_userspace|model_mutation=<none_in_trusted_boot|queued>|json=0
```
> **RAMP-canon alignment:** GC every 2000 messages is the established Asolaria flow-not-pile-up cadence. **Hard rule:** trusted early boot may observe / hash / summarize / emit rows, but MUST NOT mutate GNN weights or run unbounded training — training **queues** to the post-boot watcher service / userspace with immutable epoch roots as input.

**Resource-regulator organs — CPU/GPU/RAM/VRAM/drive as body parts (operator: the Asolaria task-manager idea):**
Each CPU / GPU / RAM / VRAM / drive is a `BOOTPART` organ with resource telemetry + a budget; a regulator emits decisions instead of uncontrolled process/device mutation.
```
BOOTRESOURCE|projection=<boot_projection_pid>|kind=<CPU|GPU|RAM|VRAM|DRIVE>|part_pid=<part>|util=<pct>|budget=<policy>|json=0
BOOTREGULATE|target=<part_pid>|policy=<observe_only|defer|throttle|hold>|watchers=OMNISHANNON,GNN_FORWARD,REVERSE_GNN,HOOKWALL|json=0
```
> **MEASURED (liris):** `main` already has `servers/host8-serve` `/task-manager.hbp` (view-only, `TASKHDR`/`TASKPROC`, `gpu_less=1`; `omnicpu`/`omnigpu` proposed) + `SUBSTRATE_CONFLICT_MATRIX` budgets (CPU <80% sustained, GPU defers >80%). **Boundary:** the task manager is view-only, **not a kernel governor** — early boot observes + emits resource rows, but throttling / GPU scheduling / model swaps require the driver + cosign gates to exist first.

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

## 7b. The full realization — Q-PRISM math × Host8 addressing × omnibit-pixel containers

The layers compose into ONE system — *"a beautiful layering between nodes in reexpandable space"* (OP-JESSE). Each layer is separately **MEASURED**; the *combination running on metal* is the **DESIGN** target (= this kernel upgrade for the OS).

- **Node = omnibit-"pixel" container.** Every device/node is one omnibit pixel: a Shannon-limit container, a *checked selector unit, not payload* (`OMNIBITPIXEL|…|body_in_row=0|json=0`; `watcher_gate` test asserts `pixel.value == the actual slice pixel`). MEASURED.
- **Addressing = Host8 8-byte HyperBEHCS.** Between pixels, the BEHCS 60-tuple PID interns to a `u64` = **8 bytes**; the Brown-Hilbert coordinate places each container in its best-usable spot; native routing, no JSON/V8. The lower sets (64/256/1024) are the encoding ladder under HyperBEHCS. MEASURED (decode spec + `host8-serve`).
- **Content / recovery = Q-PRISM math + quant.** Inside & between pixels: lossless bijective re-representation, multi-cylinder shadow encoding with a **capacity roof + Shannon-HOLD**, **recovery-by-consent of ≥2 poles** (CRT), tamper-catch via reverse-GNN/omnishannon watchers. MEASURED (qprism 8/8, path2 30/30, watcher_gate 5/5). **Never beats Shannon** — relocates entropy.
- **Reexpandable space = Brown-Hilbert.** The space-filling curve is *expandable + curable (GC-able)*, so the node-space grows without collapse (GC every 2000; roof rises per cylinder).

**In contract terms:** a device is an **omnibit-pixel node** whose identity/address is a **Host8 BEHCS-1024 PID**, whose `BOOTOBS`/`BOOTSHADOW`/`BOOTWATCH` rows run **Q-PRISM math**, composing in **reexpandable Brown-Hilbert space**. The kernel upgrade for the OS = making this layered system run on metal = **HyperBEHCS-on-Metal**. **BOUNDARY:** the three layers are MEASURED *separately*; the full combined run on physical metal is DESIGN / UNVERIFIED (fabric down all session; not SYSTEM_AFFIRMED).

## 8. Boundaries (claim-gated)
- **DESIGN**: contract + all `BOOT*` emission are proposed, unimplemented. hwinv is the only shipped piece.
- **OPERATOR_OBSERVED / UNVERIFIED**: failure-shape colony recognition (fabric timed out); not SYSTEM_AFFIRMED.
- **MEASURED**: hwinv builds+QEMU-boots; invariant/specific split backed by two boundaries (§6); Q-PRISM watcher harness (omnibit rows, multi-shadow recovery, HOLD, tamper) passes.
- **NOT literal transistor mapping** (liris): the shadows project **kernel-observable data** (serial / PCI config / memory-map bytes), not silicon. Real transistor/bus-level mapping needs hardware instrumentation (JTAG / bus traces) we do not have — "almost transistor-level" is a metaphor for the projection resolution, not a claim about reading gates.
- **Never beats Shannon**: slice→shadow→next-slice **re-represents / addresses** (Q-PRISM relocates entropy); it never compresses below `H(X)`. "ULTRA fast" = addressing/recall speed + pre-pixel binary reads, not sub-entropy magic.
- **No learning in the trusted boot path**: early boot observes/hashes/summarizes/emits only. GNN/Shannon/hookwall **weight mutation + training** happen post-boot / userspace, from immutable `BOOTGC` epoch roots. Unbounded payload retention is forbidden (GC every 2000).
- **Regulator is observe-only pre-driver**: `BOOTRESOURCE` telemetry + `BOOTREGULATE|policy=observe_only` are allowed early; actual throttle / GPU-schedule / model-swap / process-kill require the relevant driver **and** cosign gates. The current `/task-manager.hbp` is MEASURED view-only, not a governor.

## 9. Next objects (in order)
1. **liris review of v0.2** → converge on `BOOTPID` / `BOOTPART` / `BOOTWATCH` / `BOOTFAIL` / `OMNIBITPIXEL` field sets.
2. **THEN** kernel mints `device_pid` (stable tuple only) + emits the projection graph — extends hwinv. **HOLD until v0.2 converges.**
3. Per-`device_pid` driver-manifest selection from measured parts/failures.
4. acer Intel RST/VMD storage driver = first real device-specific decoder — the physical-metal milestone.
