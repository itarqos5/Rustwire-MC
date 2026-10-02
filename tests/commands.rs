use rustwire_mc::{
    nbt::{Nbt, Tag},
    packet::{chat::ChatComponent, commands::*},
    version::{Direction, State},
    Error, Limits, Version,
};

fn limits() -> Limits {
    Limits::default()
}
fn v(protocol: i32) -> Version {
    Version::from_protocol(protocol).unwrap()
}
fn root(children: Vec<u32>) -> CommandNode {
    CommandNode {
        kind: CommandNodeKind::Root,
        executable: false,
        restricted: false,
        children,
        redirect: None,
    }
}
fn literal(name: &str, children: Vec<u32>, redirect: Option<u32>) -> CommandNode {
    CommandNode {
        kind: CommandNodeKind::Literal { name: name.into() },
        executable: false,
        restricted: false,
        children,
        redirect,
    }
}
fn argument(kind: ParserKind, properties: ParserProperties) -> CommandTree {
    CommandTree {
        nodes: vec![
            root(vec![1]),
            CommandNode {
                kind: CommandNodeKind::Argument {
                    name: "x".into(),
                    parser: CommandParser { kind, properties },
                    suggestions: None,
                },
                executable: false,
                restricted: false,
                children: vec![],
                redirect: None,
            },
        ],
        root_index: 0,
    }
}
// These independently authored fixtures use explicit network-order bytes and
// ordinal lists rather than the library's Writer or generated registry table.
fn expected_names(protocol: i32) -> Vec<String> {
    let mut names: Vec<String> = "bool float double integer long string entity game_profile block_pos column_pos vec3 vec2 block_state block_predicate item_stack item_predicate color component message nbt_compound_tag nbt_tag nbt_path objective objective_criteria operation particle angle rotation scoreboard_slot score_holder swizzle team item_slot resource_location function entity_anchor int_range float_range dimension gamemode time resource_or_tag resource_or_tag_key resource resource_key template_mirror template_rotation heightmap uuid".split_whitespace().map(str::to_owned).collect();
    if protocol >= 765 {
        names.insert(18, "style".into());
    }
    if protocol >= 766 {
        names.insert(34, "item_slots".into());
        names.splice(
            50..50,
            ["loot_table", "loot_predicate", "loot_modifier"].map(str::to_owned),
        );
    }
    if protocol >= 770 {
        names.insert(47, "resource_selector".into());
    }
    if protocol >= 771 {
        names.insert(17, "hex_color".into());
        names.insert(55, "dialog".into());
    }
    if protocol == 776 {
        names[16] = "team_color".into();
    }
    names
        .into_iter()
        .enumerate()
        .map(|(id, name)| format!("{}:{name}", if id < 6 { "brigadier" } else { "minecraft" }))
        .collect()
}
fn fixture_properties(name: &str) -> (ParserProperties, Vec<u8>) {
    match name {
        "brigadier:float" => (
            ParserProperties::Float {
                min: Some(-1.0),
                max: Some(2.0),
            },
            vec![3, 0xbf, 0x80, 0, 0, 0x40, 0, 0, 0],
        ),
        "brigadier:double" => (
            ParserProperties::Double {
                min: Some(-1.0),
                max: Some(2.0),
            },
            vec![3, 0xbf, 0xf0, 0, 0, 0, 0, 0, 0, 0x40, 0, 0, 0, 0, 0, 0, 0],
        ),
        "brigadier:integer" => (
            ParserProperties::Integer {
                min: Some(-2),
                max: Some(3),
            },
            vec![3, 255, 255, 255, 254, 0, 0, 0, 3],
        ),
        "brigadier:long" => (
            ParserProperties::Long {
                min: Some(-2),
                max: Some(3),
            },
            vec![
                3, 255, 255, 255, 255, 255, 255, 255, 254, 0, 0, 0, 0, 0, 0, 0, 3,
            ],
        ),
        "brigadier:string" => (ParserProperties::String(StringMode::GreedyPhrase), vec![2]),
        "minecraft:entity" => (
            ParserProperties::Entity {
                single: true,
                players_only: true,
            },
            vec![3],
        ),
        "minecraft:score_holder" => (ParserProperties::ScoreHolder { multiple: true }, vec![1]),
        "minecraft:time" => (ParserProperties::Time { minimum: -1 }, vec![255; 4]),
        "minecraft:resource"
        | "minecraft:resource_key"
        | "minecraft:resource_or_tag"
        | "minecraft:resource_or_tag_key"
        | "minecraft:resource_selector" => (
            ParserProperties::Registry("minecraft:item".into()),
            b"\x0eminecraft:item".to_vec(),
        ),
        _ => (ParserProperties::None, vec![]),
    }
}
fn parser_fixture(id: usize, properties: &[u8]) -> Vec<u8> {
    let mut bytes = vec![2, 0, 1, 1, 2, 0, 1, b'x', id as u8];
    bytes.extend(properties);
    bytes.push(0);
    bytes
}
fn assert_tree_exact(bytes: &[u8], expected: &CommandTree, version: Version) {
    assert_eq!(
        &CommandTree::decode(bytes, version, limits()).unwrap(),
        expected
    );
    assert_eq!(expected.encode(version, limits()).unwrap(), bytes);
    for cut in 0..bytes.len() {
        assert!(
            CommandTree::decode(&bytes[..cut], version, limits()).is_err(),
            "{} truncation {cut}",
            version.protocol()
        );
    }
    let mut extra = bytes.to_vec();
    extra.push(0);
    assert!(CommandTree::decode(&extra, version, limits()).is_err());
}
#[test]
fn all_release_registries_and_every_parser_have_independent_golden_fixtures() {
    for &version in Version::ALL {
        let names = expected_names(version.protocol());
        let registry = parser_registry(version);
        assert_eq!(registry.len(), names.len());
        for (id, name) in names.iter().enumerate() {
            let kind = registry[id];
            assert_eq!(kind.name(), name);
            assert_eq!(kind.id(version).unwrap(), id as i32);
            assert_eq!(ParserKind::from_id(id as i32, version).unwrap(), kind);
            let (properties, bytes) = fixture_properties(name);
            assert_tree_exact(
                &parser_fixture(id, &bytes),
                &argument(kind, properties),
                version,
            );
        }
        assert!(matches!(
            ParserKind::from_id(-1, version),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(
            ParserKind::from_id(registry.len() as i32, version),
            Err(Error::Unsupported(_))
        ));
    }
    assert!(ParserKind::Color.id(v(776)).is_err());
    assert!(ParserKind::TeamColor.id(v(775)).is_err());
    assert!(ParserKind::ResourceSelector.id(v(769)).is_err());
    assert!(ParserKind::Dialog.id(v(770)).is_err());
}
#[test]
fn root_golden_every_release() {
    for &version in Version::ALL {
        assert_tree_exact(
            &[1, 0, 0, 0],
            &CommandTree {
                nodes: vec![root(vec![])],
                root_index: 0,
            },
            version,
        );
    }
}
#[test]
fn node_flags_and_mixed_redirect_cycle_golden_every_release() {
    // root -> run -> x, and x redirects to root: valid mixed-edge cycle.
    for &version in Version::ALL {
        let mut tree = argument(ParserKind::Bool, ParserProperties::None);
        tree.nodes.insert(1, literal("run", vec![2], None));
        tree.nodes[2].redirect = Some(0);
        tree.nodes[2].executable = true;
        tree.nodes[2].restricted = version.protocol() >= 771;
        if let CommandNodeKind::Argument { suggestions, .. } = &mut tree.nodes[2].kind {
            *suggestions = Some(SuggestionProvider::AskServer);
        }
        let mut bytes = b"\x03\x00\x01\x01\x01\x01\x02\x03run".to_vec();
        bytes.extend([
            if version.protocol() >= 771 {
                0x3e
            } else {
                0x1e
            },
            0,
            0,
            1,
            b'x',
            0,
        ]);
        bytes.extend(b"\x14minecraft:ask_server\x00");
        assert_tree_exact(&bytes, &tree, version);
    }
}
#[test]
fn branch_sharing_nonzero_root_and_custom_providers_preserved() {
    for &version in Version::ALL {
        let tree = CommandTree {
            nodes: vec![
                literal("a", vec![3], None),
                root(vec![0, 2]),
                literal("b", vec![3], None),
                literal("c", vec![], Some(1)),
            ],
            root_index: 1,
        };
        let bytes = tree.encode(version, limits()).unwrap();
        assert_tree_exact(&bytes, &tree, version);
        for provider in [
            SuggestionProvider::AskServer,
            SuggestionProvider::AvailableSounds,
            SuggestionProvider::SummonableEntities,
            SuggestionProvider::Custom("custom:hint/path".into()),
            SuggestionProvider::Custom("ask_server".into()),
        ] {
            let mut tree = argument(ParserKind::Bool, ParserProperties::None);
            if let CommandNodeKind::Argument { suggestions, .. } = &mut tree.nodes[1].kind {
                *suggestions = Some(provider);
            }
            let bytes = tree.encode(version, limits()).unwrap();
            assert_tree_exact(&bytes, &tree, version);
        }
    }
}
#[test]
fn malformed_flags_discriminants_and_unknown_ids_fail_closed() {
    for &version in Version::ALL {
        for flags in [3, 16, 17, 0x40, 0x80, 0xff] {
            assert!(CommandTree::decode(&[1, flags, 0, 0], version, limits()).is_err());
        }
        for id in [1, 2, 3, 4, 6] {
            assert!(CommandTree::decode(&parser_fixture(id, &[4]), version, limits()).is_err());
        }
        let score_id = expected_names(version.protocol())
            .iter()
            .position(|s| s == "minecraft:score_holder")
            .unwrap();
        assert!(CommandTree::decode(&parser_fixture(score_id, &[2]), version, limits()).is_err());
        for mode in [3, 127, 255] {
            assert!(CommandTree::decode(&parser_fixture(5, &[mode]), version, limits()).is_err());
        }
        // No parser payload or root is needed to establish Unsupported.
        let unknown = [2, 0, 1, 1, 2, 0, 1, b'x', 127];
        assert!(matches!(
            CommandTree::decode(&unknown, version, limits()),
            Err(Error::Unsupported(_))
        ));
        let unknown_negative = [2, 0, 1, 1, 2, 0, 1, b'x', 255, 255, 255, 255, 15];
        assert!(matches!(
            CommandTree::decode(&unknown_negative, version, limits()),
            Err(Error::Unsupported(_))
        ));
        let mut bad_shape = argument(ParserKind::Bool, ParserProperties::Time { minimum: 0 });
        assert!(bad_shape.encode(version, limits()).is_err());
        bad_shape = argument(ParserKind::Float, ParserProperties::None);
        assert!(bad_shape.encode(version, limits()).is_err());
        if version.protocol() < 771 {
            assert!(CommandTree::decode(&[1, 32, 0, 0], version, limits()).is_err());
            let mut restricted = CommandTree {
                nodes: vec![root(vec![])],
                root_index: 0,
            };
            restricted.nodes[0].restricted = true;
            assert!(restricted.encode(version, limits()).is_err());
        }
    }
}
#[test]
fn invalid_references_and_separate_dependency_cycles_rejected() {
    for &version in Version::ALL {
        for bytes in [
            &[0, 0][..],
            &[1, 0, 0, 1],
            &[1, 0, 1, 1, 0],
            &[1, 8, 0, 1, 0],
            &[1, 0, 0, 255, 255, 255, 255, 15],
            &[1, 1, 0, 1, b'a', 0],
            &[1, 0, 1, 0, 0],
            &[1, 8, 0, 0, 0],
        ] {
            assert!(
                CommandTree::decode(bytes, version, limits()).is_err(),
                "{bytes:?}"
            );
        }
        let valid = CommandTree {
            nodes: vec![
                root(vec![1]),
                literal("a", vec![], None),
                literal("b", vec![], None),
            ],
            root_index: 0,
        };
        let mut trees = vec![];
        let mut tree = valid.clone();
        tree.root_index = 1;
        trees.push(tree);
        let mut tree = valid.clone();
        tree.nodes[0].children = vec![99];
        trees.push(tree);
        let mut tree = valid.clone();
        tree.nodes[1].redirect = Some(u32::MAX);
        trees.push(tree);
        let mut tree = valid.clone();
        tree.nodes[1].children = vec![2];
        tree.nodes[2].children = vec![1];
        trees.push(tree);
        let mut tree = valid.clone();
        tree.nodes[1].redirect = Some(2);
        tree.nodes[2].redirect = Some(1);
        trees.push(tree);
        for tree in trees {
            assert!(tree.encode(version, limits()).is_err());
        }
        // Duplicated edges and unreachable nodes are not discarded or guessed.
        let mut tree = valid;
        tree.nodes[0].children = vec![1, 1];
        assert_tree_exact(&tree.encode(version, limits()).unwrap(), &tree, version);
    }
}
#[test]
fn parser_numeric_presence_modes_and_unusual_official_values_preserved() {
    for &version in Version::ALL {
        for mask in 0..4 {
            let min = mask & 1 != 0;
            let max = mask & 2 != 0;
            for (kind, props) in [
                (
                    ParserKind::Float,
                    ParserProperties::Float {
                        min: min.then_some(f32::INFINITY),
                        max: max.then_some(f32::NEG_INFINITY),
                    },
                ),
                (
                    ParserKind::Double,
                    ParserProperties::Double {
                        min: min.then_some(f64::INFINITY),
                        max: max.then_some(f64::NEG_INFINITY),
                    },
                ),
                (
                    ParserKind::Integer,
                    ParserProperties::Integer {
                        min: min.then_some(i32::MAX),
                        max: max.then_some(i32::MIN),
                    },
                ),
                (
                    ParserKind::Long,
                    ParserProperties::Long {
                        min: min.then_some(i64::MAX),
                        max: max.then_some(i64::MIN),
                    },
                ),
            ] {
                let tree = argument(kind, props);
                assert_tree_exact(&tree.encode(version, limits()).unwrap(), &tree, version);
            }
        }
        for mode in [
            StringMode::SingleWord,
            StringMode::QuotablePhrase,
            StringMode::GreedyPhrase,
        ] {
            let tree = argument(ParserKind::String, ParserProperties::String(mode));
            assert_tree_exact(&tree.encode(version, limits()).unwrap(), &tree, version);
        }
        for byte in 0..4 {
            let bytes = parser_fixture(6, &[byte]);
            let tree = CommandTree::decode(&bytes, version, limits()).unwrap();
            assert_eq!(tree.encode(version, limits()).unwrap(), bytes);
        }
        for (kind, bits) in [
            (ParserKind::Float, vec![1, 0x7f, 0xc0, 0, 1]),
            (ParserKind::Double, vec![1, 0x7f, 0xf8, 0, 0, 0, 0, 0, 1]),
        ] {
            let bytes = parser_fixture(kind.id(version).unwrap() as usize, &bits);
            let tree = CommandTree::decode(&bytes, version, limits()).unwrap();
            assert_eq!(tree.encode(version, limits()).unwrap(), bytes);
        }
    }
}
#[test]
fn strings_identifiers_and_utf16_budgets() {
    for &version in Version::ALL {
        for bad in [
            "Minecraft:item",
            "minecraft:Item",
            "minecraft:foo bar",
            "a:b:c",
            "é:x",
        ] {
            let tree = argument(ParserKind::Resource, ParserProperties::Registry(bad.into()));
            assert!(tree.encode(version, limits()).is_err());
            let mut bytes = vec![bad.len() as u8];
            bytes.extend(bad.as_bytes());
            assert!(CommandTree::decode(
                &parser_fixture(ParserKind::Resource.id(version).unwrap() as usize, &bytes),
                version,
                limits()
            )
            .is_err());
        }
        for good in ["item", ":item", "minecraft:", "foo.bar:baz/path-0"] {
            let tree = argument(
                ParserKind::Resource,
                ParserProperties::Registry(good.into()),
            );
            assert_tree_exact(&tree.encode(version, limits()).unwrap(), &tree, version);
        }
        let tree = argument(
            ParserKind::Resource,
            ParserProperties::Registry("..:item".into()),
        );
        assert_eq!(
            tree.encode(version, limits()).is_err(),
            version.protocol() >= 775
        );
        let mut tree = argument(ParserKind::Bool, ParserProperties::None);
        if let CommandNodeKind::Argument { name, .. } = &mut tree.nodes[1].kind {
            *name = "😀".into();
        }
        let bytes = tree.encode(version, limits()).unwrap();
        let tight = Limits {
            max_string_chars: 1,
            ..limits()
        };
        assert!(tree.encode(version, tight).is_err());
        assert!(CommandTree::decode(&bytes, version, tight).is_err());
        let exact = Limits {
            max_string_chars: 2,
            ..limits()
        };
        assert_eq!(tree.encode(version, exact).unwrap(), bytes);
        assert!(CommandTree::decode(&[2, 0, 1, 1, 1, 0, 1, 255, 0], version, limits()).is_err());
    }
}
#[test]
fn packet_and_aggregate_collection_budgets() {
    for &version in Version::ALL {
        let tree = CommandTree {
            nodes: vec![root(vec![1]), literal("a", vec![], Some(0))],
            root_index: 0,
        };
        let bytes = tree.encode(version, limits()).unwrap();
        let exact = Limits {
            max_collection: 4,
            max_packet: bytes.len(),
            ..limits()
        };
        assert_tree_exact(&bytes, &tree, version);
        assert_eq!(tree.encode(version, exact).unwrap(), bytes);
        assert!(CommandTree::decode(&bytes, version, exact).is_ok());
        for bad in [
            Limits {
                max_collection: 3,
                ..exact
            },
            Limits {
                max_packet: bytes.len() - 1,
                ..exact
            },
            Limits {
                max_packet: 0,
                ..exact
            },
        ] {
            assert!(tree.encode(version, bad).is_err());
            assert!(CommandTree::decode(&bytes, version, bad).is_err());
        }
        for huge in [
            &[255, 255, 255, 255, 7][..],
            &[1, 0, 255, 255, 255, 255, 7],
            &[255, 255, 255, 255, 15],
        ] {
            assert!(CommandTree::decode(huge, version, limits()).is_err());
        }
    }
}
#[test]
fn deep_graph_is_iterative_with_linear_validation() {
    let count = 20_000;
    let mut nodes = Vec::with_capacity(count);
    nodes.push(root(vec![1]));
    for at in 1..count {
        nodes.push(literal(
            "x",
            if at + 1 < count {
                vec![(at + 1) as u32]
            } else {
                vec![]
            },
            if at + 1 == count { Some(0) } else { None },
        ));
    }
    let tree = CommandTree {
        nodes,
        root_index: 0,
    };
    let bytes = tree.encode(v(776), limits()).unwrap();
    assert_eq!(CommandTree::decode(&bytes, v(776), limits()).unwrap(), tree);
    let mut cycle = tree;
    cycle.nodes[count - 1].children = vec![1];
    assert!(cycle.encode(v(776), limits()).is_err());
}
fn tooltip(version: Version) -> ChatComponent {
    if version.protocol() < 765 {
        ChatComponent::Json("\"tip\"".into())
    } else {
        ChatComponent::Nbt(Nbt {
            name: None,
            root: Tag::String("tip".into()),
        })
    }
}
fn response(version: Version) -> (CommandSuggestions, Vec<u8>) {
    let value = CommandSuggestions {
        transaction_id: 7,
        start: 2,
        length: 3,
        matches: vec![
            CommandSuggestion {
                text: "foo".into(),
                tooltip: Some(tooltip(version)),
            },
            CommandSuggestion {
                text: "bar".into(),
                tooltip: None,
            },
        ],
    };
    let mut bytes = b"\x07\x02\x03\x02\x03foo\x01".to_vec();
    if version.protocol() < 765 {
        bytes.extend(b"\x05\"tip\"");
    } else {
        bytes.extend(b"\x08\x00\x03tip");
    }
    bytes.extend(b"\x03bar\x00");
    (value, bytes)
}
#[test]
fn suggestions_independent_goldens_all_releases_truncations_trailing_and_request_ids() {
    for &version in Version::ALL {
        let (expected, bytes) = response(version);
        assert_eq!(
            CommandSuggestions::decode(&bytes, version, limits()).unwrap(),
            expected
        );
        assert_eq!(expected.encode(version, limits()).unwrap(), bytes);
        for cut in 0..bytes.len() {
            assert!(CommandSuggestions::decode(&bytes[..cut], version, limits()).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(CommandSuggestions::decode(&trailing, version, limits()).is_err());
        let req = CommandSuggestionRequest {
            transaction_id: -1,
            text: "/he".into(),
        };
        let golden = [255, 255, 255, 255, 15, 3, b'/', b'h', b'e'];
        assert_eq!(req.encode(version, limits()).unwrap(), golden);
        assert_eq!(
            CommandSuggestionRequest::decode(&golden, version, limits()).unwrap(),
            req
        );
        let packet = req.packet(version, limits()).unwrap();
        assert_eq!(
            packet.id,
            version
                .packet_id(State::Play, Direction::Serverbound, "tab_complete")
                .unwrap()
        );
        assert_eq!(packet.data, golden);
        for cut in 0..golden.len() {
            assert!(CommandSuggestionRequest::decode(&golden[..cut], version, limits()).is_err());
        }
        let mut trailing = golden.to_vec();
        trailing.push(0);
        assert!(CommandSuggestionRequest::decode(&trailing, version, limits()).is_err());
        let negative = CommandSuggestions {
            transaction_id: i32::MIN,
            start: -1,
            length: i32::MAX,
            matches: vec![],
        };
        let bytes = negative.encode(version, limits()).unwrap();
        assert_eq!(
            CommandSuggestions::decode(&bytes, version, limits()).unwrap(),
            negative
        );
    }
}
#[test]
fn suggestion_string_nbt_and_total_budgets() {
    for &version in Version::ALL {
        let (expected, bytes) = response(version);
        let exact = Limits {
            max_collection: 2,
            max_packet: bytes.len(),
            ..limits()
        };
        assert!(expected.encode(version, exact).is_ok());
        assert!(CommandSuggestions::decode(&bytes, version, exact).is_ok());
        for bad in [
            Limits {
                max_collection: 1,
                ..exact
            },
            Limits {
                max_packet: bytes.len() - 1,
                ..exact
            },
            Limits {
                max_string_chars: 2,
                ..exact
            },
        ] {
            assert!(expected.encode(version, bad).is_err());
            assert!(CommandSuggestions::decode(&bytes, version, bad).is_err());
        }
        assert!(CommandSuggestions::decode(&[0, 0, 0, 1, 0, 2], version, limits()).is_err());
        assert!(
            CommandSuggestions::decode(&[0, 0, 0, 255, 255, 255, 255, 7], version, limits())
                .is_err()
        );
        let mut mismatch = expected.clone();
        mismatch.matches[0].tooltip = Some(if version.protocol() < 765 {
            tooltip(v(765))
        } else {
            tooltip(v(763))
        });
        assert!(mismatch.encode(version, limits()).is_err());
        if version.protocol() >= 765 {
            assert!(CommandSuggestions::decode(&[0, 0, 0, 1, 0, 1, 0], version, limits()).is_err());
            let mut many = expected;
            many.matches[1].tooltip = Some(tooltip(version));
            let bytes = many.encode(version, limits()).unwrap();
            let one = Limits {
                max_nbt_nodes: 1,
                ..limits()
            };
            assert!(many.encode(version, one).is_err());
            assert!(CommandSuggestions::decode(&bytes, version, one).is_err());
            let two = Limits {
                max_nbt_nodes: 2,
                ..limits()
            };
            assert!(many.encode(version, two).is_ok());
            assert!(CommandSuggestions::decode(&bytes, version, two).is_ok());
        }
        let request = CommandSuggestionRequest {
            transaction_id: 0,
            text: "x".repeat(32500),
        };
        let bytes = request.encode(version, limits()).unwrap();
        assert!(CommandSuggestionRequest::decode(&bytes, version, limits()).is_ok());
        let too_long = CommandSuggestionRequest {
            transaction_id: 0,
            text: "x".repeat(32501),
        };
        assert!(too_long.encode(version, limits()).is_err());
        let mut bytes = vec![0, 0xf5, 0xfd, 1];
        bytes.extend(vec![b'x'; 32501]);
        assert!(CommandSuggestionRequest::decode(&bytes, version, limits()).is_err());
    }
}
#[test]
fn legacy_tooltip_uses_component_limit_not_schema_plain_string_limit() {
    let value = CommandSuggestions {
        transaction_id: 0,
        start: 0,
        length: 0,
        matches: vec![CommandSuggestion {
            text: String::new(),
            tooltip: Some(ChatComponent::Json("x".repeat(32768))),
        }],
    };
    let large = Limits {
        max_string_chars: 262144,
        ..limits()
    };
    for version in [v(763), v(764)] {
        let bytes = value.encode(version, large).unwrap();
        assert_eq!(
            CommandSuggestions::decode(&bytes, version, large).unwrap(),
            value
        );
    }
}
#[test]
fn deterministic_mutations_are_bounded_and_successful_decodes_reencode() {
    let small = Limits {
        max_packet: 4096,
        max_collection: 64,
        max_string_chars: 64,
        max_nbt_nodes: 64,
        max_nbt_depth: 8,
        ..limits()
    };
    for &version in Version::ALL {
        let mut seeds = vec![
            vec![1, 0, 0, 0],
            parser_fixture(1, &[3, 0xbf, 0x80, 0, 0, 0x40, 0, 0, 0]),
        ];
        seeds.push(
            CommandTree {
                nodes: vec![root(vec![1]), literal("a", vec![], Some(0))],
                root_index: 0,
            }
            .encode(version, limits())
            .unwrap(),
        );
        for seed in seeds {
            for offset in 0..seed.len() {
                for byte in [0, 1, 2, 3, 0x1f, 0x20, 0x7f, 0x80, 0xff] {
                    let mut mutated = seed.clone();
                    mutated[offset] = byte;
                    if let Ok(tree) = CommandTree::decode(&mutated, version, small) {
                        let encoded = tree.encode(version, small).unwrap();
                        let again = CommandTree::decode(&encoded, version, small).unwrap();
                        assert_eq!(again.encode(version, small).unwrap(), encoded);
                    }
                }
            }
        }
        let (_, seed) = response(version);
        for offset in 0..seed.len() {
            for byte in [0, 1, 2, 0x7f, 0x80, 0xff] {
                let mut mutated = seed.clone();
                mutated[offset] = byte;
                if let Ok(value) = CommandSuggestions::decode(&mutated, version, small) {
                    let encoded = value.encode(version, small).unwrap();
                    assert!(CommandSuggestions::decode(&encoded, version, small).is_ok());
                }
            }
        }
    }
}

#[test]
fn request_exact_packet_utf16_and_string_budgets() {
    for &version in Version::ALL {
        let request = CommandSuggestionRequest {
            transaction_id: 0,
            text: "😀".into(),
        };
        let golden = [0, 4, 0xf0, 0x9f, 0x98, 0x80];
        let exact = Limits {
            max_packet: golden.len(),
            max_string_chars: 2,
            ..limits()
        };
        assert_eq!(request.encode(version, exact).unwrap(), golden);
        assert_eq!(
            CommandSuggestionRequest::decode(&golden, version, exact).unwrap(),
            request
        );
        for bad in [
            Limits {
                max_packet: 5,
                ..exact
            },
            Limits {
                max_packet: 0,
                ..exact
            },
            Limits {
                max_string_chars: 1,
                ..exact
            },
        ] {
            assert!(request.encode(version, bad).is_err());
            assert!(CommandSuggestionRequest::decode(&golden, version, bad).is_err());
        }
        for invalid in [
            &[0, 1, 255][..],
            &[0, 255, 255, 255, 255, 15],
            &[128, 128, 128, 128, 128, 0],
        ] {
            assert!(CommandSuggestionRequest::decode(invalid, version, limits()).is_err());
        }
    }
}
