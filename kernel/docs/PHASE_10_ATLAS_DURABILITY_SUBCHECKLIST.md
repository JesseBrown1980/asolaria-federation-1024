# ATLAS DURABILITY ANCHOR VERIFICATION (Phase-8 step 156 sibling)

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11` · **Fingerprint:** `0xe00b1a465d6dcb50`
**Phase:** 10 · Step 185 (phase-8 step-156 sibling sub-check)
**Cycle coverage:** cycle-77 (`C:/asolaria-acer/tools/usb-raw/usb_raw_io.py` raw block-device I/O GO) + cycle-79 (disk2 dumps atlased to cp 257 `gaia-SOVLINUX-USB-axiom` per `ATLAS_INTEGRATION.md` §"Operator-class reserves")
**Precondition:** Phase-8 step 156 10K envelope bench GREEN

Operator-witness runs all 5 steps in order before `v1.0.0` tag. All 5 must PASS.
---

### Step 1 — Atlas index line-count integrity
```bash
wc -l D:/asolaria-whiteroom/behcs-1024-atlas/atlas-index.ndjson
```
**Expected:** `1024 D:/asolaria-whiteroom/behcs-1024-atlas/atlas-index.ndjson`
**PASS:** line count == 1024 (one per codepoint; matches `ATLAS_INTEGRATION.md` §"Atlas surface").
**FAIL:** count < 1024 or > 1024 (index truncated or has duplicates).

### Step 2 — cp 257 glyph ↔ atlas-index cross-reference
```bash
python -c "
import json
with open(r'D:/asolaria-whiteroom/behcs-1024-atlas/atlas-index.ndjson') as f:
    for line in f:
        o = json.loads(line)
        if o['cp'] == 257:
            print(json.dumps(o, indent=2))
            break
"
```
**Expected:**
```
{
  "cp": 257,
  "name": "gaia-SOVLINUX-USB-axiom",
  "domain": "sovereignty",
  "supervisor": "gaia",
  "source": "operator-anchored-reserve"
}
```
**PASS:** fields at cp 257 match `D:/asolaria-whiteroom/behcs-1024-atlas/glyphs/cp257.mjs` lines 5-7 (name=`gaia-SOVLINUX-USB-axiom`, domain=`sovereignty`, supervisor=`gaia`) and `mint.py` line 23.
**FAIL:** any field divergence or cp 257 missing from index.

### Step 3 — Cycle-77/79 raw-disk durability anchor (PHYSICALDRIVE2 sector 0)
```bash
python C:/asolaria-acer/tools/usb-raw/usb_raw_io.py --read 0 --device \\.\PHYSICALDRIVE2
```
**Expected:**
```
device       : \\.\PHYSICALDRIVE2
sector       : 0
size         : 512 bytes
sha256       : <64-char hex hash, must match cycle-79 dump anchor>
sha16        : <first 16 hex chars of sha256>
boot sig     : 0x55AA valid=True
partition table:
  1 0x80 0x07 NTFS/exFAT/HPFS  ...
```
**PASS criteria (revised cycle-80 per drift-detection finding envelope `:128302`):**
- sector 0 readable + 0x55AA boot sig valid
- Partition 1 = Type 0x07 EXFAT, lba_start 2048, ~500GB
- **EITHER** sha256 matches cycle-79 disk2_first1MB.bin first-512 (sha16=`acc60d6e1df7f682`) = USB untouched since historical dump
- **OR** first ~446 bytes are all-zero AND partition table bytes 0x1BE-0x1FD intact AND boot sig 0x55AA = canonical intentional anti-access boot-code wipe (sha16=`3126770d103a3bed` is the current canon state)
**FAIL:** WinError 5 (ACCESS DENIED — operator not UAC-elevated), or WinError 2 (device `\\.\PHYSICALDRIVE2` absent), or partition table corrupted, or 0x55AA boot sig absent, or sha16 is neither the historical-intact nor the canonical-wiped value.

### Step 4 — Sovereignty band operator-reserve cp-range integrity (cp 256–264)
```bash
python -c "
import json
reserve_cps = set(range(256, 265))
with open(r'D:/asolaria-whiteroom/behcs-1024-atlas/atlas-index.ndjson') as f:
    for line in f:
        o = json.loads(line)
        if o['cp'] in reserve_cps:
            print(f\"cp={o['cp']:>3d}  domain={o['domain']:<15s}  sup={o['supervisor']:<6s}  name={o['name']}\")
"
```
**Expected:**
```
cp=256  domain=sovereignty    sup=gaia    name=gaia-vault-master-key-anchor
cp=257  domain=sovereignty    sup=gaia    name=gaia-SOVLINUX-USB-axiom
cp=258  domain=sovereignty    sup=gaia    name=gaia-LAW-001-authority-kernel
cp=259  domain=sovereignty    sup=gaia    name=gaia-quintuple-cosign-window
cp=260  domain=sovereignty    sup=gaia    name=gaia-Jesse-Daniel-Brown-apex
cp=261  domain=sovereignty    sup=gaia    name=gaia-Rayssa-Chiqueto-apex
cp=262  domain=sovereignty    sup=gaia    name=gaia-Amy-witness
cp=263  domain=sovereignty    sup=gaia    name=gaia-Dan-witness
cp=264  domain=sovereignty    sup=gaia    name=gaia-Felipe-witness
```
**PASS:** all 9 operator-class reserves (cp 256–264) have `domain=sovereignty` and `supervisor=gaia`, matching `ATLAS_INTEGRATION.md` §"Operator-class reserves" table and `D:/asolaria-whiteroom/behcs-1024-atlas/mint.py` RESERVES dict lines 23-25.
**FAIL:** any cp missing from index, incorrect domain, or supervisor not `gaia`.

### Step 5 — Kernel pid/mod.rs atlas cross-reference freshness
```bash
grep -nE 'atlas|Atlas|asolaria-whiteroom|sovereignty' C:/asolaria-acer/federation-remake-1024/kernel/core/src/pid/mod.rs
```
**Expected:** Line 107 contains `"DOMAIN axis (cycle-68): Atlas at D:/asolaria-whiteroom/behcs-1024-atlas/"` and line 111 cites `kernel/docs/ATLAS_INTEGRATION.md`.
**PASS:** kernel pid/mod.rs doc-comment (lines 107-112) references atlas as the canonical DOMAIN axis source-of-truth with the correct 8-domain map (`legacy_subset_256`, `sovereignty`, `sequencing`, `language_glyph`, `hilbert_cube`, `build_proof`, `federation`, `emergent_residual`) matching `ATLAS_INTEGRATION.md` §"Domain map".
**FAIL:** zero matches (kernel source stale, no atlas cross-ref), or domain list diverges from index.

---

**Gate:** All 5 PASS → atlas durability anchor verified. Cosign-row emitted to `xe-execute-2026-05-11/PHASE_10_STEP_185_ATLAS_DURABILITY_RESULT.behcs-256.json`. Any FAIL → block `v1.0.0` tag; re-run cycle-79 disk2 dump procedure, update atlas glyph cp 257 if sector content drifted, and re-verify.
