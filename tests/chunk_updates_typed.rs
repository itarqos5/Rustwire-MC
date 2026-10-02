use rustwire_mc::{
    codec::Writer,
    connection::{Connection, Event, TypedEvent},
    frame::{FrameCodec, RawPacket},
    packet::typed::{ChunkUpdatePacket, DecodeContext, DecodedPacket},
    version::{Direction, State},
    Limits, Version,
};
use std::io::{Cursor, Read, Write};
struct Memory {
    input: Cursor<Vec<u8>>,
    output: Vec<u8>,
}
impl Read for Memory {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        self.input.read(b)
    }
}
impl Write for Memory {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.output.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn play_connection(packet: RawPacket) -> Connection<Memory> {
    let v = Version::V1_21_5;
    let codec = FrameCodec::default();
    let mut success = Writer::new();
    success.raw(&[0; 16]);
    success.string("Rustwire", 16).unwrap();
    success.var_i32(0);
    let mut bytes = codec
        .encode(&RawPacket::new(2, success.into_inner()))
        .unwrap();
    bytes.extend(
        codec
            .encode(&RawPacket::new(
                v.packet_id(
                    State::Configuration,
                    Direction::Clientbound,
                    "finish_configuration",
                )
                .unwrap(),
                vec![],
            ))
            .unwrap(),
    );
    bytes.extend(codec.encode(&packet).unwrap());
    let io = Memory {
        input: Cursor::new(bytes),
        output: vec![],
    };
    let mut c = Connection::new(io, v, Limits::default());
    c.start_login("localhost", 25565, "Rustwire", [0; 16])
        .unwrap();
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Control(Event::LoginSuccess(_))
    ));
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Control(Event::Ready)
    ));
    c
}

#[test]
fn contextual_packets_preserve_raw_without_inventing_world_height() {
    let v = Version::V1_21_5;
    let light = RawPacket::new(
        v.packet_id(State::Play, Direction::Clientbound, "update_light")
            .unwrap(),
        vec![0; 8],
    );
    let mut connection = play_connection(light.clone());
    assert!(
        matches!(connection.next_typed_event().unwrap(),TypedEvent::Raw {packet,unsupported:None,..} if packet==light)
    );
    let mut connection = play_connection(light);
    assert!(matches!(
        connection
            .next_typed_event_with_context(DecodeContext {
                section_count: Some(24)
            })
            .unwrap(),
        TypedEvent::Decoded(DecodedPacket::ChunkUpdate(ChunkUpdatePacket::Light(_)))
    ));
}
#[test]
fn explicit_context_dispatches_lighting_and_biomes_all_families() {
    for &v in Version::ALL {
        for (name, body) in [("update_light", vec![0; 8]), ("chunk_biomes", vec![0])] {
            assert!(
                DecodedPacket::decode(State::Play, name, &body, v, Limits::default())
                    .unwrap()
                    .is_none()
            );
            assert!(matches!(
                DecodedPacket::decode_with_context(
                    State::Play,
                    name,
                    &body,
                    v,
                    Limits::default(),
                    DecodeContext {
                        section_count: Some(16)
                    }
                )
                .unwrap(),
                Some(DecodedPacket::ChunkUpdate(_))
            ));
            assert!(DecodedPacket::decode_with_context(
                State::Play,
                name,
                &body,
                v,
                Limits::default(),
                DecodeContext {
                    section_count: Some(0)
                }
            )
            .is_err());
            assert!(DecodedPacket::decode_with_context(
                State::Play,
                name,
                &[],
                v,
                Limits::default(),
                DecodeContext {
                    section_count: Some(16)
                }
            )
            .is_err());
        }
    }
}
#[test]
fn malformed_contextual_connection_payload_is_not_silently_hidden() {
    let v = Version::V1_21_5;
    let light = RawPacket::new(
        v.packet_id(State::Play, Direction::Clientbound, "update_light")
            .unwrap(),
        vec![0],
    );
    assert!(play_connection(light)
        .next_typed_event_with_context(DecodeContext {
            section_count: Some(24)
        })
        .is_err());
}
#[test]
fn view_controls_dispatch_without_dimension_context() {
    for &v in Version::ALL {
        for (name, body) in [
            ("update_view_position", vec![0, 0]),
            ("update_view_distance", vec![0]),
            ("simulation_distance", vec![0]),
        ] {
            assert!(matches!(
                DecodedPacket::decode(State::Play, name, &body, v, Limits::default()).unwrap(),
                Some(DecodedPacket::ChunkUpdate(_))
            ));
            assert!(
                DecodedPacket::decode(State::Configuration, name, &body, v, Limits::default())
                    .unwrap()
                    .is_none()
            );
        }
        let mut entity = vec![0; 9]; // packed position and block-entity kind
        entity.push(10); // compound root
        if v.protocol() == 763 {
            entity.extend([0, 0]);
        } // legacy root name
        entity.push(0); // compound end
        assert!(matches!(
            DecodedPacket::decode(
                State::Play,
                "tile_entity_data",
                &entity,
                v,
                Limits::default()
            )
            .unwrap(),
            Some(DecodedPacket::ChunkUpdate(ChunkUpdatePacket::BlockEntity(
                _
            )))
        ));
    }
}

#[test]
fn dimension_context_tracks_explicit_registry_height() {
    for height in [16, 128, 256, 384] {
        let dimension = rustwire_mc::registry::DimensionInfo {
            min_y: -64,
            height,
            logical_height: None,
            has_skylight: None,
            has_ceiling: None,
        };
        assert_eq!(
            DecodeContext::for_dimension(&dimension).section_count,
            Some((height / 16) as usize)
        );
    }
}
#[test]
fn full_chunk_dispatch_reuses_the_dimension_explicit_codec() {
    use rustwire_mc::{
        chunk::{ChunkData, ChunkSection, ContainerKind, Heightmaps, LightData, PaletteContainer},
        nbt::{Nbt, Tag},
    };
    for &v in Version::ALL {
        let chunk = ChunkData {
            x: -2,
            z: 3,
            heightmaps: if v.protocol() < 770 {
                Heightmaps::Nbt(Nbt {
                    name: (v.protocol() < 764).then(|| "".into()),
                    root: Tag::Compound(vec![]),
                })
            } else {
                Heightmaps::Typed(vec![])
            },
            sections: vec![ChunkSection {
                non_air_count: 0,
                fluid_count: (v.protocol() >= 775).then_some(0),
                blocks: PaletteContainer::single(ContainerKind::Blocks, 0),
                biomes: PaletteContainer::single(ContainerKind::Biomes, 1),
            }],
            block_entities: vec![],
            light: LightData {
                sky_mask: vec![],
                block_mask: vec![],
                empty_sky_mask: vec![],
                empty_block_mask: vec![],
                sky_arrays: vec![],
                block_arrays: vec![],
            },
        };
        let body = chunk.encode(v, Limits::default()).unwrap();
        assert!(
            DecodedPacket::decode(State::Play, "map_chunk", &body, v, Limits::default())
                .unwrap()
                .is_none()
        );
        let decoded = DecodedPacket::decode_with_context(
            State::Play,
            "map_chunk",
            &body,
            v,
            Limits::default(),
            DecodeContext {
                section_count: Some(1),
            },
        )
        .unwrap();
        assert_eq!(
            decoded,
            Some(DecodedPacket::ChunkUpdate(ChunkUpdatePacket::FullChunk(
                Box::new(chunk)
            )))
        );
    }
}
