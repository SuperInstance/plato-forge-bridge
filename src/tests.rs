#[cfg(test)]
mod tests {
    use crate::{
        bridge::{BridgeConfig, ForgeBridge},
        mapper::{TickToTileMapper, TileToTickMapper},
        room::ForgeRoom,
        types::{ForgeTickKind, Tile},
    };
    use uuid::Uuid;

    fn make_tile(domain: &str, q: &str, a: &str) -> Tile {
        Tile::new(domain, q, a)
            .with_confidence(0.9)
            .with_tags(["rust", "forge"])
            .with_source("test-suite")
    }

    fn make_mapper() -> TileToTickMapper {
        TileToTickMapper::new(Uuid::new_v4())
    }

    // ── Mapper tests ──────────────────────────────────────────────────────────

    #[test]
    fn tile_to_tick_ingest_kind() {
        let tile = make_tile("math", "What is 2+2?", "4");
        let mut m = make_mapper();
        let tick = m.ingest(&tile).unwrap();
        assert!(matches!(tick.kind, ForgeTickKind::Ingest));
    }

    #[test]
    fn tile_to_tick_preserves_tile_id() {
        let tile = make_tile("math", "What is 2+2?", "4");
        let mut m = make_mapper();
        let tick = m.ingest(&tile).unwrap();
        assert_eq!(tick.tile_id, tile.id);
    }

    #[test]
    fn tile_to_tick_payload_roundtrip() {
        let tile = make_tile("history", "Who wrote Hamlet?", "Shakespeare");
        let mut m = make_mapper();
        let tick = m.ingest(&tile).unwrap();
        let from = TickToTileMapper::new();
        let recovered = from.from_tick(&tick).unwrap();
        assert_eq!(recovered.question, tile.question);
        assert_eq!(recovered.answer, tile.answer);
        assert_eq!(recovered.confidence, tile.confidence);
        assert_eq!(recovered.tags, tile.tags);
    }

    #[test]
    fn transform_tick_records_agent() {
        let tile = make_tile("science", "Speed of light?", "3e8 m/s");
        let mut m = make_mapper();
        let tick = m.transform(&tile, "verifier-agent").unwrap();
        assert!(tick.agent_path.contains(&"verifier-agent".to_string()));
        assert!(matches!(tick.kind, ForgeTickKind::Transform));
    }

    #[test]
    fn assembled_tick_is_terminal() {
        let tile = make_tile("science", "Speed of light?", "3e8 m/s");
        let mut m = make_mapper();
        let tick = m.assemble(&tile).unwrap();
        assert!(tick.is_terminal());
    }

    #[test]
    fn rejected_tick_is_terminal_and_errors_on_reconstruct() {
        let tile = make_tile("unknown", "???", "???");
        let mut m = make_mapper();
        let tick = m.reject(&tile, "domain not recognized").unwrap();
        assert!(tick.is_terminal());
        let from = TickToTileMapper::new();
        assert!(from.from_tick(&tick).is_err());
    }

    #[test]
    fn seq_is_monotonically_increasing() {
        let mut m = make_mapper();
        let tile = make_tile("x", "q", "a");
        let t0 = m.ingest(&tile).unwrap();
        let t1 = m.ingest(&tile).unwrap();
        let t2 = m.ingest(&tile).unwrap();
        assert!(t0.seq < t1.seq);
        assert!(t1.seq < t2.seq);
    }

    #[test]
    fn field_accessor_returns_correct_value() {
        let tile = make_tile("phys", "What is gravity?", "9.8 m/s²");
        let mut m = make_mapper();
        let tick = m.ingest(&tile).unwrap();
        let from = TickToTileMapper::new();
        let domain = from.field(&tick, "domain").unwrap();
        assert_eq!(domain.as_str().unwrap(), "phys");
    }

    // ── Bridge tests ──────────────────────────────────────────────────────────

    #[test]
    fn bridge_ingest_increments_stats() {
        let mut bridge = ForgeBridge::new(BridgeConfig::default());
        let tile = make_tile("cs", "What is a monad?", "a monoid in the category of endofunctors");
        bridge.ingest(&tile).unwrap();
        bridge.ingest(&tile).unwrap();
        assert_eq!(bridge.stats.ticks_ingested, 2);
    }

    #[test]
    fn bridge_transform_closure_mutates_tile() {
        let mut bridge = ForgeBridge::new(BridgeConfig::default());
        let tile = make_tile("cs", "What is a monad?", "?");
        let tick = bridge.ingest(&tile).unwrap().clone();

        let result = bridge
            .transform(&tick, "answer-agent", |mut t| {
                t.answer = "a monoid in the category of endofunctors".into();
                t.confidence = 0.95;
                t
            })
            .unwrap()
            .clone();

        let from = TickToTileMapper::new();
        let out = from.from_tick(&result).unwrap();
        assert_eq!(out.answer, "a monoid in the category of endofunctors");
        assert!((out.confidence - 0.95).abs() < 1e-5);
    }

    #[test]
    fn bridge_assemble_routes_to_room() {
        let mut bridge = ForgeBridge::new(BridgeConfig::default());
        bridge.register_room(ForgeRoom::new("math-room").with_domain("math"));

        let tile = make_tile("math", "What is pi?", "3.14159...");
        let tick = bridge.ingest(&tile).unwrap().clone();
        let assembled = bridge.assemble(&tick).unwrap();

        assert_eq!(assembled.domain, "math");
        assert_eq!(bridge.rooms()[0].tile_count(), 1);
        assert_eq!(bridge.stats.tiles_routed, 1);
    }

    #[test]
    fn bridge_assemble_does_not_route_to_wrong_domain_room() {
        let mut bridge = ForgeBridge::new(BridgeConfig::default());
        bridge.register_room(ForgeRoom::new("bio-room").with_domain("biology"));

        let tile = make_tile("math", "What is pi?", "3.14159...");
        let tick = bridge.ingest(&tile).unwrap().clone();
        bridge.assemble(&tick).unwrap();

        assert_eq!(bridge.rooms()[0].tile_count(), 0);
        assert_eq!(bridge.stats.tiles_routed, 0);
    }

    #[test]
    fn bridge_reject_goes_to_dead_letter() {
        let mut bridge = ForgeBridge::new(BridgeConfig::default());
        let tile = make_tile("spam", "Buy now!", "click here");
        let tick = bridge.ingest(&tile).unwrap().clone();
        bridge.reject(&tick, "spam domain").unwrap();
        assert_eq!(bridge.stats.ticks_rejected, 1);
        assert_eq!(bridge.rejected().len(), 1);
    }

    // ── ForgeRoom tests ───────────────────────────────────────────────────────

    #[test]
    fn room_domain_filter_accepts_matching() {
        let room = ForgeRoom::new("physics").with_domain("physics");
        let tile = make_tile("physics", "q", "a");
        assert!(room.accepts(&tile));
    }

    #[test]
    fn room_domain_filter_rejects_non_matching() {
        let room = ForgeRoom::new("physics").with_domain("physics");
        let tile = make_tile("art", "q", "a");
        assert!(!room.accepts(&tile));
    }

    #[test]
    fn room_no_filter_accepts_all() {
        let room = ForgeRoom::new("any");
        let tile1 = make_tile("math", "q", "a");
        let tile2 = make_tile("art", "q", "a");
        assert!(room.accepts(&tile1));
        assert!(room.accepts(&tile2));
    }

    #[test]
    fn room_tiles_with_tag_filters_correctly() {
        let mut room = ForgeRoom::new("all");
        let t1 = make_tile("x", "q1", "a1"); // has tag "forge"
        let t2 = Tile::new("x", "q2", "a2").with_tags(["untagged"]);
        room.receive(t1);
        room.receive(t2);
        let tagged = room.tiles_with_tag("forge");
        assert_eq!(tagged.len(), 1);
    }

    #[test]
    fn room_drain_empties_room() {
        let mut room = ForgeRoom::new("all");
        room.receive(make_tile("x", "q", "a"));
        room.receive(make_tile("y", "q", "a"));
        let drained = room.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(room.tile_count(), 0);
    }

    #[test]
    fn provenance_accumulated_across_transforms() {
        let mut bridge = ForgeBridge::new(BridgeConfig::default());
        let tile = make_tile("cs", "What is Rust?", "a systems language");
        let t0 = bridge.ingest(&tile).unwrap().clone();

        let t1 = bridge.transform(&t0, "agent-a", |t| t).unwrap().clone();
        let t2 = bridge.transform(&t1, "agent-b", |t| t).unwrap().clone();
        let out = bridge.assemble(&t2).unwrap();

        assert!(out.provenance.contains(&"agent-a".to_string()));
        assert!(out.provenance.contains(&"agent-b".to_string()));
    }

    #[test]
    fn was_touched_by_tracks_agent_path() {
        let tile = make_tile("cs", "q", "a");
        let mut m = make_mapper();
        let tick = m.transform(&tile, "editor-bot").unwrap();
        assert!(tick.was_touched_by("editor-bot"));
        assert!(!tick.was_touched_by("other-bot"));
    }
}
