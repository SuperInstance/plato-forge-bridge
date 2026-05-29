use crate::types::{ForgeTick, Tile};

// ─── ForgeRoom ────────────────────────────────────────────────────────────────
// A Plato room that sits at the output of a forge pipeline.
// Rooms have a domain filter; tiles land here if their domain matches.
// The room is the boundary between the forge pipeline and the Plato agent world.

#[derive(Debug, Clone)]
pub struct ForgeRoom {
    pub name: String,
    /// Accepted domains. Empty = accept all.
    pub domain_filter: Vec<String>,
    tiles: Vec<Tile>,
    /// Log of ticks that passed through this room's pipeline stage.
    tick_log: Vec<ForgeTick>,
}

impl ForgeRoom {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            domain_filter: Vec::new(),
            tiles: Vec::new(),
            tick_log: Vec::new(),
        }
    }

    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain_filter.push(domain.into());
        self
    }

    /// Returns true if this room accepts tiles from the given tile's domain.
    pub fn accepts(&self, tile: &Tile) -> bool {
        self.domain_filter.is_empty()
            || self.domain_filter.iter().any(|d| d == &tile.domain)
    }

    /// Receive an assembled tile into this room.
    pub(crate) fn receive(&mut self, tile: Tile) {
        self.tiles.push(tile);
    }

    /// Record a tick that transited this room.
    pub fn log_tick(&mut self, tick: ForgeTick) {
        self.tick_log.push(tick);
    }

    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    pub fn tick_log(&self) -> &[ForgeTick] {
        &self.tick_log
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    /// Drain tiles from the room (agent consumes them).
    pub fn drain(&mut self) -> Vec<Tile> {
        std::mem::take(&mut self.tiles)
    }

    /// Find tiles by tag.
    pub fn tiles_with_tag(&self, tag: &str) -> Vec<&Tile> {
        self.tiles.iter().filter(|t| t.tags.iter().any(|x| x == tag)).collect()
    }

    /// Find tiles by minimum confidence threshold.
    pub fn tiles_above_confidence(&self, min: f32) -> Vec<&Tile> {
        self.tiles.iter().filter(|t| t.confidence >= min).collect()
    }
}
