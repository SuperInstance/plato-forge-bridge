use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeBridge {
    pub pipeline_id: Uuid,
    pub rooms: HashMap<String, ForgeRoom>,
    pub tile_queue: Vec<BridgeTile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeRoom {
    pub name: String,
    pub agent_count: usize,
    pub tile_count: u64,
    pub last_tick_ms: u64,
    pub cr: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeTile {
    pub id: Uuid,
    pub room: String,
    pub tile_kind: String,
    pub payload: Vec<u8>,
    pub text: Option<String>,
    pub cr: f64,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTick {
    pub id: Uuid,
    pub room: String,
    pub agent: String,
    pub content: String,
    pub tick_type: String,
    pub timestamp_ms: u64,
}

pub struct TileToTickMapper;

impl TileToTickMapper {
    pub fn map(tile: &BridgeTile, room: &str) -> AgentTick {
        AgentTick {
            id: tile.id,
            room: room.to_string(),
            agent: "bridge".to_string(),
            content: tile.text.clone().unwrap_or_else(|| format!("{:?}", tile.payload)),
            tick_type: tile.tile_kind.clone(),
            timestamp_ms: tile.timestamp_ms,
        }
    }

    pub fn map_batch(tiles: &[BridgeTile], room: &str) -> Vec<AgentTick> {
        tiles.iter().map(|t| Self::map(t, room)).collect()
    }

    pub fn with_agent(tile: &BridgeTile, room: &str, agent: &str) -> AgentTick {
        AgentTick {
            id: tile.id,
            room: room.to_string(),
            agent: agent.to_string(),
            content: tile.text.clone().unwrap_or_else(|| format!("{:?}", tile.payload)),
            tick_type: tile.tile_kind.clone(),
            timestamp_ms: tile.timestamp_ms,
        }
    }
}

pub struct TickToTileMapper;

impl TickToTileMapper {
    pub fn to_tile(tick: &AgentTick) -> BridgeTile {
        BridgeTile {
            id: tick.id,
            room: tick.room.clone(),
            tile_kind: tick.tick_type.clone(),
            payload: tick.content.as_bytes().to_vec(),
            text: Some(tick.content.clone()),
            cr: 1.0,
            timestamp_ms: tick.timestamp_ms,
        }
    }

    pub fn to_text(tick: &AgentTick) -> String {
        tick.content.clone()
    }

    pub fn extract_meta(tick: &AgentTick) -> HashMap<String, String> {
        let mut meta = HashMap::new();
        meta.insert("id".to_string(), tick.id.to_string());
        meta.insert("room".to_string(), tick.room.clone());
        meta.insert("agent".to_string(), tick.agent.clone());
        meta.insert("tick_type".to_string(), tick.tick_type.clone());
        meta.insert("timestamp_ms".to_string(), tick.timestamp_ms.to_string());
        meta
    }
}

impl ForgeBridge {
    pub fn new(pipeline_id: Uuid) -> Self {
        ForgeBridge {
            pipeline_id,
            rooms: HashMap::new(),
            tile_queue: Vec::new(),
        }
    }

    pub fn add_room(&mut self, name: &str) {
        self.rooms.insert(
            name.to_string(),
            ForgeRoom {
                name: name.to_string(),
                agent_count: 0,
                tile_count: 0,
                last_tick_ms: 0,
                cr: 1.0,
            },
        );
    }

    pub fn enqueue_tile(&mut self, tile: BridgeTile) {
        self.tile_queue.push(tile);
    }

    pub fn dequeue_ticks(&mut self, room: &str) -> Vec<AgentTick> {
        let (matching, other): (Vec<BridgeTile>, Vec<BridgeTile>) =
            std::mem::take(&mut self.tile_queue)
                .into_iter()
                .partition(|t| t.room == room);
        self.tile_queue = other;

        let ticks = TileToTickMapper::map_batch(&matching, room);

        if let Some(r) = self.rooms.get_mut(room) {
            r.tile_count += ticks.len() as u64;
            if let Some(last) = ticks.last() {
                r.last_tick_ms = last.timestamp_ms;
            }
            let total: f64 = matching.iter().map(|t| t.cr).sum();
            r.cr = if matching.is_empty() { 1.0 } else { total / matching.len() as f64 };
            r.agent_count = ticks.iter().map(|t| t.agent.clone()).collect::<std::collections::HashSet<_>>().len();
        }

        ticks
    }

    pub fn agent_response(&mut self, tick: AgentTick) -> BridgeTile {
        let tile = TickToTileMapper::to_tile(&tick);
        if let Some(r) = self.rooms.get_mut(&tick.room) {
            r.tile_count += 1;
            r.last_tick_ms = tick.timestamp_ms;
        }
        tile
    }

    pub fn room_status(&self, room: &str) -> Option<&ForgeRoom> {
        self.rooms.get(room)
    }

    pub fn pipeline_status(&self) -> HashMap<String, &ForgeRoom> {
        self.rooms.iter().map(|(k, v)| (k.clone(), v)).collect()
    }

    pub fn conservation_ratio(&self) -> f64 {
        if self.rooms.is_empty() {
            return 1.0;
        }
        let total: f64 = self.rooms.values().map(|r| r.cr).sum();
        total / self.rooms.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tile(room: &str, kind: &str, text: &str) -> BridgeTile {
        BridgeTile {
            id: Uuid::new_v4(),
            room: room.to_string(),
            tile_kind: kind.to_string(),
            payload: text.as_bytes().to_vec(),
            text: Some(text.to_string()),
            cr: 0.95,
            timestamp_ms: 1000,
        }
    }

    fn make_tick(room: &str, agent: &str, content: &str) -> AgentTick {
        AgentTick {
            id: Uuid::new_v4(),
            room: room.to_string(),
            agent: agent.to_string(),
            content: content.to_string(),
            tick_type: "response".to_string(),
            timestamp_ms: 2000,
        }
    }

    #[test]
    fn test_bridge_creation() {
        let id = Uuid::new_v4();
        let bridge = ForgeBridge::new(id);
        assert_eq!(bridge.pipeline_id, id);
        assert!(bridge.rooms.is_empty());
        assert!(bridge.tile_queue.is_empty());
    }

    #[test]
    fn test_add_room() {
        let mut bridge = ForgeBridge::new(Uuid::new_v4());
        bridge.add_room("room-a");
        assert!(bridge.rooms.contains_key("room-a"));
        assert_eq!(bridge.rooms["room-a"].name, "room-a");
        assert_eq!(bridge.rooms["room-a"].agent_count, 0);
    }

    #[test]
    fn test_enqueue_and_count() {
        let mut bridge = ForgeBridge::new(Uuid::new_v4());
        bridge.enqueue_tile(make_tile("room-a", "text", "hello"));
        bridge.enqueue_tile(make_tile("room-b", "text", "world"));
        assert_eq!(bridge.tile_queue.len(), 2);
    }

    #[test]
    fn test_dequeue_ticks() {
        let mut bridge = ForgeBridge::new(Uuid::new_v4());
        bridge.add_room("room-a");
        bridge.enqueue_tile(make_tile("room-a", "text", "hello"));
        bridge.enqueue_tile(make_tile("room-a", "text", "world"));
        bridge.enqueue_tile(make_tile("room-b", "text", "other"));

        let ticks = bridge.dequeue_ticks("room-a");
        assert_eq!(ticks.len(), 2);
        assert_eq!(bridge.tile_queue.len(), 1); // room-b tile remains
    }

    #[test]
    fn test_tile_to_tick_mapping() {
        let tile = make_tile("room-a", "text", "hello");
        let tick = TileToTickMapper::map(&tile, "room-a");
        assert_eq!(tick.content, "hello");
        assert_eq!(tick.tick_type, "text");
        assert_eq!(tick.room, "room-a");
    }

    #[test]
    fn test_tile_to_tick_with_agent() {
        let tile = make_tile("room-a", "text", "hello");
        let tick = TileToTickMapper::with_agent(&tile, "room-a", "agent-1");
        assert_eq!(tick.agent, "agent-1");
    }

    #[test]
    fn test_batch_mapping() {
        let tiles = vec![make_tile("r", "t", "a"), make_tile("r", "t", "b")];
        let ticks = TileToTickMapper::map_batch(&tiles, "r");
        assert_eq!(ticks.len(), 2);
    }

    #[test]
    fn test_tick_to_tile() {
        let tick = make_tick("room-a", "agent-1", "response text");
        let tile = TickToTileMapper::to_tile(&tick);
        assert_eq!(tile.room, "room-a");
        assert_eq!(tile.text, Some("response text".to_string()));
        assert_eq!(tile.payload, b"response text");
    }

    #[test]
    fn test_tick_to_text() {
        let tick = make_tick("r", "a", "hello world");
        assert_eq!(TickToTileMapper::to_text(&tick), "hello world");
    }

    #[test]
    fn test_extract_meta() {
        let tick = make_tick("room-a", "agent-1", "content");
        let meta = TickToTileMapper::extract_meta(&tick);
        assert_eq!(meta.get("room").unwrap(), "room-a");
        assert_eq!(meta.get("agent").unwrap(), "agent-1");
        assert_eq!(meta.get("tick_type").unwrap(), "response");
    }

    #[test]
    fn test_agent_response() {
        let mut bridge = ForgeBridge::new(Uuid::new_v4());
        bridge.add_room("room-a");
        let tick = make_tick("room-a", "agent-1", "hi back");
        let tile = bridge.agent_response(tick);
        assert_eq!(tile.room, "room-a");
        assert_eq!(bridge.rooms["room-a"].tile_count, 1);
    }

    #[test]
    fn test_room_status() {
        let mut bridge = ForgeBridge::new(Uuid::new_v4());
        bridge.add_room("room-a");
        let status = bridge.room_status("room-a").unwrap();
        assert_eq!(status.name, "room-a");
        assert!(bridge.room_status("nonexistent").is_none());
    }

    #[test]
    fn test_conservation_ratio() {
        let mut bridge = ForgeBridge::new(Uuid::new_v4());
        assert_eq!(bridge.conservation_ratio(), 1.0);
        bridge.add_room("room-a");
        bridge.add_room("room-b");
        // Default CR is 1.0 per room
        assert_eq!(bridge.conservation_ratio(), 1.0);
    }

    #[test]
    fn test_pipeline_status() {
        let mut bridge = ForgeBridge::new(Uuid::new_v4());
        bridge.add_room("room-a");
        bridge.add_room("room-b");
        let status = bridge.pipeline_status();
        assert_eq!(status.len(), 2);
        assert!(status.contains_key("room-a"));
        assert!(status.contains_key("room-b"));
    }
}
