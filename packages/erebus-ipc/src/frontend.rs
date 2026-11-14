//! erebus frontend IPC server implementation

use std::{
    io,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    thread,
};

use interprocess::local_socket::{ListenerOptions, Stream, traits::ListenerExt};

use crate::protocol::{BackendMessage, FrontendMessage, Handshake, HandshakeReply, LogRecord};
use crate::transport;

pub type PeerId = usize;

/// Something a backend did.
///
/// This is reported through the callback given to [`IpcFrontend::start`].
pub enum Event {
    Connected {
        peer: PeerId,
        process: String,
        backend: String,
        pid: u32,
        /// the protocol version we agreed on
        version: u32,
    },
    Disconnected {
        peer: PeerId,
    },
    Log {
        peer: PeerId,
        record: LogRecord,
    },
}

pub struct IpcFrontend {
    peers: Peers,
}

struct Peer {
    id: PeerId,
    process: String,
    stream: Arc<Stream>,
}

type Peers = Arc<Mutex<Vec<Peer>>>;
type OnEvent = Arc<dyn Fn(Event) + Send + Sync>;

fn lock(peers: &Peers) -> MutexGuard<'_, Vec<Peer>> {
    peers.lock().unwrap_or_else(PoisonError::into_inner)
}

impl IpcFrontend {
    /// Starts listening on [`socket_name`](transport::socket_name). `on_event` is called from
    /// background threads.
    pub fn start(on_event: impl Fn(Event) + Send + Sync + 'static) -> io::Result<Self> {
        let listener = ListenerOptions::new()
            .name(transport::socket_name()?)
            .try_overwrite(true)
            .create_sync()?;

        let on_event: OnEvent = Arc::new(on_event);
        let peers = Peers::default();

        thread::spawn({
            let peers = peers.clone();
            move || {
                for (id, stream) in listener.incoming().filter_map(Result::ok).enumerate() {
                    let peers = peers.clone();
                    let on_event = on_event.clone();

                    thread::spawn(move || {
                        if let Err(err) = serve(id, stream, &peers, &*on_event) {
                            eprintln!("backend connection failed: {err}");
                        }
                    });
                }
            }
        });

        Ok(Self { peers })
    }

    /// Tells the backends of `process`, or every backend if `None`, to reload the store.
    pub fn sync(&self, process: Option<&str>) {
        let peers = lock(&self.peers);
        let targets = peers
            .iter()
            .filter(|peer| process.is_none_or(|process| peer.process == process));

        for peer in targets {
            // a failed peer gets cleaned up by its own thread once it notices the hangup
            let _ = transport::send(&*peer.stream, &FrontendMessage::Sync);
        }
    }
}

fn serve(
    id: PeerId,
    stream: Stream,
    peers: &Peers,
    on_event: &(dyn Fn(Event) + Send + Sync),
) -> io::Result<()> {
    let handshake: Handshake = transport::recv(&stream)?;
    let reply = HandshakeReply::negotiate_version(&handshake);

    let (stream, version) = {
        // reply and register under the lock, so no sync can be sent before the reply and
        // none are missed after it
        let mut peers = lock(peers);
        transport::send(&stream, &reply)?;

        let HandshakeReply::Accept { version } = reply else {
            return Ok(());
        };

        let stream = Arc::new(stream);
        peers.push(Peer {
            id,
            process: handshake.process.clone(),
            stream: stream.clone(),
        });
        (stream, version)
    };

    on_event(Event::Connected {
        peer: id,
        process: handshake.process,
        pid: handshake.pid,
        backend: handshake.backend,
        version,
    });

    let ended = loop {
        match transport::recv(&*stream) {
            Ok(BackendMessage::Log(record)) => {
                on_event(Event::Log { peer: id, record });
            }
            Err(err) => break err,
        }
    };

    lock(peers).retain(|peer| peer.id != id);
    on_event(Event::Disconnected { peer: id });

    // hanging up is the normal way for a backend to leave
    match ended.kind() {
        io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset => Ok(()),
        _ => Err(ended),
    }
}
