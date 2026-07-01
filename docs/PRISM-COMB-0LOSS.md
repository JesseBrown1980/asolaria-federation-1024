# Prism/Comb 0-loss — the 1024 alphabet as comb teeth (2026-07-01)

**Scope:** docs-only, E=0 — describes the law, fires nothing. Claims tagged MEASURED / CANON / UNVERIFIED.

## The law, stated for THIS repo

BEHCS-1024 is a base-2^10 alphabet: **the 1,024 glyph values are the comb teeth**. Like a frequency
comb (`f_n = n*f_rep + f_ceo`, integer-linear, exact), the glyph lattice joins alphabets `2^q` by
exact bit-count conservation at lcm boundaries. Every such re-relation is a **bijection**, and entropy
is invariant under bijection (`H(f(X)) = H(X)`): the kernel re-relates information with **0 loss and
never claims compression below entropy** (`E[bits] >= H(X)` stands).

## The MEASURED rung: this repo's bridge to BEHCS-256

MAP.md names `asolaria-behcs-256` the bridge stratum below this kernel ("old decodes new"). That
bridge is a proven theorem — **MEASURED** (Q-PRISM `53023b6`; cross-links `79e8d63`, `de00aca`):
bytes are base-2^8 digits, glyphs base-2^10 digits of the SAME integer `N`
(`s_j = floor(N / 1024^(m-1-j)) mod 1024`); exact packing at `lcm(8,10) = 40` bits gives
**5 bytes <-> 4 glyphs**, remainder 0. Round trip `transcode(1024->256) o transcode(256->1024) = id`,
sha256-identical, **Rust==Python symbol-identical** — load-bearing here, where the Rust Host-8 lane
must agree with Node-era artifacts during map-while-upgrading. Code rate exactly 1.0.

## Scoped frame around the one proof

- **CANON frame:** the 43+ level ladder is a groupoid (`T_ji o T_ij = id`, `T_jk o T_ij = T_ik`) —
  translation omnidirectional, path-independent. Only the 256<->1024 rung is MEASURED; **every other
  rung earns MEASURED only by its own round-trip proof** (UNVERIFIED until then).
- **Honest bound for the 8-byte host:** `handle8 = sha256(content)[:8]` is a **coordinate against the
  content-addressed store** (`H(content | store) = 0`; birthday bound `~ M^2 / 2^65`). Referential
  cubes = infinite ADDRESSING capacity, **not** lossless infinite compression.
- **Integrity dual (cosign-ledger / vote-quorum):** verification = recomputation = the inverse map;
  `reported == recomputed over children` is the groupoid coherence check at every level — a fabricated
  signal cannot reach consent, the same way a lossy step cannot hide in a bijection chain.

**Boundary line:** the prism relates information perfectly; it never creates or destroys it — no
bijection beats Shannon; the hash store relocates entropy and names it. Duality with waves-cascades:
forward comb = collision-avoidance; backward prism = many->1 search. Materialization stays operator-gated (E=0).
