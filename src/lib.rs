//! Rustwire: bounded Minecraft Java protocol building blocks.
//!
//! The default build uses a synchronous standard-library transport and pure-Rust
//! compression. Enable `crypto` for online-mode encryption and `auth` for the
//! optional Microsoft/Xbox/Minecraft HTTP flow. See the README coverage matrix:
//! packet catalogs are not a promise that every play packet has a typed codec.
#![forbid(unsafe_code)]
pub mod chunk;
pub mod codec;
pub mod error;
pub mod frame;
pub mod nbt;
pub mod registry;
pub mod version;
pub use error::{Error, Result};
pub use version::Version;

/// Resource budgets used before allocations, recursion, and decompression.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_frame: usize,
    pub max_packet: usize,
    pub max_string_chars: usize,
    pub max_collection: usize,
    pub max_nbt_depth: usize,
    pub max_nbt_nodes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_frame: 2_097_151,
            max_packet: 8 * 1024 * 1024,
            max_string_chars: 32767,
            max_collection: 1_048_576,
            max_nbt_depth: 64,
            max_nbt_nodes: 1_048_576,
        }
    }
}
