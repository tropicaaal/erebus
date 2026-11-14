
use std::{
    io::{self, Read, Write},
};

use interprocess::local_socket::{GenericFilePath, GenericNamespaced, Name, ToFsName, ToNsName};
use serde::{Serialize, de::DeserializeOwned};

pub fn socket_name() -> io::Result<Name<'static>> {
    match dirs::runtime_dir() {
        Some(dir) => dir.join("erebus.sock").to_fs_name::<GenericFilePath>(),
        None => "erebus".to_ns_name::<GenericNamespaced>(),
    }
}

pub fn send<T: Serialize>(mut writer: impl Write, message: &T) -> io::Result<()> {
    let payload = postcard::to_allocvec(message).map_err(io::Error::other)?;
    let len = u32::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "payload length too large"))?;

    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(&payload);

    writer.write_all(&frame)?;
    writer.flush()
}

pub fn recv<T: DeserializeOwned>(mut reader: impl Read) -> io::Result<T> {
    let mut len = [0; 4];
    reader.read_exact(&mut len)?;

    let len = u32::from_le_bytes(len);

    let mut payload = vec![0; len as usize];
    reader.read_exact(&mut payload)?;

    postcard::from_bytes(&payload).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}
