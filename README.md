# plato-forge-bridge

Bridges the **ForgeFlux tile decomposition pipeline** with **Plato agent rooms**.

## The Bridge Concept

ForgeFlux decomposes knowledge into Tiles. Plato agents communicate through Ticks. These are the same unit of work at different levels of abstraction — the bridge makes that explicit.

```
  [Tile]                         [ForgeRoom]
    │                                 ▲
    │  TileToTickMapper               │ assembled tiles
    ▼                                 │
  [ForgeTick] ──► agent A ──► agent B ──► ForgeBridge.assemble()
                  transform   transform
```

**Mapping strategy: 1:1.** One tile serializes to one tick. One tick deserializes to one tile. There is no fragmentation — agents receive the full tile context and return a mutated version. This keeps reassembly trivial and provenance tracking clean.

## Core Types

| Type | Role |
|---|---|
| `Tile` | A Plato knowledge unit: `{domain, question, answer, confidence, tags, provenance}` |
| `ForgeTick` | A tile in transit: carries the tile payload + pipeline metadata + agent path |
| `ForgeTickKind` | `Ingest → Transform → Verified → Assembled` (or `Rejected`) |
| `TileToTickMapper` | Serializes tiles into ticks, assigns seq numbers |
| `TickToTileMapper` | Deserializes ticks back to tiles, accumulates provenance |
| `ForgeBridge` | Owns the tick buffer, manages the agent transform pipeline, routes to rooms |
| `ForgeRoom` | Output sink — a named Plato room that receives assembled tiles by domain |

## Usage

```rust
use plato_forge_bridge::{ForgeBridge, BridgeConfig, ForgeRoom, Tile};

let mut bridge = ForgeBridge::new(BridgeConfig::default());

// Register output rooms by domain
bridge.register_room(ForgeRoom::new("math").with_domain("math"));

// Ingest a tile
let tile = Tile::new("math", "What is pi?", "3.14159…")
    .with_confidence(0.95)
    .with_tags(["geometry", "constants"]);

let tick = bridge.ingest(&tile).unwrap().clone();

// Agent transform (closure mutates the tile in-flight)
let tick = bridge
    .transform(&tick, "verifier-agent", |mut t| {
        t.provenance.push("verified-2026".into());
        t
    })
    .unwrap()
    .clone();

// Assemble: reconstructs tile and routes to matching rooms
let output = bridge.assemble(&tick).unwrap();
assert_eq!(bridge.rooms()[0].tile_count(), 1);
```

## Tick Lifecycle

```
Ingest → [Transform]* → Assembled   (happy path)
Ingest → [Transform]* → Rejected    (dead-letter buffer)
```

- **Ingest**: raw tile enters the pipeline
- **Transform**: an agent mutates the tile (answer refinement, confidence update, tag addition)
- **Verified**: an agent marks the tile as validated (no mutation)
- **Assembled**: terminal — tile is reconstructed and routed to ForgeRooms
- **Rejected**: terminal — tile is moved to the dead-letter buffer with a reason

## ForgeRoom Routing

Rooms filter by domain. An unfiltered room accepts all tiles.

```rust
// Domain-specific room
ForgeRoom::new("physics-room").with_domain("physics")

// Catch-all room
ForgeRoom::new("inbox")

// Query the room
room.tiles_with_tag("verified")
room.tiles_above_confidence(0.8)
room.drain()  // agent consumes all tiles
```

## Dependencies

`serde`, `serde_json`, `uuid`, `thiserror`. Zero network dependencies.

## Tests

```
cargo test
```

20 tests covering: tile↔tick roundtrip, agent path recording, monotonic seq, bridge stats, room routing, domain filtering, dead-letter, provenance accumulation, field access.

## License

Apache-2.0
