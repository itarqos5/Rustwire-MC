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
        let reader = thread::spawn(move || {
            if let Err(error) = read_events(&mut connection, version, &tx) {
                let _ = tx.send(Err(error.to_string()));
            }
        });
        Ok(Self {
            events,
            writer,
            codec: FrameCodec::new(limits),
            shutdown,
            reader: Some(reader),
        })
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
) -> Result<()> {
    for _ in 0..100_000 {
        let event = connection.next_typed_event()?;
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
