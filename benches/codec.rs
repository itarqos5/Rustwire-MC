use rustwire_mc::{
    codec::{Reader, Writer},
    version::{Direction, State},
    Limits, Version,
};
use std::{hint::black_box, time::Instant};
fn main() {
    let iterations = 2_000_000;
    let mut w = Writer::new();
    w.var_i32(2_097_151);
    let data = w.into_inner();
    let start = Instant::now();
    let mut sum = 0i64;
    for _ in 0..iterations {
        sum += Reader::new(black_box(&data), Limits::default())
            .var_i32()
            .unwrap() as i64;
    }
    black_box(sum);
    let elapsed = start.elapsed();
    println!(
        "{iterations} 3-byte VarInt decodes in {elapsed:?}; {:.2} ns/op",
        elapsed.as_nanos() as f64 / iterations as f64
    );
    let version = black_box(Version::V26_2);
    let start = Instant::now();
    let mut count = 0;
    for i in 0..iterations {
        count += version
            .packet(
                black_box(State::Play),
                black_box(Direction::Clientbound),
                black_box(i % 180),
            )
            .is_some() as usize;
    }
    black_box(count);
    let elapsed = start.elapsed();
    println!(
        "{iterations} mixed play packet-ID lookups in {elapsed:?}; {:.2} ns/op",
        elapsed.as_nanos() as f64 / iterations as f64
    );
}
