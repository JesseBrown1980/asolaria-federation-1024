# OmniSCRPY Orbital Link

OmniSCRPY now has a USB-free orbital registration path for visual/control
surfaces. A mobile endpoint calls the fabric over the trusted LAN and emits
HBP/HBI tuple receipts that dashboards, PID supervisors, visual maps, and agent
teams can ingest.

This is a registrar and receipt layer, not a secret-key tool and not an
execution engine.

## Current Falcon Evidence

Evidence class: `OPERATOR_OBSERVED_ACER`

- Falcon is reachable from Acer over WiFi at `http://192.168.1.6:8789`.
- Falcon omnicoder served `0.2.4-shannon-hardened`.
- Falcon host PID8 observed by Acer: `4c7e27b6bfb76666`.
- Falcon called Acer bus `http://192.168.1.9:4947/behcs/send` successfully.
- Acer inbox contained Falcon/omnicoder markers.
- Runtime USB dependency after orbital launch: `0`.

Liris-to-Falcon attach remains a separate pending lane until Liris performs its
own attach/attack and emits receipts.

## Register A Link

```bash
node tools/omniscrcpy/omniscrcpy-orbital-link.mjs
```

The command writes:

- `tools/omniscrcpy/broadcasts/orbital/latest-falcon-orbital.hbp`
- a timestamped `.hbp` receipt

JSON mirrors are diagnostics only. If a cold validator needs one, make it
explicit:

```bash
node tools/omniscrcpy/omniscrcpy-orbital-link.mjs --json-out
```

Use `--post` only when the target bus is intentionally reachable from the seat
running the command. It posts the HBP rows as `text/plain`:

```bash
node tools/omniscrcpy/omniscrcpy-orbital-link.mjs --post
```

Use `--post-json` only for a compatibility endpoint that cannot accept tuple
text.

Override any endpoint without changing the source:

```bash
node tools/omniscrcpy/omniscrcpy-orbital-link.mjs \
  --from falcon \
  --to acer \
  --endpoint http://192.168.1.6:8789 \
  --bus http://192.168.1.9:4947/behcs/send \
  --recall http://192.168.1.9:4796 \
  --host-pid8 4c7e27b6bfb76666
```

## Tuple Classes

The generated `.hbp` carries five rows:

- `ORBITALREG` - link registration
- `ORBITALENDPOINT` - LAN endpoint and bus target
- `HBIHOTPATH` - Hilbra/recall hot-path contract boundary
- `VISPIDSUP` - visual supervisor and PID registration pointers
- `MAPADD` - unified fabric map ingest hint

All rows keep the HyperBEHCS selector frame explicit:

```text
tuple_dim=60|levels=16|catalogs=HILBRA,HBI,ASOLARIA_ATLAS_RECALL,OMNISCRPY,VIS_SUPERVISORS,PID_ROSTER,ORBITAL_ENDPOINT,SHANNON_RECEIPT,GAC,AGENT_TEAMS
```

## Key And PII Boundary

The orbital registrar publishes public handles only. It must not publish:

- private keys
- vault paths
- device serials
- owner-private recall content

If the backend has an existing Falcon enrollment, this tool records the public
link shape and delegates key approval to that backend. If Liris later attaches
Falcon through the same method, Liris emits a new receipt rather than copying
Acer secrets.

## Dashboard Ingest

The generated `MAPADD` and `VISPIDSUP` rows are intended for:

- Asolaria unified fabric map
- Hilbra atlas/recall views
- PID roster folds
- visual supervisors
- agent-team dashboards

The generated map hint is a data edit. It does not rewrite the large generated
HTML map directly.
