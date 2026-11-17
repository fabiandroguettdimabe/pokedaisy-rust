//! Per-player Windows named-pipe handshake and session lifetime monitor.
//! The pipe ACL admits only the creator/owner and LocalSystem; a per-process
//! random nonce, session, epoch and monotonic frame sequence authenticate the child.

use crate::transport::{TransportFrame, read_frame, validate_frame, write_frame};
use interprocess::local_socket::{
    ConnectOptions, GenericNamespaced, ListenerNonblockingMode, ListenerOptions, prelude::*,
};
use interprocess::os::windows::{
    local_socket::ListenerOptionsExt, security_descriptor::SecurityDescriptor,
};
use serde_json::json;
use std::io::{self, Read};
use std::thread;
use std::time::{Duration, Instant};
use widestring::U16CString;

const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REJECTED_PIPE_CLIENTS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipeEventKind {
    Authenticated,
    Disconnected,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeEvent {
    pub session_id: String,
    pub epoch: u64,
    pub kind: PipeEventKind,
}

pub struct PipeLaunch {
    pub name: String,
    pub nonce: String,
}

pub fn start(
    session_id: String,
    epoch: u64,
    on_event: impl Fn(PipeEvent) + Send + 'static,
) -> Result<PipeLaunch, String> {
    start_with_timeout(session_id, epoch, AUTH_TIMEOUT, on_event)
}

fn start_with_timeout(
    session_id: String,
    epoch: u64,
    timeout: Duration,
    on_event: impl Fn(PipeEvent) + Send + 'static,
) -> Result<PipeLaunch, String> {
    let mut random = [0u8; 32];
    getrandom::fill(&mut random).map_err(|error| format!("No se pudo generar nonce: {error}"))?;
    let nonce = encode_hex(&random);
    let mut suffix = [0u8; 16];
    getrandom::fill(&mut suffix)
        .map_err(|error| format!("No se pudo generar nombre de pipe: {error}"))?;
    let name = format!("vyntra-player-{}", encode_hex(&suffix));
    let owned_name = name
        .as_str()
        .to_ns_name::<GenericNamespaced>()
        .map_err(|error| error.to_string())?
        .into_owned();
    let sddl =
        U16CString::from_str("D:P(A;;GA;;;SY)(A;;GA;;;OW)").map_err(|error| error.to_string())?;
    let security = SecurityDescriptor::deserialize(&sddl).map_err(|error| error.to_string())?;
    let listener = ListenerOptions::new()
        .name(owned_name)
        .security_descriptor(security)
        .create_sync()
        .map_err(|error| format!("No se pudo crear named pipe seguro: {error}"))?;
    listener
        .set_nonblocking(ListenerNonblockingMode::Accept)
        .map_err(|error| error.to_string())?;

    let pipe_nonce = nonce.clone();
    thread::Builder::new()
        .name("vyntra-player-pipe".into())
        .spawn(move || {
            serve_session(listener, session_id, epoch, pipe_nonce, timeout, on_event);
        })
        .map_err(|error| error.to_string())?;
    Ok(PipeLaunch { name, nonce })
}

pub fn connect(
    name: &str,
    nonce: &str,
    session_id: &str,
    epoch: u64,
) -> Result<interprocess::local_socket::Stream, String> {
    let owned_name = name
        .to_ns_name::<GenericNamespaced>()
        .map_err(|error| error.to_string())?
        .into_owned();
    let mut stream = ConnectOptions::new()
        .name(owned_name)
        .connect_sync()
        .map_err(|error| format!("No se pudo conectar al launcher: {error}"))?;
    let hello = TransportFrame {
        protocol_version: crate::domain::CONTRACT_VERSION,
        session_id: session_id.into(),
        epoch,
        nonce: nonce.into(),
        sequence: 1,
        payload: json!({"kind":"hello"}),
    };
    write_frame(&mut stream, &hello).map_err(|error| {
        format!(
            "Handshake rechazado: {error:?} ({})",
            io::Error::last_os_error()
        )
    })?;
    let ack =
        read_frame(&mut stream).map_err(|error| format!("ACK del launcher inválido: {error:?}"))?;
    let mut last_sequence = 0;
    validate_frame(&ack, session_id, epoch, nonce, &mut last_sequence)
        .map_err(|error| format!("ACK del launcher no pertenece a esta sesión: {error:?}"))?;
    if ack.sequence != 1 || ack.payload["kind"] != "hello_ack" {
        return Err("ACK del launcher inesperado".into());
    }
    Ok(stream)
}

fn serve_session(
    listener: interprocess::local_socket::Listener,
    session_id: String,
    epoch: u64,
    nonce: String,
    timeout: Duration,
    on_event: impl Fn(PipeEvent),
) {
    let deadline = Instant::now() + timeout;
    let mut rejected_clients = 0;
    while Instant::now() < deadline && rejected_clients < MAX_REJECTED_PIPE_CLIENTS {
        let mut stream = match listener.accept() {
            Ok(stream) => stream,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(error) => {
                on_event(PipeEvent {
                    session_id,
                    epoch,
                    kind: PipeEventKind::Failed(error.to_string()),
                });
                return;
            }
        };
        rejected_clients += 1;
        if stream.set_nonblocking(true).is_err() {
            continue;
        }
        let hello = match read_frame_with_deadline(&mut stream, deadline) {
            Ok(frame) => frame,
            Err(_) => continue,
        };
        let mut last_sequence = 0;
        if validate_frame(&hello, &session_id, epoch, &nonce, &mut last_sequence).is_err()
            || hello.sequence != 1
            || hello.payload["kind"] != "hello"
        {
            continue;
        }
        if stream.set_nonblocking(false).is_err() {
            continue;
        }
        let ack = TransportFrame {
            protocol_version: crate::domain::CONTRACT_VERSION,
            session_id: session_id.clone(),
            epoch,
            nonce: nonce.clone(),
            sequence: 1,
            payload: json!({"kind":"hello_ack"}),
        };
        if write_frame(&mut stream, &ack).is_err() {
            continue;
        }
        on_event(PipeEvent {
            session_id: session_id.clone(),
            epoch,
            kind: PipeEventKind::Authenticated,
        });
        let mut sink = [0u8; 4096];
        loop {
            match stream.read(&mut sink) {
                Ok(0) => break,
                Ok(_) => {} // Reserved for bounded session events in the next transport step.
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        on_event(PipeEvent {
            session_id,
            epoch,
            kind: PipeEventKind::Disconnected,
        });
        return;
    }
    on_event(PipeEvent {
        session_id,
        epoch,
        kind: PipeEventKind::Failed("No se autenticó el proceso Player dentro del plazo".into()),
    });
}

fn read_frame_with_deadline(
    stream: &mut impl interprocess::local_socket::traits::Stream,
    deadline: Instant,
) -> Result<TransportFrame, String> {
    let mut bytes = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    loop {
        if Instant::now() >= deadline {
            return Err("timeout de handshake".into());
        }
        match stream.read(&mut chunk) {
            // Nonblocking Windows named pipes can report zero bytes while a
            // connection is established but no frame bytes are ready yet.
            Ok(0) => thread::sleep(Duration::from_millis(5)),
            Ok(count) => {
                bytes.extend_from_slice(&chunk[..count]);
                if bytes.len() >= 4 {
                    let length = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
                    if length == 0 || length > crate::transport::MAX_FRAME_BYTES {
                        return Err("longitud inválida".into());
                    }
                    if bytes.len() >= 4 + length {
                        return serde_json::from_slice(&bytes[4..4 + length])
                            .map_err(|error| error.to_string());
                    }
                }
                if bytes.len() > crate::transport::MAX_FRAME_BYTES + 4 {
                    return Err("frame excedido".into());
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{PipeEventKind, connect, start, start_with_timeout};
    use interprocess::local_socket::{ConnectOptions, GenericNamespaced, prelude::*};
    use std::io::Write;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn local_named_pipe_authenticates_child_and_observes_disconnect() {
        let (sender, receiver) = mpsc::channel();
        let launch = start("session-test".into(), 3, move |event| {
            sender.send(event).unwrap()
        })
        .unwrap();
        assert!(connect(&launch.name, "wrong-nonce", "session-test", 3).is_err());
        let stream = connect(&launch.name, &launch.nonce, "session-test", 3).unwrap();
        let authenticated = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(authenticated.kind, PipeEventKind::Authenticated);
        drop(stream);
        let disconnected = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(disconnected.kind, PipeEventKind::Disconnected);
    }

    #[test]
    fn incomplete_handshake_expires() {
        let (sender, receiver) = mpsc::channel();
        let launch = start_with_timeout(
            "session-timeout".into(),
            4,
            Duration::from_millis(120),
            move |event| sender.send(event).unwrap(),
        )
        .unwrap();
        let name = launch
            .name
            .as_str()
            .to_ns_name::<GenericNamespaced>()
            .unwrap()
            .into_owned();
        let mut client = ConnectOptions::new().name(name).connect_sync().unwrap();
        client.write_all(&[0x20, 0x00]).unwrap();
        let event = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(event.kind, PipeEventKind::Failed(_)));
    }
}
