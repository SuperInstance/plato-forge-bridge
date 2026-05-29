use crate::types::{ForgeTick, ForgeTickKind, Tile};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum MapperError {
    #[error("serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("payload missing field: {0}")]
    MissingField(&'static str),
    #[error("tick kind {0:?} cannot produce a complete tile")]
    NonAssemblable(String),
}

// ─── TileToTickMapper ─────────────────────────────────────────────────────────
// Converts a Tile into a ForgeTick for ingestion into the forge pipeline.
// 1:1 mapping — the full tile is serialized into the tick payload.

pub struct TileToTickMapper {
    pipeline_id: Uuid,
    seq_counter: u64,
}

impl TileToTickMapper {
    pub fn new(pipeline_id: Uuid) -> Self {
        Self { pipeline_id, seq_counter: 0 }
    }

    pub fn ingest(&mut self, tile: &Tile) -> Result<ForgeTick, MapperError> {
        self.map_with_kind(tile, ForgeTickKind::Ingest)
    }

    pub fn transform(&mut self, tile: &Tile, agent: &str) -> Result<ForgeTick, MapperError> {
        let mut tick = self.map_with_kind(tile, ForgeTickKind::Transform)?;
        tick.agent_path.push(agent.to_string());
        Ok(tick)
    }

    pub fn verify(&mut self, tile: &Tile, agent: &str) -> Result<ForgeTick, MapperError> {
        let mut tick = self.map_with_kind(tile, ForgeTickKind::Verified)?;
        tick.agent_path.push(agent.to_string());
        Ok(tick)
    }

    pub fn assemble(&mut self, tile: &Tile) -> Result<ForgeTick, MapperError> {
        self.map_with_kind(tile, ForgeTickKind::Assembled)
    }

    pub fn reject(&mut self, tile: &Tile, reason: impl Into<String>) -> Result<ForgeTick, MapperError> {
        self.map_with_kind(tile, ForgeTickKind::Rejected { reason: reason.into() })
    }

    fn map_with_kind(&mut self, tile: &Tile, kind: ForgeTickKind) -> Result<ForgeTick, MapperError> {
        let seq = self.seq_counter;
        self.seq_counter += 1;
        Ok(ForgeTick {
            tick_id: Uuid::new_v4(),
            tile_id: tile.id,
            pipeline_id: self.pipeline_id,
            kind,
            payload: serde_json::to_value(tile)?,
            agent_path: Vec::new(),
            seq,
            emitted_at_ms: now_ms(),
        })
    }
}

// ─── TickToTileMapper ─────────────────────────────────────────────────────────
// Reconstructs a Tile from a ForgeTick payload.
// Only ticks with kind Assembled (or Verified/Transform for intermediate reads)
// can be reconstructed into valid tiles. Rejected ticks return an error.

pub struct TickToTileMapper;

impl TickToTileMapper {
    pub fn new() -> Self {
        Self
    }

    pub fn from_tick(&self, tick: &ForgeTick) -> Result<Tile, MapperError> {
        match &tick.kind {
            ForgeTickKind::Rejected { reason } => {
                Err(MapperError::NonAssemblable(reason.clone()))
            }
            _ => {
                let mut tile: Tile = serde_json::from_value(tick.payload.clone())?;
                // Stamp provenance with the agent path recorded in the tick
                for agent in &tick.agent_path {
                    if !tile.provenance.contains(agent) {
                        tile.provenance.push(agent.clone());
                    }
                }
                tile.metadata.pipeline_id = Some(tick.pipeline_id);
                Ok(tile)
            }
        }
    }

    /// Extract a field from the tick payload without full deserialization.
    pub fn field<'a>(&self, tick: &'a ForgeTick, key: &str) -> Option<&'a serde_json::Value> {
        tick.payload.get(key)
    }
}

impl Default for TickToTileMapper {
    fn default() -> Self { Self::new() }
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
