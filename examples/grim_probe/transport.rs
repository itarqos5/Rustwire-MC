//! Test-only plaintext duplex transport. A single blocking reader owns Connection;
//! every writer holds the same lock for an entire encoded frame, never a prefix.
use rustwire_mc::{
    codec::Writer,
    connection::{Connection, Event, TypedEvent},
    frame::{FrameCodec, RawPacket},
    packet::{
        self,
        common::{self, CommonPacket},
        typed::DecodedPacket,
        ClientSettings,
    },
    Error, Limits, Result, Version,
};
use std::{
    io::{self, Read, Write},
    net::{Shutdown, SocketAddr, TcpStream},
    sync::{
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Clone)]
struct FrameWriter(Arc<Mutex<TcpStream>>);
impl FrameWriter {
    fn frame(&self, bytes: &[u8]) -> io::Result<()> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("frame writer poisoned"))?
            .write_all(bytes)
    }
}
struct Duplex {
    read: TcpStream,
    write: FrameWriter,
}
impl Read for Duplex {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.read.read(bytes)
    }
}
impl Write for Duplex {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write.frame(bytes)?;
        Ok(bytes.len())
    }
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.write.frame(bytes)
    }
    // TCP is unbuffered. Connection calls flush after each full-frame write.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct Transport {
    pub events: Receiver<std::result::Result<TypedEvent, String>>,
    writer: FrameWriter,
    codec: FrameCodec,
    shutdown: TcpStream,
    reader: Option<JoinHandle<()>>,
    position_completed: Option<SyncSender<()>>,
}
impl Transport {
    pub fn connect(address: SocketAddr, version: Version, limits: Limits) -> Result<Self> {
        if !address.ip().is_loopback() {
            return Err(Error::Invalid("grim probe requires a loopback address"));
        }
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(10))?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_secs(20)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        let shutdown = stream.try_clone()?;
        let writer = FrameWriter(Arc::new(Mutex::new(stream.try_clone()?)));
        let mut connection = Connection::new(
            Duplex {
                read: stream,
                write: writer.clone(),
            },
            version,
            limits,
        );
        connection.start_login(
            &address.ip().to_string(),
            address.port(),
            "Rustwire",
            [0; 16],
        )?;
        let (tx, events) = mpsc::sync_channel(64);
        let (position_completed, position_barrier) = mpsc::sync_channel(1);
        let reader = thread::spawn(move || {
            if let Err(error) = read_events(&mut connection, version, &tx, &position_barrier) {
                let _ = tx.send(Err(error.to_string()));
            }
        });
        Ok(Self {
            events,
            writer,
            codec: FrameCodec::new(limits),
            shutdown,
            reader: Some(reader),
            position_completed: Some(position_completed),
        })
    }
    #[cfg(test)]
    pub fn capture() -> Result<(Self, TcpStream)> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let client = TcpStream::connect(listener.local_addr()?)?;
        let (server, _) = listener.accept()?;
        client.set_write_timeout(Some(Duration::from_secs(5)))?;
        server.set_read_timeout(Some(Duration::from_secs(5)))?;
        Ok((
            Self {
                events: mpsc::channel().1,
                writer: FrameWriter(Arc::new(Mutex::new(client.try_clone()?))),
                codec: FrameCodec::default(),
                shutdown: client,
                reader: None,
                position_completed: None,
            },
            server,
        ))
    }
    pub fn complete_position(&self) -> Result<()> {
        self.position_completed
            .as_ref()
            .ok_or(Error::State("position barrier absent"))?
            .send(())
            .map_err(|_| Error::State("position reader stopped"))
    }
    pub fn compression(&mut self, threshold: Option<usize>) -> Result<()> {
        self.codec.set_compression(threshold)
    }
    pub fn send(&self, packet: &RawPacket) -> Result<()> {
        self.writer.frame(&self.codec.encode(packet)?)?;
        Ok(())
    }
    pub fn stop(&mut self) -> Result<()> {
        let _ = self.shutdown.shutdown(Shutdown::Both);
        // Dropping the old receiver cancels a producer blocked on a full queue.
        // Socket shutdown alone would not unblock SyncSender::send.
        self.events = mpsc::channel().1;
        self.position_completed.take();
        if let Some(reader) = self.reader.take() {
            reader
                .join()
                .map_err(|_| Error::State("probe reader panicked"))?;
        }
        Ok(())
    }
}
impl Drop for Transport {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
fn read_events(
    connection: &mut Connection<Duplex>,
    version: Version,
    tx: &SyncSender<std::result::Result<TypedEvent, String>>,
    position_barrier: &Receiver<()>,
) -> Result<()> {
    for _ in 0..100_000 {
        let event = connection.next_typed_event()?;
        let position = matches!(&event, TypedEvent::Control(Event::Position(_)));
        match &event {
            TypedEvent::Control(Event::EncryptionRequested(_)) => return Err(Error::Unsupported("grim probe requires plaintext offline mode; online authentication and encryption are not supported")),
            TypedEvent::Control(Event::LoginSuccess(_)) if version.has_configuration() => { connection.send_settings(&ClientSettings::default())?; send_brand(connection, version)?; },
            TypedEvent::Control(Event::Joined(_)) if !version.has_configuration() => { connection.send_settings(&ClientSettings::default())?; send_brand(connection, version)?; },
            TypedEvent::Control(Event::KnownPacks(_)) => connection.select_known_packs(&[])?,
            TypedEvent::Control(Event::CookieRequest(key)) => connection.answer_cookie(key, None)?,
            TypedEvent::Control(Event::LoginPluginRequest { id, .. }) => connection.answer_login_plugin(*id, None)?,
            TypedEvent::Control(Event::Reconfigure) => return Err(Error::Unsupported("reconfiguration during bounded Grim probe")),
            TypedEvent::Decoded(DecodedPacket::Common(CommonPacket::ChunkBatchFinished(_))) => connection.send(&common::chunk_batch_received(version, 20.)?)?,
            TypedEvent::Raw { name: Some("map_chunk" | "update_light"), .. } => continue,
            _ => {}
        }
        if tx.send(Ok(event)).is_err() {
            return Ok(());
        }
        // Preserve the server's packet order across the two client threads:
        // a following Ping must not receive its automatic Pong before the main
        // thread finishes the preceding teleport ACK and resolved PosRot reply.
        if position
            && position_barrier
                .recv_timeout(Duration::from_secs(5))
                .is_err()
        {
            return Err(Error::State(
                "position handling barrier cancelled or timed out",
            ));
        }
    }
    Err(Error::Limit("grim probe received packet budget"))
}

fn send_brand(connection: &mut Connection<Duplex>, version: Version) -> Result<()> {
    let mut payload = Writer::new();
    payload.string("rustwire-test", 32767)?;
    connection.send(&packet::custom_payload(
        version,
        connection.state(),
        "minecraft:brand",
        payload.as_slice(),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn teleport_response_precedes_pong_for_a_later_coalesced_ping() {
        use rustwire_mc::{
            packet::{movement::PlayerMovement, PositionSync},
            version::{Direction, State},
        };
        let version = Version::V26_2;
        let limits = Limits::default();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut connection = Connection::new(stream, version, limits);
            connection.receive().unwrap(); // handshake
            connection.receive().unwrap(); // login start
            let mut success = Writer::new();
            success.raw(&[0; 16]);
            success.string("Rustwire", 16).unwrap();
            success.var_i32(0);
            success.raw(&[0; 16]); // protocol 776 login session ID
            connection
                .send(&RawPacket::new(
                    version
                        .packet_id(State::Login, Direction::Clientbound, "success")
                        .unwrap(),
                    success.into_inner(),
                ))
                .unwrap();
            for _ in 0..3 {
                connection.receive().unwrap();
            } // ack, settings, truthful brand
            connection
                .send(&RawPacket::new(
                    version
                        .packet_id(
                            State::Configuration,
                            Direction::Clientbound,
                            "finish_configuration",
                        )
                        .unwrap(),
                    [],
                ))
                .unwrap();
            connection.receive().unwrap(); // configuration acknowledgement
            let mut position = Writer::new();
            position.var_i32(1);
            for n in [0.5, -60., 0.5, 0., 0., 0.] {
                position.f64(n);
            }
            position.f32(0.);
            position.f32(0.);
            position.i32(0);
            let mut ping = Writer::new();
            ping.i32(42);
            let codec = FrameCodec::new(limits);
            let mut frames = codec
                .encode(&RawPacket::new(
                    version
                        .packet_id(State::Play, Direction::Clientbound, "position")
                        .unwrap(),
                    position.into_inner(),
                ))
                .unwrap();
            frames.extend(
                codec
                    .encode(&RawPacket::new(
                        version
                            .packet_id(State::Play, Direction::Clientbound, "ping")
                            .unwrap(),
                        ping.into_inner(),
                    ))
                    .unwrap(),
            );
            let mut stream = connection.into_inner();
            stream.write_all(&frames).unwrap();
            stream
        });
        let mut transport = Transport::connect(address, version, limits).unwrap();
        let position: PositionSync = loop {
            if let TypedEvent::Control(Event::Position(p)) = transport
                .events
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap()
            {
                break p;
            }
        };
        let server = server.join().unwrap();
        // Hold the application event unprocessed. The old auto-reply reader
        // emitted Pong here, allowing it to overtake the pending teleport.
        server
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let error = server.peek(&mut [0; 1]).unwrap_err();
        assert!(matches!(
            error.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
        ));
        transport
            .send(&position.acknowledgement(version).unwrap())
            .unwrap();
        transport
            .send(
                &PlayerMovement {
                    position: Some([position.x, position.y, position.z]),
                    rotation: Some([position.yaw, position.pitch]),
                    on_ground: false,
                    horizontal_collision: false,
                }
                .packet(version, limits)
                .unwrap(),
            )
            .unwrap();
        transport.complete_position().unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut capture = Connection::new(server, version, limits);
        for name in ["teleport_confirm", "position_look", "pong"] {
            assert_eq!(
                capture.receive().unwrap().id,
                version
                    .packet_id(State::Play, Direction::Serverbound, name)
                    .unwrap(),
                "{name}"
            );
        }
        transport.stop().unwrap();
    }

    #[test]
    fn shutdown_cancels_a_reader_waiting_for_position_completion() {
        let (mut transport, _server) = Transport::capture().unwrap();
        let (completed, barrier) = mpsc::sync_channel(1);
        let (ready_tx, ready_rx) = mpsc::channel();
        transport.position_completed = Some(completed);
        transport.reader = Some(thread::spawn(move || {
            ready_tx.send(()).unwrap();
            assert!(barrier.recv().is_err());
        }));
        ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        transport.stop().unwrap();
        assert!(transport.reader.is_none());
    }

    #[test]
    fn shutdown_cancels_a_reader_blocked_on_a_full_event_queue() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_server, _) = listener.accept().unwrap();
        let writer = FrameWriter(Arc::new(Mutex::new(client.try_clone().unwrap())));
        let (tx, events) = mpsc::sync_channel(1);
        let (ready_tx, ready_rx) = mpsc::channel();
        let reader = thread::spawn(move || {
            tx.send(Ok(TypedEvent::Control(Event::Ready))).unwrap();
            ready_tx.send(()).unwrap();
            assert!(tx.send(Ok(TypedEvent::Control(Event::Ready))).is_err());
        });
        ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let mut transport = Transport {
            events,
            writer,
            codec: FrameCodec::default(),
            shutdown: client,
            reader: Some(reader),
            position_completed: None,
        };
        transport.stop().unwrap();
        assert!(transport.reader.is_none());
    }

    #[test]
    fn concurrent_writes_preserve_complete_frames() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        client
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let (mut server, _) = listener.accept().unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let writer = FrameWriter(Arc::new(Mutex::new(client)));
        let workers: Vec<_> = (1_u8..=2)
            .map(|id| {
                let writer = writer.clone();
                thread::spawn(move || {
                    let frame = FrameCodec::default()
                        .encode(&RawPacket::new(i32::from(id), vec![id; 32768]))
                        .unwrap();
                    for _ in 0..16 {
                        writer.frame(&frame).unwrap();
                    }
                })
            })
            .collect();
        let mut counts = [0; 2];
        for _ in 0..32 {
            let mut size = 0;
            for shift in (0..21).step_by(7) {
                let mut byte = [0];
                server.read_exact(&mut byte).unwrap();
                size |= usize::from(byte[0] & 0x7f) << shift;
                if byte[0] & 0x80 == 0 {
                    break;
                }
            }
            assert_eq!(size, 32769);
            let mut body = vec![0; size];
            server.read_exact(&mut body).unwrap();
            let packet = FrameCodec::default().decode_body(&body).unwrap();
            assert!((1..=2).contains(&packet.id));
            assert!(packet.data.iter().all(|b| i32::from(*b) == packet.id));
            counts[(packet.id - 1) as usize] += 1;
        }
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(counts, [16, 16]);
    }
}
