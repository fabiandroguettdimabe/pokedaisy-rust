//! Per-player Windows named-pipe handshake and session lifetime monitor.
//! The pipe ACL admits only the creator/owner and LocalSystem; a per-process
//! random nonce, session, epoch and monotonic frame sequence authenticate the child.

use crate::transport::{
    PlayerCommand, PlayerCommandQueue, PlayerCommandSender, TransportFrame, read_frame,
    validate_frame, write_frame,
};
use interprocess::local_socket::{
    ConnectOptions, GenericNamespaced, ListenerNonblockingMode, ListenerOptions, prelude::*,
};
use interprocess::os::windows::{
    local_socket::ListenerOptionsExt, security_descriptor::SecurityDescriptor,
};
use serde_json::{Value, json};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use widestring::U16CString;

const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
pub const RECONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REJECTED_PIPE_CLIENTS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipeEventKind {
    Authenticated,
    Cancelled,
    Snapshot(Value),
    CommandResult {
        request_id: u64,
        command: String,
        result: Result<(), String>,
    },
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
    pub commands: PlayerCommandSender,
    pub cancellation: PipeCancellation,
}

#[derive(Clone, Default)]
pub struct PipeCancellation(Arc<AtomicBool>);

impl PipeCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
}

type PipeSendHalf =
    <interprocess::local_socket::Stream as interprocess::local_socket::traits::Stream>::SendHalf;
pub struct PlayerPipeClient {
    send: PipeSendHalf,
    commands: PlayerCommandQueue,
    reader_cancel: Arc<AtomicBool>,
    session_id: String,
    nonce: String,
    epoch: u64,
    sequence: u64,
}

impl Drop for PlayerPipeClient {
    fn drop(&mut self) {
        self.reader_cancel.store(true, Ordering::Release);
    }
}

impl PlayerPipeClient {
    pub fn try_command(&self) -> Result<Option<PlayerCommand>, crate::transport::QueueError> {
        self.commands.try_next()
    }

    pub fn send_snapshot(&mut self, payload: Value) -> Result<(), String> {
        self.sequence = self.sequence.saturating_add(1);
        let frame = TransportFrame {
            protocol_version: crate::domain::CONTRACT_VERSION,
            session_id: self.session_id.clone(),
            epoch: self.epoch,
            nonce: self.nonce.clone(),
            sequence: self.sequence,
            payload: json!({"kind":"snapshot","data":payload}),
        };
        write_frame_cancellable(&mut self.send, &frame, &self.reader_cancel)
            .map_err(|error| format!("No se pudo enviar snapshot Player: {error:?}"))
    }

    pub fn send_command_result(
        &mut self,
        request_id: u64,
        command: &str,
        result: Result<(), String>,
    ) -> Result<(), String> {
        self.sequence = self.sequence.saturating_add(1);
        let payload = match result {
            Ok(()) => {
                json!({"kind":"command_result","request_id":request_id,"command":command,"ok":true})
            }
            Err(error) => {
                json!({"kind":"command_result","request_id":request_id,"command":command,"ok":false,"error":error})
            }
        };
        let frame = TransportFrame {
            protocol_version: crate::domain::CONTRACT_VERSION,
            session_id: self.session_id.clone(),
            epoch: self.epoch,
            nonce: self.nonce.clone(),
            sequence: self.sequence,
            payload,
        };
        write_frame_cancellable(&mut self.send, &frame, &self.reader_cancel)
            .map_err(|error| format!("No se pudo confirmar comando Player: {error:?}"))
    }
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

    let command_queue = Arc::new(Mutex::new(PlayerCommandQueue::new(16)));
    let command_sender = command_queue
        .lock()
        .map_err(|_| "Cola de comandos Player envenenada".to_owned())?
        .sender();
    let cancellation = PipeCancellation::default();
    let worker_cancellation = Arc::clone(&cancellation.0);
    let pipe_nonce = nonce.clone();
    thread::Builder::new()
        .name("vyntra-player-pipe".into())
        .spawn(move || {
            serve_session(
                listener,
                session_id,
                epoch,
                pipe_nonce,
                timeout,
                command_queue,
                worker_cancellation,
                on_event,
            );
        })
        .map_err(|error| error.to_string())?;
    Ok(PipeLaunch {
        name,
        nonce,
        commands: command_sender,
        cancellation,
    })
}

pub fn connect(
    name: &str,
    nonce: &str,
    session_id: &str,
    epoch: u64,
) -> Result<PlayerPipeClient, String> {
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
    stream
        .set_nonblocking(true)
        .map_err(|error| format!("No se pudo habilitar lectura cancelable: {error}"))?;
    let (mut recv, send) = stream.split();
    let commands = PlayerCommandQueue::new(16);
    let command_sender = commands.sender();
    let reader_cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&reader_cancel);
    let reader_session = session_id.to_owned();
    let reader_nonce = nonce.to_owned();
    thread::Builder::new()
        .name("vyntra-player-command-reader".into())
        .spawn(move || {
            let mut last_sequence = 1;
            while let Ok(Some(frame)) = read_frame_cancellable(&mut recv, &worker_cancel) {
                if validate_frame(
                    &frame,
                    &reader_session,
                    epoch,
                    &reader_nonce,
                    &mut last_sequence,
                )
                .is_err()
                {
                    break;
                }
                let command = parse_player_command(&frame.payload);
                let Some(command) = command else {
                    break;
                };
                if command_sender.send(command).is_err() {
                    break;
                }
            }
        })
        .map_err(|error| error.to_string())?;
    Ok(PlayerPipeClient {
        send,
        commands,
        reader_cancel,
        session_id: session_id.into(),
        nonce: nonce.into(),
        epoch,
        sequence: 1,
    })
}

fn serve_session(
    listener: interprocess::local_socket::Listener,
    session_id: String,
    epoch: u64,
    nonce: String,
    timeout: Duration,
    command_queue: Arc<Mutex<PlayerCommandQueue>>,
    cancellation: Arc<AtomicBool>,
    on_event: impl Fn(PipeEvent),
) {
    let mut deadline = Instant::now() + timeout;
    let mut rejected_clients = 0;
    let mut authenticated_once = false;
    loop {
        if cancellation.load(Ordering::Acquire) {
            on_event(PipeEvent {
                session_id,
                epoch,
                kind: PipeEventKind::Cancelled,
            });
            return;
        }
        if Instant::now() >= deadline || rejected_clients >= MAX_REJECTED_PIPE_CLIENTS {
            let message = if authenticated_once {
                "No se restableció el canal de Player dentro del plazo"
            } else {
                "No se autenticó el proceso Player dentro del plazo"
            };
            on_event(PipeEvent {
                session_id,
                epoch,
                kind: PipeEventKind::Failed(message.into()),
            });
            return;
        }
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
        let hello = match read_frame_with_deadline(&mut stream, deadline, &cancellation) {
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
        authenticated_once = true;
        let (mut recv, send) = stream.split();
        let alive = Arc::new(AtomicBool::new(true));
        let writer_alive = Arc::clone(&alive);
        let writer_session = session_id.clone();
        let writer_nonce = nonce.clone();
        let writer_queue = Arc::clone(&command_queue);
        let writer = thread::Builder::new()
            .name("vyntra-player-command-writer".into())
            .spawn(move || {
                let mut sequence = 1u64;
                while writer_alive.load(Ordering::Acquire) {
                    let next_command = match writer_queue.lock() {
                        Ok(queue) => queue.try_next(),
                        Err(_) => break,
                    };
                    match next_command {
                        Ok(Some(command)) => {
                            sequence = sequence.saturating_add(1);
                            let stop = command == PlayerCommand::Stop;
                            let frame = TransportFrame {
                                protocol_version: crate::domain::CONTRACT_VERSION,
                                session_id: writer_session.clone(),
                                epoch,
                                nonce: writer_nonce.clone(),
                                sequence,
                                payload: command_payload(command),
                            };
                            if write_frame(&mut &send, &frame).is_err() {
                                break;
                            }
                            if stop {
                                break;
                            }
                        }
                        Ok(None) => thread::sleep(Duration::from_millis(8)),
                        Err(_) => break,
                    }
                }
            });
        let mut last_sequence = 1u64;
        loop {
            match read_frame(&mut recv) {
                Ok(frame) => {
                    if validate_frame(&frame, &session_id, epoch, &nonce, &mut last_sequence)
                        .is_err()
                    {
                        break;
                    }
                    if frame.payload["kind"] == "snapshot" {
                        if let Some(payload) = frame.payload.get("data") {
                            on_event(PipeEvent {
                                session_id: session_id.clone(),
                                epoch,
                                kind: PipeEventKind::Snapshot(payload.clone()),
                            });
                        }
                    } else if frame.payload["kind"] == "command_result" {
                        let (Some(request_id), Some(command), Some(ok)) = (
                            frame.payload["request_id"].as_u64(),
                            frame.payload["command"].as_str(),
                            frame.payload["ok"].as_bool(),
                        ) else {
                            break;
                        };
                        if request_id == 0 || !matches!(command, "save_state" | "load_state") {
                            break;
                        }
                        let result = if ok {
                            Ok(())
                        } else {
                            Err(frame.payload["error"]
                                .as_str()
                                .unwrap_or("Player rechazó la operación")
                                .to_owned())
                        };
                        on_event(PipeEvent {
                            session_id: session_id.clone(),
                            epoch,
                            kind: PipeEventKind::CommandResult {
                                request_id,
                                command: command.to_owned(),
                                result,
                            },
                        });
                    } else {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        alive.store(false, Ordering::Release);
        if let Ok(writer) = writer {
            let _ = writer.join();
        }
        on_event(PipeEvent {
            session_id: session_id.clone(),
            epoch,
            kind: PipeEventKind::Disconnected,
        });
        deadline = Instant::now() + RECONNECT_TIMEOUT;
        rejected_clients = 0;
    }
}

fn parse_player_command(payload: &Value) -> Option<PlayerCommand> {
    if payload["kind"] != "command" {
        return None;
    }
    match payload["command"].as_str()? {
        "stop" => Some(PlayerCommand::Stop),
        "pause" => payload["value"].as_bool().map(PlayerCommand::Pause),
        "save_state" => payload["request_id"]
            .as_u64()
            .filter(|id| *id > 0)
            .map(PlayerCommand::SaveState),
        "load_state" => payload["request_id"]
            .as_u64()
            .filter(|id| *id > 0)
            .map(PlayerCommand::LoadState),
        _ => None,
    }
}

fn command_payload(command: PlayerCommand) -> Value {
    match command {
        PlayerCommand::Pause(value) => json!({"kind":"command","command":"pause","value":value}),
        PlayerCommand::SaveState(request_id) => {
            json!({"kind":"command","command":"save_state","request_id":request_id})
        }
        PlayerCommand::LoadState(request_id) => {
            json!({"kind":"command","command":"load_state","request_id":request_id})
        }
        PlayerCommand::Stop => json!({"kind":"command","command":"stop"}),
    }
}

fn read_frame_with_deadline(
    stream: &mut impl interprocess::local_socket::traits::Stream,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<TransportFrame, String> {
    let mut bytes = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    loop {
        if cancellation.load(Ordering::Acquire) {
            return Err("handshake cancelado".into());
        }
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

fn read_frame_cancellable(
    stream: &mut impl io::Read,
    cancellation: &AtomicBool,
) -> Result<Option<TransportFrame>, String> {
    let mut prefix = [0u8; 4];
    if !read_exact_cancellable(stream, &mut prefix, cancellation)? {
        return Ok(None);
    }
    let size = u32::from_le_bytes(prefix) as usize;
    if size == 0 || size > crate::transport::MAX_FRAME_BYTES {
        return Err("longitud de frame inválida".into());
    }
    let mut body = vec![0; size];
    if !read_exact_cancellable(stream, &mut body, cancellation)? {
        return Ok(None);
    }
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| format!("frame JSON inválido: {error}"))
}

fn read_exact_cancellable(
    stream: &mut impl io::Read,
    bytes: &mut [u8],
    cancellation: &AtomicBool,
) -> Result<bool, String> {
    let mut filled = 0;
    while filled < bytes.len() {
        if cancellation.load(Ordering::Acquire) {
            return Ok(false);
        }
        match stream.read(&mut bytes[filled..]) {
            Ok(0) => thread::sleep(Duration::from_millis(5)),
            Ok(count) => filled += count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(true)
}

fn write_frame_cancellable(
    writer: &mut impl io::Write,
    frame: &TransportFrame,
    cancellation: &AtomicBool,
) -> Result<(), String> {
    let body = serde_json::to_vec(frame).map_err(|error| error.to_string())?;
    if body.is_empty() || body.len() > crate::transport::MAX_FRAME_BYTES {
        return Err("tamaño de frame inválido".into());
    }
    write_all_cancellable(writer, &(body.len() as u32).to_le_bytes(), cancellation)?;
    write_all_cancellable(writer, &body, cancellation)
}

fn write_all_cancellable(
    writer: &mut impl io::Write,
    bytes: &[u8],
    cancellation: &AtomicBool,
) -> Result<(), String> {
    let mut written = 0;
    while written < bytes.len() {
        if cancellation.load(Ordering::Acquire) {
            return Err("pipe cancelado".into());
        }
        match writer.write(&bytes[written..]) {
            Ok(0) => return Err("el pipe dejó de aceptar datos".into()),
            Ok(count) => written += count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
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
    use super::{PipeEventKind, connect, parse_player_command, start, start_with_timeout};
    use crate::transport::PlayerCommand;
    use interprocess::local_socket::{ConnectOptions, GenericNamespaced, prelude::*};
    use serde_json::json;
    use std::io::Write;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn local_named_pipe_authenticates_child_and_exchanges_commands_and_snapshots() {
        let (sender, receiver) = mpsc::channel();
        let launch = start("session-test".into(), 3, move |event| {
            sender.send(event).unwrap()
        })
        .unwrap();
        assert!(connect(&launch.name, "wrong-nonce", "session-test", 3).is_err());
        let mut stream = connect(&launch.name, &launch.nonce, "session-test", 3).unwrap();
        let authenticated = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(authenticated.kind, PipeEventKind::Authenticated);
        launch.commands.send(PlayerCommand::Stop).unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if stream.try_command().unwrap() == Some(PlayerCommand::Stop) {
                break;
            }
            assert!(std::time::Instant::now() < until, "Stop no llegó al Player");
            std::thread::sleep(Duration::from_millis(5));
        }
        stream.send_snapshot(json!({"title":"test"})).unwrap();
        let snapshot = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(
            snapshot.kind,
            PipeEventKind::Snapshot(json!({"title":"test"}))
        );
        stream
            .send_command_result(17, "save_state", Err("No hay espacio".into()))
            .unwrap();
        let result = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(
            result.kind,
            PipeEventKind::CommandResult {
                request_id: 17,
                command: "save_state".into(),
                result: Err("No hay espacio".into()),
            }
        );
        drop(stream);
    }

    #[test]
    fn authenticated_player_can_reconnect_and_keep_session_commands() {
        let (sender, receiver) = mpsc::channel();
        let launch = start("session-reconnect".into(), 8, move |event| {
            sender.send(event).unwrap()
        })
        .unwrap();
        let first = connect(&launch.name, &launch.nonce, "session-reconnect", 8).unwrap();
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap().kind,
            PipeEventKind::Authenticated
        );

        drop(first);
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap().kind,
            PipeEventKind::Disconnected
        );

        let second = connect(&launch.name, &launch.nonce, "session-reconnect", 8).unwrap();
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap().kind,
            PipeEventKind::Authenticated
        );
        launch.commands.send(PlayerCommand::Pause(true)).unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if second.try_command().unwrap() == Some(PlayerCommand::Pause(true)) {
                break;
            }
            assert!(
                std::time::Instant::now() < until,
                "comando no sobrevivió a la reconexión"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        drop(second);
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap().kind,
            PipeEventKind::Disconnected
        );
        launch.cancellation.cancel();
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap().kind,
            PipeEventKind::Cancelled
        );
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

    #[test]
    fn pending_handshake_can_be_cancelled_before_its_timeout() {
        let (sender, receiver) = mpsc::channel();
        let launch = start_with_timeout(
            "session-cancel".into(),
            5,
            Duration::from_secs(10),
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

        launch.cancellation.cancel();

        let event = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(event.kind, PipeEventKind::Cancelled);
    }

    #[test]
    fn state_commands_require_a_positive_request_id() {
        assert_eq!(
            parse_player_command(&json!({"kind":"command","command":"save_state","request_id":17})),
            Some(PlayerCommand::SaveState(17))
        );
        assert_eq!(
            parse_player_command(&json!({"kind":"command","command":"load_state","request_id":0})),
            None
        );
    }
}
