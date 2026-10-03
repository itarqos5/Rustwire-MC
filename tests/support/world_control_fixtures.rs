use rustwire_mc::Version;

pub struct Fixture {
    pub version: Version,
    pub name: &'static str,
    pub id: i32,
    pub case: &'static str,
    pub bytes: Vec<u8>,
}
pub fn fixtures() -> Vec<Fixture> {
    include_str!("../fixtures/world-control.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let parts: Vec<_> = line.split('\t').collect();
            assert_eq!(parts.len(), 5);
            assert_eq!(parts[4].len() % 2, 0, "odd fixture hex length");
            let bytes = parts[4]
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            Fixture {
                version: Version::from_protocol(parts[0].parse().unwrap()).unwrap(),
                name: parts[1],
                id: parts[2].parse().unwrap(),
                case: parts[3],
                bytes,
            }
        })
        .collect()
}
