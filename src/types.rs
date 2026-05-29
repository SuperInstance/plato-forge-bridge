use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Tile ────────────────────────────────────────────────────────────────────
// A Plato knowledge tile. Immutable once created; transforms produce new tiles.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub id: Uuid,
    pub domain: String,
    pub question: String,
    pub answer: String,
    pub source: String,
    pub confidence: f32,
    pub tags: Vec<String>,
    pub provenance: Vec<String>,
    pub metadata: TileMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TileMetadata {
    pub created_at_ms: u64,
    pub version: u32,
    pub room: Option<String>,
    pub pipeline_id: Option<Uuid>,
}

impl Tile {
    pub fn new(domain: impl Into<String>, question: impl Into<String>, answer: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            domain: domain.into(),
            question: question.into(),
            answer: answer.into(),
            source: String::new(),
            confidence: 1.0,
            tags: Vec::new(),
            provenance: Vec::new(),
            metadata: TileMetadata::default(),
        }
    }

    pub fn with_confidence(mut self, c: f32) -> Self {
        self.confidence = c.clamp(0.0, 1.0);
        self
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }
}

// ─── ForgeTick ───────────────────────────────────────────────────────────────
// A Plato inter-agent message carrying a tile through the forge pipeline.
// The tick is the unit of work: agents consume ticks, transform them, emit ticks.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForgeTick {
    pub tick_id: Uuid,
    pub tile_id: Uuid,           // identity of the originating tile
    pub pipeline_id: Uuid,       // which forge pipeline this belongs to
    pub kind: ForgeTickKind,
    pub payload: serde_json::Value,  // serialized tile (or partial result)
    pub agent_path: Vec<String>, // agents that have touched this tick, in order
    pub seq: u64,                // monotonic sequence within the pipeline
    pub emitted_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForgeTickKind {
    /// Fresh tile entering the pipeline
    Ingest,
    /// Tile mid-transform by an agent
    Transform,
    /// Tile has been verified/validated
    Verified,
    /// Tile rejected by policy or agent
    Rejected { reason: String },
    /// Final assembled output tile
    Assembled,
}

impl ForgeTick {
    pub fn is_terminal(&self) -> bool {
        matches!(self.kind, ForgeTickKind::Assembled | ForgeTickKind::Rejected { .. })
    }

    pub fn was_touched_by(&self, agent: &str) -> bool {
        self.agent_path.iter().any(|a| a == agent)
    }
}
