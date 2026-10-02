use rustwire_mc::{connection::Connection, Limits, Version};
use std::time::Duration;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let host = args.get(1).map(String::as_str).unwrap_or("127.0.0.1");
    let port = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(25565);
    let version = args
        .get(3)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(Version::V1_21);
    let mut c = Connection::connect(
        (host, port),
        version,
        Duration::from_secs(10),
        Limits::default(),
    )?;
    let status = c.status(host, port, 42)?;
    println!("{}\nround trip: {:?}", status.json, status.round_trip);
    Ok(())
}
