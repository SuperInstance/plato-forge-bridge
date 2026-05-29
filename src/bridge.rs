use crate::mapper::{MapperError, TickToTileMapper, TileToTickMapper};
use crate::room::ForgeRoom;
use crate::types::{ForgeTick, Tile};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct BridgeConfig {
    pub pipeline_id: Uuid,
    /// Max ticks to buffer before the oldest are dropped
    pub buffer_capacity: usize,
    /// If true, rejected ticks are kept in a dead-letter buffer
    pub keep_rejected: bool,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            pipeline_id: Uuid::new_v4(),
            buffer_capacity: 1024,
            keep_rejected: true,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct BridgeStats {
    pub ticks_ingested: u64,
    pub ticks_assembled: u64,
    pub ticks_rejected: u64,
    pub tiles_routed: u64,
}

// ─── ForgeBridge ─────────────────────────────────────────────────────────────
// The main bridge struct. Owns the tick buffer and routes ticks to ForgeRooms.
//
// Lifecycle:
//   1. ingest(tile)          → emits an Ingest tick into the pipeline
//   2. transform(tick, agent) → agent processes the tick, emits Transform tick
//   3. assemble(tick)         → terminal tick; tile is reconstructed and stored
//
// ForgeRooms are the output side: assembled tiles land in a named room.

pub struct ForgeBridge {
    pub config: BridgeConfig,
    pub stats: BridgeStats,
    to_tick: TileToTickMapper,
    from_tick: TickToTileMapper,
    buffer: Vec<ForgeTick>,
    rejected: Vec<ForgeTick>,
    rooms: Vec<ForgeRoom>,
}

impl ForgeBridge {
    pub fn new(config: BridgeConfig) -> Self {
        let pipeline_id = config.pipeline_id;
        Self {
            to_tick: TileToTickMapper::new(pipeline_id),
            from_tick: TickToTileMapper::new(),
            config,
            stats: BridgeStats::default(),
            buffer: Vec::new(),
            rejected: Vec::new(),
            rooms: Vec::new(),
        }
    }

    /// Register a room to receive assembled tiles for a given domain.
    pub fn register_room(&mut self, room: ForgeRoom) {
        self.rooms.push(room);
    }

    /// Ingest a tile: serialize it into a ForgeTick and push to the buffer.
    pub fn ingest(&mut self, tile: &Tile) -> Result<&ForgeTick, MapperError> {
        let tick = self.to_tick.ingest(tile)?;
        self.push_to_buffer(tick);
        self.stats.ticks_ingested += 1;
        Ok(self.buffer.last().unwrap())
    }

    /// Apply an agent transform to a tick: deserializes tile, agent mutates it,
    /// then a new Transform tick is produced and buffered.
    pub fn transform<F>(
        &mut self,
        tick: &ForgeTick,
        agent: &str,
        f: F,
    ) -> Result<&ForgeTick, MapperError>
    where
        F: FnOnce(Tile) -> Tile,
    {
        let tile = self.from_tick.from_tick(tick)?;
        let transformed = f(tile);
        let new_tick = self.to_tick.transform(&transformed, agent)?;
        self.push_to_buffer(new_tick);
        Ok(self.buffer.last().unwrap())
    }

    /// Assemble the final tick: reconstruct tile and route to matching rooms.
    pub fn assemble(&mut self, tick: &ForgeTick) -> Result<Tile, MapperError> {
        let tile = self.from_tick.from_tick(tick)?;
        let assembled_tick = self.to_tick.assemble(&tile)?;
        self.stats.ticks_assembled += 1;
        self.stats.tiles_routed += self.route_to_rooms(&tile) as u64;
        self.push_to_buffer(assembled_tick);
        Ok(tile)
    }

    /// Reject a tick with a reason; moves it to dead-letter buffer.
    pub fn reject(&mut self, tick: &ForgeTick, reason: impl Into<String>) -> Result<(), MapperError> {
        let tile = self.from_tick.from_tick(tick)?;
        let rejected_tick = self.to_tick.reject(&tile, reason)?;
        self.stats.ticks_rejected += 1;
        if self.config.keep_rejected {
            self.rejected.push(rejected_tick);
        }
        Ok(())
    }

    /// Return all ticks currently in the pipeline buffer (excluding rejected).
    pub fn buffer(&self) -> &[ForgeTick] {
        &self.buffer
    }

    /// Return all rejected ticks in the dead-letter buffer.
    pub fn rejected(&self) -> &[ForgeTick] {
        &self.rejected
    }

    /// Return all registered rooms.
    pub fn rooms(&self) -> &[ForgeRoom] {
        &self.rooms
    }

    fn push_to_buffer(&mut self, tick: ForgeTick) {
        if self.buffer.len() >= self.config.buffer_capacity {
            self.buffer.remove(0);
        }
        self.buffer.push(tick);
    }

    fn route_to_rooms(&mut self, tile: &Tile) -> usize {
        let mut count = 0;
        for room in &mut self.rooms {
            if room.accepts(tile) {
                room.receive(tile.clone());
                count += 1;
            }
        }
        count
    }
}
