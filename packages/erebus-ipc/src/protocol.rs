use std::ops::RangeInclusive;


/// Protocol versions this build supports. This covers the on-disk [`store`] format too.
pub const SUPPORTED_VERSIONS: RangeInclusive<u32> = 1..=1;

//
// MARK: handshake
//

/// The first message a backend sends after connecting.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Handshake {
    /// list of supported protocol versions by the backend
    pub versions: RangeInclusive<u32>,
    /// name of the app the backend is running in
    pub process: String,
    /// name of the backend that erebus is running in
    pub backend: String,
    /// process ID
    pub pid: u32,
}

/// The frontend's reply to a [`Handshake`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum HandshakeReply {
    Accept { version: u32 },
    Reject,
}

impl HandshakeReply {
    /// Picks the newest version both `SUPPORTED_VERSIONS` and the peer can speak.
    pub fn negotiate_version(handshake: &Handshake) -> Self {
        let lo = *SUPPORTED_VERSIONS.start().max(handshake.versions.start());
        let hi = *SUPPORTED_VERSIONS.end().min(handshake.versions.end());

        if lo <= hi {
            Self::Accept { version: hi }
        } else {
            Self::Reject
        }
    }
}

//
// MARK: messages
//

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FrontendMessage {
    /// The [`store::Store`] on disk or the theme binaries it points to changed.
    Sync,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum BackendMessage {
    Log(LogRecord),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogRecord {
    pub level: log::Level,
    pub target: String,
    pub message: String,
    /// milliseconds since the unix epoch
    pub timestamp: u64,
}
