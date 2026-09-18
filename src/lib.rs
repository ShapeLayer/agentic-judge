pub mod ai;
pub mod api;
pub mod engine;
pub mod mcp;
pub mod model;
pub mod runtime;
pub mod storage;
pub mod builtin {
    include!(concat!(env!("OUT_DIR"), "/checkers.rs"));
}
