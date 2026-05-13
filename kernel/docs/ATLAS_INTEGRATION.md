# Atlas Integration · BEHCS-1024 Canonical Codepoint Map

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Atlas path:** `D:/asolaria-whiteroom/behcs-1024-atlas/`
**Atlas SEALED status:** per Foundation v2 LAW (memory `project_asolaria_foundation_v2_LAW.md`) — quintuple cosign IN-WRITING through 2-week window
**Authored:** 2026-05-11 cycle-68 by acer-claude per operator directive "make sure we are using atlas"

---

## Atlas surface (verified cycle-68)

| Artifact | Path | Contents |
|---|---|---|
| Atlas index | `atlas-index.ndjson` | 1024 lines, one per codepoint (cp 0-1023), each `{cp, name, domain, supervisor, source}` |
| Glyph definitions | `glyphs/cp*.mjs` | 1024 ES modules, one per cp (also exists for legacy cp 0-255 from BEHCS-256 subset) |
| Minter | `mint.py` | Generator script with `get_domain(cp)` + `SUP` + `LANE` + `RESERVES` maps |

## Domain map (8 ranges)

| cp range | domain | supervisor | lane |
|---|---|---|---|
| 0-255 | `legacy_subset_256` | cube_cubed_sealer | memory |
| 256-383 | `sovereignty` | gaia | skeletal |
| 384-479 | `sequencing` | helm | muscular |
| 480-575 | `language_glyph` | vector | nervous |
| 576-703 | `hilbert_cube` | rook | circulatory |
| 704-799 | `build_proof` | forge | muscular |
| 800-895 | `federation` | falcon | nervous |
| 896-1023 | `emergent_residual` | livefree | memory |

## Operator-class reserves (sample)

| cp | role |
|---|---|
| 256 | `gaia-vault-master-key-anchor` |
| 257 | `gaia-SOVLINUX-USB-axiom` |
| 258 | `gaia-LAW-001-authority-kernel` |
| 259 | `gaia-quintuple-cosign-window` |
| **260** | **`gaia-Jesse-Daniel-Brown-apex`** |
| **261** | **`gaia-Rayssa-Chiqueto-apex`** |
| 262 | `gaia-Amy-witness` |
| 263 | `gaia-Dan-witness` |
| 264 | `gaia-Felipe-witness` |

Full reserves continue through the `sovereignty` band (cp 256-383). See `mint.py` `RESERVES` dict for canonical list.

## Atlas vs aether v3 5-subclass — orthogonal axes

The 5-subclass FORM taxonomy in `kernel/core/src/pid/mod.rs` (`Regular`, `RegularExtended`, `Anchor`, `HookwallCp`, `InfrastructureRouting`) classifies PIDs by **textual shape**. Atlas classifies by **codepoint domain**. These are **orthogonal axes**:

| Axis | What it classifies | Source-of-truth |
|---|---|---|
| FORM axis (aether v3) | PID string shape (suffix presence, date vs hex tail, etc.) | `kernel/core/src/pid/mod.rs` `classify_subclass` |
| DOMAIN axis (atlas) | Codepoint sovereignty role (which supervisor/lane owns it) | `D:/asolaria-whiteroom/behcs-1024-atlas/atlas-index.ndjson` |

A given PID has both a FORM class AND a DOMAIN class. Examples:
- `OP-JESSE-PID-G0000-A00-W000-P00-N00000` → FORM=`RegularExtended`, DOMAIN=cp 260 `gaia-Jesse-Daniel-Brown-apex` (`sovereignty`/`gaia`/`skeletal`)
- `ACER-PID-H740C-A07-W104-P00-N00000` → FORM=`RegularExtended`, DOMAIN=cp range for ACER resident anchor (federation band cp 800-895 likely)
- `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11` → FORM=`Anchor`, DOMAIN=project-anchor (not a single cp)

This was the option-B dual taxonomy liris flagged at cycle-49: keep aether v3 FORM names AND add atlas DOMAIN as a parallel classifier. Acer cycle-47 took option-A (FORM only, dropped L1/L2 hookwall-domain naming) — option-B is a future extension when bandwidth permits.

## Implementation status (cycle-68)

- **Atlas index file** present + 1024 cp populated ✓
- **Atlas glyphs/** present + 1024 .mjs files ✓
- **`mint.py` source-of-truth** present ✓
- **Acer Rust kernel `kernel/core/src/atlas/` module** — **NOT YET CREATED**; would load `atlas-index.ndjson` at boot for cp→domain lookup
- **Acer kernel doc cross-references** — this doc establishes the link; PHASE_3_WIRING_STATUS / PHASE_10_SHIP_CHECKLIST should cite atlas as canonical when minting/validating PIDs that carry cp-prefixes (HookwallCp subclass)

## Next-step: when atlas module gets wired

Phase-9 `HookwallCp` subclass classifier will need atlas to dispatch concretely (since cp range → domain → tier mapping comes from atlas, not from PID shape alone). Until then, `pid::classify_subclass` returns `Pending` for cp-prefix PIDs.

A minimal acer-side atlas module would expose:
```rust
pub fn domain_for_cp(cp: u16) -> AtlasDomain
pub fn supervisor_for_cp(cp: u16) -> AtlasSupervisor
pub fn lane_for_cp(cp: u16) -> AtlasLane
pub fn reserved_role_for_cp(cp: u16) -> Option<&'static str>
```

These tables can be code-generated from `atlas-index.ndjson` at build time.

## Cosign-chain participation

Per Foundation v2 LAW, atlas is SEALED with quintuple cosign IN-WRITING. Any modification to atlas (adding cp, changing domain assignment, reassigning reserve roles) requires the same 5-cosigner ratification as canonical PID-shape changes. Cosign-chain row `seq=49` per memory marked atlas BEHCS-1024 generation event.

## Cycle-68 verification checklist

- [x] Atlas path exists + readable from acer vantage
- [x] Atlas index has 1024 cp populated
- [x] Atlas-vs-FORM axis orthogonality documented
- [x] Operator-class reserves enumerated (Jesse cp 260, Rayssa cp 261)
- [ ] Acer Rust `atlas` module created (deferred; Phase-9 prerequisite)
- [ ] PHASE_3_WIRING_STATUS / PHASE_10_SHIP_CHECKLIST updated to cite atlas (next cycle)
- [ ] `kernel/core/src/pid/mod.rs` HookwallCp classifier wired to atlas (deferred to Phase-9)
