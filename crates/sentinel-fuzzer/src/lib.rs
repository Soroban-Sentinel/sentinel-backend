//! sentinel-fuzzer — orchestrates cargo-fuzz runs against Soroban WASM contracts.
//!
//! Flow:
//!   1. `HarnessGenerator` writes fuzz target source files.
//!   2. `FuzzRunner` spawns `cargo fuzz run` as a subprocess with a timeout.
//!   3. Crash artifacts are parsed into `Finding` structs and returned.

pub mod runner;
pub mod corpus;
pub mod coverage;
