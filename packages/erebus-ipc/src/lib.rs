//! Wire protocol spoken between erebus backends (injected into the target app) and an
//! external frontend application.
//!
//! Works roughly like this:
//! - Frontend opens a socket and awaits data from backends.
//! - Each backend connects to the socket, does a handshake where protocol version is
//!   negotiated, etc... Connection is established.
//! - Whenever the frontend modifies something that the backend needs to see, it sends a
//!   Sync packet to every backend on the connection, which informs them to re-read the config
//!   directory (see [`config`]).
//!
//! All messages message are a little-endian `u32` length followed by that many bytes of
//! postcard-encoded data.
//!
//! [`backend::IpcBackend`] and [`frontend::IpcFrontend`] implement each side of the protocol.

pub mod protocol;
pub mod transport;
pub mod config;
pub mod frontend;
pub mod backend;