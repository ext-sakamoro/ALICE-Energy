# ALICE-Energy

Deterministic power grid simulation, phase synchronization, and battery degradation prediction.

## Modules

| Module | Description |
|--------|-------------|
| `battery` | Battery degradation model, state-of-charge, cycle life prediction |
| `contingency` | N-1 contingency analysis for grid reliability |
| `dispatch` | Economic dispatch optimization |
| `facts` | Flexible AC Transmission Systems (FACTS) devices |
| `grid` | Power grid topology, transmission lines |
| `load_flow` | Newton-Raphson / DC load flow solver |
| `node` | Bus/node types (PQ, PV, slack) |
| `phase` | Phase synchronization, Kuramoto model |
| `renewable` | Solar/wind capacity factor, output estimation |
| `stability` | Voltage/frequency stability analysis |

## Example

```rust
use alice_energy::{PowerGrid, PowerNode, NodeKind, economic_dispatch};

// Create grid
let mut grid = PowerGrid::new();
let bus = PowerNode::new(NodeKind::Pq, 100.0, 50.0);

// Renewable output
let solar = alice_energy::solar_output(&panel, irradiance, temp);
let wind = alice_energy::wind_output(&turbine, wind_speed);
```

## Quality

| Metric | Value |
|--------|-------|
| clippy (pedantic+nursery) | 0 warnings |
| Tests | 165 |
| fmt | clean |

## License

`AGPL-3.0-only OR LicenseRef-Commercial` — dual-licensed. Pick either.

| Option | Terms | Use it when |
|--------|-------|-------------|
| **AGPL-3.0-only** | [LICENSE-AGPL](LICENSE-AGPL) — free, no reporting obligation | Your project is itself AGPL-compatible open source, or you are only using it internally |
| **Commercial License** | [LICENSE-COMMERCIAL.md](LICENSE-COMMERCIAL.md) — paid, removes the copyleft | Closed-source product, proprietary SaaS, edge / firmware distribution, plugin redistribution, or a platform NDA that forbids source disclosure |

AGPL is a strong copyleft: a product, firmware image, or service that links
`alice-energy` and is distributed or served to users must be released under the AGPL
as well. That is intentional for the open ecosystem, and the Commercial
License exists for the cases where it is not something you are able to do.

Commercial licence enquiries: <contact@extoria.co.jp>
