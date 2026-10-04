use rustwire_mc::Version;
pub struct Fixture {
    pub version: Version,
    pub packet_id: i32,
    pub case: &'static str,
    pub bytes: Vec<u8>,
}
pub fn fixtures() -> Vec<Fixture> {
    include_str!("../fixtures/recipe-properties.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 4);
            assert_eq!(fields[3].len() % 2, 0);
            Fixture {
                version: Version::from_protocol(fields[0].parse().unwrap()).unwrap(),
                packet_id: fields[1].parse().unwrap(),
                case: fields[2],
                bytes: fields[3]
                    .as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
                    .collect(),
            }
        })
        .collect()
}
