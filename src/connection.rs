//! Synchronous, timeout-aware transport. Generic over Read + Write for embedding and tests.
use crate::{
    codec::{Reader, Writer},
    frame::{FrameCodec, RawPacket},
    packet::{self, ClientSettings, EncryptionRequest, LoginSuccess, PositionSync},
    version::{Direction, State},
    Error, Limits, Result, Version,
};
use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};
#[derive(Debug)]
pub struct Connection<S> {
    stream: S,
    version: Version,
    state: State,
    codec: FrameCodec,
    poisoned: bool,
    world_ready: bool,
    #[cfg(feature = "crypto")]
    encrypt: Option<crate::crypto::Cipher>,
    #[cfg(feature = "crypto")]
    decrypt: Option<crate::crypto::Cipher>,
}
impl Connection<TcpStream> {
    /// DNS lookup uses the system resolver and is not covered by connect_timeout.
    pub fn connect(
        address: impl ToSocketAddrs,
        version: Version,
        timeout: Duration,
        limits: Limits,
    ) -> Result<Self> {
        let mut last = None;
        for addr in address.to_socket_addrs()? {
            match TcpStream::connect_timeout(&addr, timeout) {
                Ok(s) => {
                    s.set_read_timeout(Some(timeout))?;
                    s.set_write_timeout(Some(timeout))?;
                    s.set_nodelay(true)?;
                    return Ok(Self::new(s, version, limits));
                }
                Err(e) => last = Some(e),
            }
        }
        Err(last
            .map(Error::Io)
            .unwrap_or(Error::Invalid("no socket addresses")))
    }
}
impl<S: Read + Write> Connection<S> {
    pub fn new(stream: S, version: Version, limits: Limits) -> Self {
        Self {
            stream,
            version,
            state: State::Handshake,
            codec: FrameCodec::new(limits),
            poisoned: false,
            world_ready: false,
            #[cfg(feature = "crypto")]
            encrypt: None,
            #[cfg(feature = "crypto")]
            decrypt: None,
        }
    }
    pub fn version(&self) -> Version {
        self.version
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn limits(&self) -> Limits {
        self.codec.limits
    }
    pub fn into_inner(self) -> S {
        self.stream
    }
    pub fn set_compression(&mut self, threshold: Option<usize>) -> Result<()> {
        self.codec.set_compression(threshold)
    }
    #[cfg(feature = "crypto")]
    pub fn enable_encryption(&mut self, secret: &[u8; 16]) -> Result<()> {
        if self.encrypt.is_some() {
            return Err(Error::State("encryption already enabled"));
        }
        self.encrypt = Some(crate::crypto::Cipher::new(secret));
        self.decrypt = Some(crate::crypto::Cipher::new(secret));
        Ok(())
    }
    /// Write one raw packet; failed I/O poisons the stream to prevent unsafe retries.
    pub fn send(&mut self, packet: &RawPacket) -> Result<()> {
        if self.poisoned {
            return Err(Error::State("connection unusable after I/O/framing error"));
        }
        #[allow(unused_mut)]
        let mut bytes = self.codec.encode(packet)?;
        #[cfg(feature = "crypto")]
        if let Some(cipher) = &mut self.encrypt {
            cipher.encrypt(&mut bytes);
        }
        if let Err(e) = self
            .stream
            .write_all(&bytes)
            .and_then(|_| self.stream.flush())
        {
            self.poisoned = true;
            return Err(e.into());
        }
        Ok(())
    }
    fn read_exact(&mut self, bytes: &mut [u8]) -> Result<()> {
        self.stream.read_exact(bytes)?;
        #[cfg(feature = "crypto")]
        if let Some(cipher) = &mut self.decrypt {
            cipher.decrypt(bytes);
        }
        Ok(())
    }
    pub fn receive(&mut self) -> Result<RawPacket> {
        if self.poisoned {
            return Err(Error::State("connection unusable after I/O/framing error"));
        }
        let result = self.receive_inner();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn receive_inner(&mut self) -> Result<RawPacket> {
        let mut n = 0usize;
        for i in 0..3 {
            let mut b = [0];
            self.read_exact(&mut b)?;
            n |= ((b[0] & 0x7f) as usize) << (7 * i);
            if b[0] & 0x80 == 0 {
                if n == 0 || n > self.codec.limits.max_frame {
                    return Err(Error::Limit("frame"));
                }
                let mut bytes = vec![0; n];
                self.read_exact(&mut bytes)?;
                return self.codec.decode_body(&bytes);
            }
        }
        Err(Error::Invalid("frame length exceeds three bytes"))
    }
    pub fn start_login(
        &mut self,
        host: &str,
        port: u16,
        username: &str,
        uuid: [u8; 16],
    ) -> Result<()> {
        if self.state != State::Handshake {
            return Err(Error::State("login already started"));
        }
        // Validate both packets before writing either one.
        let handshake = packet::handshake(self.version, host, port, State::Login)?;
        let start = packet::login_start(self.version, username, uuid)?;
        // Frame budgets can fail even when the individual fields are valid.
        // Preflight both encodings before any externally visible write.
        self.codec.encode(&handshake)?;
        self.codec.encode(&start)?;
        self.send(&handshake)?;
        self.state = State::Login;
        self.send(&start)
    }
    /// Status request followed by ping. Returns raw JSON and verified roundtrip.
    pub fn status(&mut self, host: &str, port: u16, nonce: i64) -> Result<StatusResponse> {
        if self.state != State::Handshake {
            return Err(Error::State("status already started"));
        }
        self.send(&packet::handshake(self.version, host, port, State::Status)?)?;
        self.state = State::Status;
        self.send(&RawPacket::new(0, []))?;
        let response = self.receive()?;
        if response.id != 0 {
            return Err(Error::Invalid("status response id"));
        }
        let mut r = Reader::new(&response.data, self.limits());
        let json = r.string(32767)?.into();
        r.finish()?;
        let mut w = Writer::new();
        w.i64(nonce);
        let started = std::time::Instant::now();
        self.send(&RawPacket::new(1, w.into_inner()))?;
        let pong = self.receive()?;
        let mut r = Reader::new(&pong.data, self.limits());
        if pong.id != 1 || r.i64()? != nonce {
            return Err(Error::Invalid("status pong"));
        }
        r.finish()?;
        Ok(StatusResponse {
            json,
            round_trip: started.elapsed(),
        })
    }
    /// Receives and handles common control traffic. Unknown packets remain available.
    /// No resource pack download, server transfer, or gameplay action is implicit.
    pub fn next_event(&mut self) -> Result<Event> {
        let packet = self.receive()?;
        let name = self
            .version
            .packet(self.state, Direction::Clientbound, packet.id)
            .map(|p| p.name);
        match (self.state, name) {
            (State::Login, Some("compress")) => {
                let mut r = Reader::new(&packet.data, self.limits());
                let n = r.var_i32()?;
                r.finish()?;
                let threshold = if n < 0 { None } else { Some(n as usize) };
                self.set_compression(threshold)?;
                Ok(Event::Compression(threshold))
            }
            (State::Login, Some("encryption_begin")) => Ok(Event::EncryptionRequested(
                EncryptionRequest::decode(&packet.data, self.version, self.limits())?,
            )),
            (State::Login, Some("success")) => {
                let profile = LoginSuccess::decode(&packet.data, self.version, self.limits())?;
                if self.version.has_configuration() {
                    self.send(&packet::named(
                        self.version,
                        State::Login,
                        "login_acknowledged",
                        vec![],
                    )?)?;
                    self.state = State::Configuration;
                } else {
                    self.state = State::Play;
                }
                Ok(Event::LoginSuccess(profile))
            }
            (State::Login, Some("login_plugin_request")) => {
                let mut r = Reader::new(&packet.data, self.limits());
                let id = r.var_i32()?;
                let channel = r.string(32767)?.to_owned();
                let data = r.remaining().to_vec();
                Ok(Event::LoginPluginRequest { id, channel, data })
            }
            (State::Configuration, Some("finish_configuration")) => {
                Reader::new(&packet.data, self.limits()).finish()?;
                self.send(&packet::named(
                    self.version,
                    self.state,
                    "finish_configuration",
                    vec![],
                )?)?;
                self.state = State::Play;
                Ok(Event::Ready)
            }
            (State::Play, Some("login")) => {
                let world = packet::JoinGame::decode(&packet.data, self.version, self.limits())?;
                self.world_ready = true;
                Ok(Event::Joined(Box::new(world)))
            }
            (State::Play, Some("start_configuration")) => {
                self.world_ready = false;
                Reader::new(&packet.data, self.limits()).finish()?;
                self.send(&packet::named(
                    self.version,
                    State::Play,
                    "configuration_acknowledged",
                    vec![],
                )?)?;
                self.state = State::Configuration;
                Ok(Event::Reconfigure)
            }
            (State::Configuration | State::Play, Some("keep_alive")) => {
                let mut r = Reader::new(&packet.data, self.limits());
                let id = r.i64()?;
                r.finish()?;
                self.send(&packet::named(
                    self.version,
                    self.state,
                    "keep_alive",
                    packet.data,
                )?)?;
                Ok(Event::KeepAlive(id))
            }
            (State::Configuration | State::Play, Some("ping")) => {
                let mut r = Reader::new(&packet.data, self.limits());
                let id = r.i32()?;
                r.finish()?;
                self.send(&packet::named(
                    self.version,
                    self.state,
                    "pong",
                    packet.data,
                )?)?;
                Ok(Event::Ping(id))
            }
            (State::Configuration, Some("select_known_packs")) => Ok(Event::KnownPacks(
                packet::known_packs(&packet.data, self.limits())?,
            )),
            (State::Configuration, Some("registry_data")) => Ok(Event::Registry(
                crate::registry::RegistryData::decode(&packet.data, self.version, self.limits())?,
            )),
            (State::Play, Some("position")) => Ok(Event::Position(PositionSync::decode(
                &packet.data,
                self.version,
                self.limits(),
            )?)),
            (_, Some("cookie_request")) => {
                let mut r = Reader::new(&packet.data, self.limits());
                let key = r.string(32767)?.into();
                r.finish()?;
                Ok(Event::CookieRequest(key))
            }
            (_, Some("disconnect" | "kick_disconnect")) => Ok(Event::Disconnected(packet.data)),
            _ => Ok(Event::Packet {
                state: self.state,
                name,
                packet,
            }),
        }
    }
    /// Receives control traffic as usual, with optional typed gameplay dispatch.
    /// Unknown packets and unsupported item/component layouts retain all raw bytes.
    /// Malformed known layouts still return an error and should end the connection.
    pub fn next_typed_event(&mut self) -> Result<TypedEvent> {
        match self.next_event()? {
            Event::Packet {
                state,
                name,
                packet,
            } => {
                let result = if let Some(name) = name {
                    packet::typed::DecodedPacket::decode(
                        state,
                        name,
                        &packet.data,
                        self.version,
                        self.limits(),
                    )
                } else {
                    Ok(None)
                };
                match result {
                    Ok(Some(decoded)) => Ok(TypedEvent::Decoded(decoded)),
                    Ok(None) => Ok(TypedEvent::Raw {
                        state,
                        name,
                        packet,
                        unsupported: None,
                    }),
                    Err(Error::Unsupported(reason)) => Ok(TypedEvent::Raw {
                        state,
                        name,
                        packet,
                        unsupported: Some(reason),
                    }),
                    Err(error) => Err(error),
                }
            }
            Event::Disconnected(bytes) => Ok(TypedEvent::Disconnected(
                packet::chat::Disconnect::decode(&bytes, self.version, self.state, self.limits())?,
            )),
            event => Ok(TypedEvent::Control(event)),
        }
    }
    pub fn send_settings(&mut self, settings: &ClientSettings) -> Result<()> {
        if self.state == State::Play && !self.world_ready {
            return Err(Error::State("wait for Join Game before play settings"));
        }
        if !matches!(self.state, State::Configuration | State::Play) {
            return Err(Error::State("settings require configuration/play"));
        }
        self.send(&settings.encode(self.version, self.state)?)
    }
    pub fn answer_login_plugin(&mut self, id: i32, response: Option<&[u8]>) -> Result<()> {
        if self.state != State::Login {
            return Err(Error::State("login plugin response"));
        }
        let mut w = Writer::new();
        w.var_i32(id);
        w.bool(response.is_some());
        if let Some(data) = response {
            if data.len() > self.limits().max_packet {
                return Err(Error::Limit("plugin response"));
            }
            w.raw(data);
        }
        self.send(&packet::named(
            self.version,
            self.state,
            "login_plugin_response",
            w.into_inner(),
        )?)
    }
    /// Report only packs actually available locally. Empty requests full registry data.
    pub fn select_known_packs(&mut self, packs: &[packet::KnownPack]) -> Result<()> {
        if self.state != State::Configuration {
            return Err(Error::State("known packs"));
        }
        if packs.len() > 1024 {
            return Err(Error::Limit("known packs"));
        }
        let mut w = Writer::new();
        w.var_i32(packs.len() as i32);
        for p in packs {
            w.string(&p.namespace, 32767)?;
            w.string(&p.id, 32767)?;
            w.string(&p.version, 32767)?;
        }
        self.send(&packet::named(
            self.version,
            self.state,
            "select_known_packs",
            w.into_inner(),
        )?)
    }
    pub fn answer_cookie(&mut self, key: &str, value: Option<&[u8]>) -> Result<()> {
        let mut w = Writer::new();
        w.string(key, 32767)?;
        w.bool(value.is_some());
        if let Some(value) = value {
            if value.len() > 5120 {
                return Err(Error::Limit("cookie"));
            }
            w.bytes(value)?;
        }
        self.send(&packet::named(
            self.version,
            self.state,
            "cookie_response",
            w.into_inner(),
        )?)
    }
    #[cfg(feature = "crypto")]
    /// Send the RSA response unencrypted, then enable continuous AES-CFB8 streams.
    /// The caller must join the session server first when should_authenticate is true.
    pub fn complete_encryption(
        &mut self,
        response: &crate::crypto::EncryptionResponse,
    ) -> Result<()> {
        if self.state != State::Login {
            return Err(Error::State("encryption requires login"));
        }
        if self.encrypt.is_some() {
            return Err(Error::State("already encrypted"));
        }
        let mut w = Writer::new();
        w.bytes(&response.encrypted_secret)?;
        w.bytes(&response.encrypted_verify_token)?;
        self.send(&packet::named(
            self.version,
            self.state,
            "encryption_begin",
            w.into_inner(),
        )?)?;
        self.enable_encryption(&response.shared_secret)
    }
}
#[derive(Debug)]
pub struct StatusResponse {
    pub json: String,
    pub round_trip: Duration,
}
#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    Compression(Option<usize>),
    EncryptionRequested(EncryptionRequest),
    LoginSuccess(LoginSuccess),
    Ready,
    Joined(Box<packet::JoinGame>),
    Reconfigure,
    KeepAlive(i64),
    Ping(i32),
    Registry(crate::registry::RegistryData),
    KnownPacks(Vec<packet::KnownPack>),
    Position(PositionSync),
    CookieRequest(String),
    LoginPluginRequest {
        id: i32,
        channel: String,
        data: Vec<u8>,
    },
    Disconnected(Vec<u8>),
    Packet {
        state: State,
        name: Option<&'static str>,
        packet: RawPacket,
    },
}

/// A typed view layered over raw/control events, without implicit gameplay policy.
#[derive(Debug)]
#[non_exhaustive]
pub enum TypedEvent {
    Control(Event),
    Decoded(packet::typed::DecodedPacket),
    Disconnected(packet::chat::Disconnect),
    Raw {
        state: State,
        name: Option<&'static str>,
        packet: RawPacket,
        unsupported: Option<&'static str>,
    },
}
