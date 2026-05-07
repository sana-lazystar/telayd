//! Library target for telayd-daemon.
//!
//! Exposes internal modules so that integration tests under `tests/`
//! can exercise the daemon's in-process components without spinning up
//! a real socket or process.
//!
//! All modules declared here must also be declared in `main.rs`.
//! (Rust requires separate module trees for `main.rs` and `lib.rs`.)

#![forbid(unsafe_code)]

pub mod cf_tunnel;
pub mod cli;
pub mod config;
pub mod hook_installer;
pub mod ipc;
pub mod logger;
pub mod metrics;
pub mod notification;
pub mod paths;
pub mod pairing;
pub mod protocol;
pub mod sentinel;
pub mod supervisor;
pub mod tmux_controller;
pub mod tmux_keymap;
pub mod ws_bridge;
