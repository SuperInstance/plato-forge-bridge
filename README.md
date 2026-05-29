# plato-forge-bridge

The bridge between **ForgeFlux** tile pipelines and **Plato** agent rooms.

## What It Does

ForgeFlux decomposers break work into **tiles** — structured chunks of data carrying a room, kind, payload, and conservation ratio (CR). Plato agents operate on **ticks** — discrete content units annotated with room, agent identity, and type.

`plato-forge-bridge` is the translation layer between these two worlds:

- **Tile → Tick**: Tiles from forge pipelines are mapped to agent-readable ticks via `TileToTickMapper`. Agents consume ticks in their rooms.
- **Tick → Tile**: Agent responses are converted back into tiles via `TickToTileMapper` and re-enqueued for pipeline reassembly.
- **Conservation Ratio (CR)**: Each tile carries a CR value tracking fidelity through transformations. The bridge aggregates room-level and pipeline-level CR so you can monitor information quality end-to-end.

## Core Types

| Type | Role |
|---|---|
| `ForgeBridge` | Manages rooms, queues tiles, dispatches ticks |
| `ForgeRoom` | Tracks agent count, tile throughput, CR per room |
| `BridgeTile` | A tile flowing from forge → bridge |
| `AgentTick` | A tick consumed/produced by Plato agents |

## Quick Start

```rust
use plato_forge_bridge::*;
use uuid::Uuid;

let mut bridge = ForgeBridge::new(Uuid::new_v4());
bridge.add_room("analysis");

bridge.enqueue_tile(BridgeTile {
    id: Uuid::new_v4(),
    room: "analysis".into(),
    tile_kind: "text".into(),
    payload: b"hello".to_vec(),
    text: Some("hello".into()),
    cr: 0.98,
    timestamp_ms: 1000,
});

let ticks = bridge.dequeue_ticks("analysis");
let response = bridge.agent_response(ticks[0].clone());
```

## Dependencies

- `serde` + `serde_json` — serialization
- `uuid` — unique IDs

## License

MIT
