// plato-forge-bridge
//
// Bridges ForgeFlux (tile decomposition pipeline) with Plato agent rooms.
// Mapping strategy: 1:1 — one Tile maps to one ForgeTick and back.
// Tiles carry their full payload through the tick layer; agents process the
// JSON envelope and may produce a modified tick that reassembles cleanly.

pub mod types;
pub mod mapper;
pub mod bridge;
pub mod room;

pub use types::{Tile, ForgeTick, ForgeTickKind, TileMetadata};
pub use mapper::{TileToTickMapper, TickToTileMapper};
pub use bridge::{ForgeBridge, BridgeConfig, BridgeStats};
pub use room::ForgeRoom;

mod tests;
