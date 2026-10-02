//! Real loopback HTTP exchanges with entirely synthetic credentials.
//!
//! Microsoft wire contracts:
//! https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-device-code
//! https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow#refresh-the-access-token
//! https://learn.microsoft.com/en-us/gaming/gdk/docs/services/fundamentals/s2s-auth-calls/service-authentication/live-website-authentication
//! Device polling/backoff: https://www.rfc-editor.org/rfc/rfc8628#section-3.5
//! Minecraft fixtures exercise the library contract, not live account interoperability.
use super::*;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    thread::{self, JoinHandle},
};

const CLIENT_ID: &str = "00000000-0000-0000-0000-000000000000";
const DEVICE_PATH: &str = "/login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_PATH: &str = "/login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL_PATH: &str = "/user.auth.xboxlive.com/user/authenticate";
const XSTS_PATH: &str = "/xsts.auth.xboxlive.com/xsts/authorize";
const LOGIN_PATH: &str = "/api.minecraftservices.com/authentication/login_with_xbox";
const PROFILE_PATH: &str = "/api.minecraftservices.com/minecraft/profile";
const JOIN_PATH: &str = "/sessionserver.mojang.com/session/minecraft/join";
const DEVICE_SECRET: &str = "synthetic-device+/=& secret";
const MS_SECRET: &str = "synthetic-microsoft+/=& secret";
const REFRESH_SECRET: &str = "synthetic-refresh+/=& secret";
const XBL_SECRET: &str = "synthetic-xbox-token";
const XSTS_SECRET: &str = "synthetic-xsts-token";
const MC_SECRET: &str = "synthetic-minecraft-token";
const UUID: &str = "00112233445566778899aabbccddeeff";

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
    received: Instant,
}
impl Request {
    fn read(stream: &mut TcpStream) -> Self {
        let mut reader = BufReader::new(stream);
        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        let mut parts = first.split_whitespace();
        let method = parts.next().unwrap().to_owned();
        let path = parts.next().unwrap().to_owned();
        assert_eq!(parts.next(), Some("HTTP/1.1"));
        let mut headers = BTreeMap::new();
        loop {
            let mut line = String::new();
            assert_ne!(reader.read_line(&mut line).unwrap(), 0);
            if line == "\r\n" {
                break;
            }
            let (name, value) = line.split_once(':').unwrap();
            assert!(headers
                .insert(name.to_ascii_lowercase(), value.trim().to_owned())
                .is_none());
        }
        assert!(!headers.contains_key("transfer-encoding"));
        let size = headers
            .get("content-length")
            .map_or(0, |s| s.parse().unwrap());
        assert!(size < 64 * 1024, "unexpectedly large test request");
        let mut body = vec![0; size];
        reader.read_exact(&mut body).unwrap();
        Self {
            method,
            path,
            headers,
            body,
            received: Instant::now(),
        }
    }
    fn assert_target(&self, method: &str, path: &str) {
        assert_eq!(self.method, method);
        assert_eq!(self.path, path);
        assert!(!self.path.contains("synthetic-"), "credential in URL");
        assert!(!self.headers.contains_key("cookie"));
    }
    fn json(&self) -> Value {
        assert_eq!(
            self.headers.get("content-type").unwrap(),
            "application/json"
        );
        serde_json::from_slice(&self.body).unwrap()
    }
    fn form(&self) -> BTreeMap<String, String> {
        assert_eq!(
            self.headers.get("content-type").unwrap(),
            "application/x-www-form-urlencoded"
        );
        let mut fields = BTreeMap::new();
        for pair in std::str::from_utf8(&self.body).unwrap().split('&') {
            let (key, value) = pair.split_once('=').unwrap();
            assert!(fields
                .insert(decode_form(key), decode_form(value))
                .is_none());
        }
        fields
    }
}

fn decode_form(value: &str) -> String {
    let mut bytes = value.bytes();
    let mut out = Vec::new();
    while let Some(byte) = bytes.next() {
        out.push(match byte {
            b'+' => b' ',
            b'%' => {
                let hex = [bytes.next().unwrap(), bytes.next().unwrap()];
                u8::from_str_radix(std::str::from_utf8(&hex).unwrap(), 16).unwrap()
            }
            other => other,
        });
    }
    String::from_utf8(out).unwrap()
}
fn fields(items: &[(&str, &str)]) -> BTreeMap<String, String> {
    items
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

struct Response {
    status: u16,
    body: Vec<u8>,
    headers: Vec<(String, String)>,
    chunked: bool,
    claimed_length: Option<usize>,
    delay: Duration,
    disconnect: bool,
}
impl Response {
    fn raw(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            body: body.into(),
            headers: vec![],
            chunked: false,
            claimed_length: None,
            delay: Duration::ZERO,
            disconnect: false,
        }
    }
    fn json(status: u16, value: Value) -> Self {
        Self::raw(status, serde_json::to_vec(&value).unwrap())
    }
    fn write(self, stream: &mut TcpStream) -> std::io::Result<()> {
        if self.disconnect {
            return Ok(());
        }
        write!(
            stream,
            "HTTP/1.1 {} Test\r\nConnection: close\r\nContent-Type: application/json\r\n",
            self.status
        )?;
        for (name, value) in self.headers {
            write!(stream, "{name}: {value}\r\n")?;
        }
        if self.chunked {
            write!(stream, "Transfer-Encoding: chunked\r\n\r\n")?;
        } else {
            write!(
                stream,
                "Content-Length: {}\r\n\r\n",
                self.claimed_length.unwrap_or(self.body.len())
            )?;
        }
        stream.flush()?;
        thread::sleep(self.delay);
        if self.chunked {
            for chunk in self.body.chunks(8192) {
                write!(stream, "{:x}\r\n", chunk.len())?;
                stream.write_all(chunk)?;
                write!(stream, "\r\n")?;
            }
            write!(stream, "0\r\n\r\n")?;
        } else {
            stream.write_all(&self.body)?;
        }
        stream.flush()
    }
}

/// A bounded server that records *every* connection, even unexpected extra requests.
/// Drop stops and joins its worker so failing tests do not leave listeners behind.
struct Server {
    address: SocketAddr,
    requests: Receiver<Request>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Server {
    fn new(responses: Vec<Response>) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let (send, requests) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let worker = thread::spawn(move || {
            let mut responses = responses.into_iter();
            while !stopping.load(Ordering::Acquire) {
                let mut stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(e) => panic!("mock accept: {e}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let request = Request::read(&mut stream);
                if send.send(request).is_err() {
                    break;
                }
                let response = responses
                    .next()
                    .unwrap_or_else(|| Response::json(500, json!({"error":"unexpected_request"})));
                // Limit/timeout tests intentionally close their response reader early.
                if let Err(e) = response.write(&mut stream) {
                    assert!(
                        matches!(
                            e.kind(),
                            std::io::ErrorKind::BrokenPipe
                                | std::io::ErrorKind::ConnectionReset
                                | std::io::ErrorKind::ConnectionAborted
                        ),
                        "mock write: {e}"
                    );
                }
            }
        });
        Self {
            address,
            requests,
            stop,
            worker: Some(worker),
        }
    }
    fn client(&self) -> AuthClient {
        let mut client = AuthClient::new(CLIENT_ID).unwrap();
        client.test_server = Some(self.address);
        client
    }
    fn take(&self) -> Request {
        self.requests
            .recv_timeout(Duration::from_secs(3))
            .expect("missing HTTP request")
    }
    fn finish(mut self) {
        self.stop.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap();
        assert!(self.requests.try_recv().is_err(), "unexpected HTTP request");
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let result = worker.join();
            if !thread::panicking() {
                result.unwrap();
            }
        }
    }
}

fn device_response() -> Value {
    json!({"device_code": DEVICE_SECRET, "user_code":"SYNTH-USER", "verification_uri":"https://example.invalid/device", "message":"Use SYNTH-USER for the synthetic fixture", "expires_in":900, "interval":5})
}
fn device_code() -> DeviceCode {
    DeviceCode {
        device_code: Secret::new(DEVICE_SECRET.into()),
        user_code: "SYNTH-USER".into(),
        verification_uri: "https://example.invalid/device".into(),
        message: "synthetic only".into(),
        expires_in: Duration::from_secs(60),
        interval: Duration::from_millis(10),
        created: Instant::now(),
    }
}
fn token_response() -> Value {
    json!({"access_token":MS_SECRET, "refresh_token":REFRESH_SECRET, "expires_in":3600, "token_type":"Bearer"})
}
fn xbox_response(token: &str, uhs: &str) -> Value {
    json!({"Token":token, "DisplayClaims":{"xui":[{"uhs":uhs}]}})
}
fn profile_response() -> Value {
    json!({"id":UUID, "name":"TestPlayer"})
}
fn session() -> MinecraftSession {
    MinecraftSession {
        access_token: Secret::new(MC_SECRET.into()),
        profile: MinecraftProfile {
            uuid: parse_uuid(UUID).unwrap(),
            name: "TestPlayer".into(),
        },
        expires_in: Duration::from_secs(3600),
    }
}
fn assert_redacted(value: impl std::fmt::Debug) {
    let output = format!("{value:?}");
    for secret in [
        DEVICE_SECRET,
        MS_SECRET,
        REFRESH_SECRET,
        XBL_SECRET,
        XSTS_SECRET,
        MC_SECRET,
        "SYNTH-USER",
    ] {
        assert!(!output.contains(secret), "credential leaked in Debug");
    }
}
fn assert_error(error: Error, expected: &str) {
    assert!(error.to_string().contains(expected), "{error}");
    assert_redacted(&error);
    assert_redacted(error.to_string());
}

#[test]
fn device_code_posts_form_and_parses_response_without_debug_credentials() {
    let server = Server::new(vec![Response::json(200, device_response())]);
    let client = server.client();
    let code = client.begin_device_code().unwrap();
    assert_eq!(code.device_code.expose(), DEVICE_SECRET);
    assert_eq!(code.user_code, "SYNTH-USER");
    assert_eq!(code.verification_uri, "https://example.invalid/device");
    assert_eq!(code.expires_in, Duration::from_secs(900));
    assert_eq!(code.interval, Duration::from_secs(5));
    assert_redacted(&code);
    assert_redacted(&client);
    let request = server.take();
    request.assert_target("POST", DEVICE_PATH);
    assert_eq!(
        request.form(),
        fields(&[
            ("client_id", CLIENT_ID),
            ("scope", "XboxLive.signin offline_access")
        ])
    );
    assert!(!request.headers.contains_key("authorization"));
    server.finish();
}

#[test]
fn device_code_rejects_missing_fields_and_out_of_bounds_timing() {
    let mutations = [
        ("device_code", Value::Null),
        ("user_code", json!("")),
        ("verification_uri", json!(false)),
        ("message", Value::Null),
        ("expires_in", json!(0)),
        ("expires_in", json!(3601)),
        ("expires_in", json!(-1)),
        ("interval", json!(0)),
        ("interval", json!(301)),
        ("interval", json!("5")),
    ];
    for (field, value) in mutations {
        let mut body = device_response();
        body[field] = value;
        let server = Server::new(vec![Response::json(200, body)]);
        assert_redacted(server.client().begin_device_code().unwrap_err());
        server.take().assert_target("POST", DEVICE_PATH);
        server.finish();
    }
}

#[test]
fn device_poll_handles_pending_slow_down_and_completion_with_encoded_form() {
    let server = Server::new(vec![
        Response::json(400, json!({"error":"authorization_pending"})),
        Response::json(400, json!({"error":"slow_down"})),
        Response::json(200, token_response()),
    ]);
    let client = server.client();
    let code = device_code();
    assert!(matches!(
        client.poll_device(&code).unwrap(),
        DevicePoll::Pending
    ));
    assert!(matches!(
        client.poll_device(&code).unwrap(),
        DevicePoll::SlowDown
    ));
    let poll = client.poll_device(&code).unwrap();
    assert_redacted(&poll);
    let DevicePoll::Complete(token) = poll else {
        panic!("expected token")
    };
    assert_eq!(token.access_token.expose(), MS_SECRET);
    assert_eq!(
        token.refresh_token.as_ref().unwrap().expose(),
        REFRESH_SECRET
    );
    assert_eq!(token.expires_in, Duration::from_secs(3600));
    for _ in 0..3 {
        let request = server.take();
        request.assert_target("POST", TOKEN_PATH);
        assert_eq!(
            request.form(),
            fields(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", CLIENT_ID),
                ("device_code", DEVICE_SECRET)
            ])
        );
        assert!(!request.headers.contains_key("authorization"));
    }
    server.finish();
}

#[test]
fn device_poll_denial_expiry_and_unknown_errors_stop_with_redacted_errors() {
    for (kind, expected) in [
        ("authorization_declined", "device authorization denied"),
        ("access_denied", "device authorization denied"),
        ("expired_token", "device code expired"),
        ("bad_verification_code", "device token request rejected"),
        ("invalid_client", "device token request rejected"),
    ] {
        let server = Server::new(vec![Response::json(
            400,
            json!({"error":kind,"error_description":MS_SECRET,"device_code":DEVICE_SECRET}),
        )]);
        assert_error(
            server.client().poll_device(&device_code()).unwrap_err(),
            expected,
        );
        server.take().assert_target("POST", TOKEN_PATH);
        server.finish();
    }
}

#[test]
fn expired_device_code_never_sends_an_http_request() {
    let server = Server::new(vec![]);
    let mut code = device_code();
    code.created = Instant::now() - Duration::from_secs(61);
    assert_error(
        server.client().poll_device(&code).unwrap_err(),
        "device code expired",
    );
    server.finish();
}

#[test]
fn wait_for_device_obeys_interval_and_persistent_slow_down() {
    let server = Server::new(vec![
        Response::json(400, json!({"error":"slow_down"})),
        Response::json(400, json!({"error":"authorization_pending"})),
        Response::json(200, token_response()),
    ]);
    let code = device_code();
    let started = Instant::now();
    let token = server.client().wait_for_device(&code, || false).unwrap();
    assert_eq!(token.access_token.expose(), MS_SECRET);
    let first = server.take();
    let second = server.take();
    let third = server.take();
    assert!(first.received.duration_since(started) >= code.interval);
    let slowed = code.interval + Duration::from_secs(5);
    assert!(second.received.duration_since(first.received) >= slowed);
    assert!(third.received.duration_since(second.received) >= slowed);
    for request in [first, second, third] {
        request.assert_target("POST", TOKEN_PATH);
    }
    server.finish();
}

#[test]
fn wait_cancellation_before_and_after_sleep_and_expiry_send_no_request() {
    for cancel_on_call in [1, 2] {
        let server = Server::new(vec![]);
        let mut calls = 0;
        let error = server
            .client()
            .wait_for_device(&device_code(), || {
                calls += 1;
                calls == cancel_on_call
            })
            .unwrap_err();
        assert_eq!(calls, cancel_on_call);
        assert_error(error, "device authorization cancelled");
        server.finish();
    }
    let server = Server::new(vec![]);
    let mut code = device_code();
    code.interval = Duration::from_secs(30);
    code.expires_in = Duration::from_millis(5);
    assert_error(
        server
            .client()
            .wait_for_device(&code, || false)
            .unwrap_err(),
        "device code expired",
    );
    server.finish();
}

#[test]
fn refresh_posts_encoded_secret_and_returns_rotated_token_or_absence() {
    for include_refresh in [true, false] {
        let mut body = token_response();
        if !include_refresh {
            body.as_object_mut().unwrap().remove("refresh_token");
        }
        let server = Server::new(vec![Response::json(200, body)]);
        let input = Secret::new("old-synthetic+/=& refresh".into());
        let token = server.client().refresh(&input).unwrap();
        assert_eq!(token.access_token.expose(), MS_SECRET);
        assert_eq!(
            token.refresh_token.as_ref().map(Secret::expose),
            include_refresh.then_some(REFRESH_SECRET)
        );
        assert_eq!(input.expose(), "old-synthetic+/=& refresh");
        assert_redacted(token);
        let request = server.take();
        request.assert_target("POST", TOKEN_PATH);
        assert_eq!(
            request.form(),
            fields(&[
                ("grant_type", "refresh_token"),
                ("client_id", CLIENT_ID),
                ("refresh_token", input.expose()),
                ("scope", "XboxLive.signin offline_access")
            ])
        );
        assert!(!request.headers.contains_key("authorization"));
        server.finish();
    }
}

#[test]
fn refresh_rejects_malformed_access_refresh_and_expiration_fields() {
    for (key, value) in [
        ("access_token", json!("")),
        ("access_token", Value::Null),
        ("refresh_token", json!("")),
        ("refresh_token", json!(42)),
        ("refresh_token", Value::Null),
        ("expires_in", json!(0)),
        ("expires_in", json!(31_536_001)),
        ("expires_in", json!(-1)),
        ("expires_in", json!(1.5)),
        ("expires_in", json!("3600")),
    ] {
        let mut body = token_response();
        body[key] = value;
        let server = Server::new(vec![Response::json(200, body)]);
        assert_redacted(
            server
                .client()
                .refresh(&Secret::new(REFRESH_SECRET.into()))
                .unwrap_err(),
        );
        server.take().assert_target("POST", TOKEN_PATH);
        server.finish();
    }
}

#[test]
fn microsoft_to_minecraft_chain_and_join_use_exact_requests() {
    let server = Server::new(vec![
        Response::json(200, xbox_response(XBL_SECRET, "123456")),
        Response::json(200, xbox_response(XSTS_SECRET, "123456")),
        Response::json(200, json!({"access_token":MC_SECRET,"expires_in":86400})),
        Response::json(200, profile_response()),
        Response::raw(204, vec![]),
    ]);
    let client = server.client();
    let session = client
        .minecraft_session(&Secret::new(MS_SECRET.into()))
        .unwrap();
    assert_eq!(session.access_token.expose(), MC_SECRET);
    assert_eq!(session.profile.uuid, parse_uuid(UUID).unwrap());
    assert_eq!(session.profile.name, "TestPlayer");
    assert_eq!(session.expires_in, Duration::from_secs(86400));
    assert_redacted(&session);
    client.join_server(&session, "-123abc").unwrap();
    let xbl = server.take();
    xbl.assert_target("POST", XBL_PATH);
    assert_eq!(
        xbl.headers
            .get("x-xbl-contract-version")
            .map(String::as_str),
        Some("1")
    );
    assert_eq!(
        xbl.json(),
        json!({"Properties":{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":format!("d={MS_SECRET}")},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"})
    );
    assert!(!xbl.headers.contains_key("authorization"));
    let xsts = server.take();
    xsts.assert_target("POST", XSTS_PATH);
    assert_eq!(
        xsts.headers
            .get("x-xbl-contract-version")
            .map(String::as_str),
        Some("1")
    );
    assert_eq!(
        xsts.json(),
        json!({"Properties":{"SandboxId":"RETAIL","UserTokens":[XBL_SECRET]},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"})
    );
    assert!(!xsts.headers.contains_key("authorization"));
    let login = server.take();
    login.assert_target("POST", LOGIN_PATH);
    assert_eq!(
        login.json(),
        json!({"identityToken":format!("XBL3.0 x=123456;{XSTS_SECRET}")})
    );
    assert!(!login.headers.contains_key("authorization"));
    assert!(!login.headers.contains_key("x-xbl-contract-version"));
    let profile = server.take();
    profile.assert_target("GET", PROFILE_PATH);
    assert!(profile.body.is_empty());
    assert_eq!(
        profile.headers.get("authorization"),
        Some(&format!("Bearer {MC_SECRET}"))
    );
    let join = server.take();
    join.assert_target("POST", JOIN_PATH);
    assert_eq!(
        join.json(),
        json!({"accessToken":MC_SECRET,"selectedProfile":UUID,"serverId":"-123abc"})
    );
    assert!(!join.headers.contains_key("authorization"));
    server.finish();
}

#[test]
fn xbox_user_hash_mismatch_missing_and_empty_stop_before_minecraft() {
    for (xbl_hash, xsts_hash) in [
        (json!("123"), json!("456")),
        (Value::Null, json!("123")),
        (json!("123"), Value::Null),
        (json!(""), json!("")),
        (json!(123), json!(123)),
    ] {
        let xbl = json!({"Token":XBL_SECRET,"DisplayClaims":{"xui":[{"uhs":xbl_hash}]}});
        let xsts = json!({"Token":XSTS_SECRET,"DisplayClaims":{"xui":[{"uhs":xsts_hash}]}});
        let server = Server::new(vec![Response::json(200, xbl), Response::json(200, xsts)]);
        assert_redacted(
            server
                .client()
                .minecraft_session(&Secret::new(MS_SECRET.into()))
                .unwrap_err(),
        );
        server.take().assert_target("POST", XBL_PATH);
        server.take().assert_target("POST", XSTS_PATH);
        server.finish();
    }
}

#[test]
fn authentication_chain_rejections_do_not_continue_or_leak_response_details() {
    for failing_stage in 0..4 {
        let mut responses = vec![
            Response::json(200, xbox_response(XBL_SECRET, "123")),
            Response::json(200, xbox_response(XSTS_SECRET, "123")),
            Response::json(200, json!({"access_token":MC_SECRET,"expires_in":3600})),
            Response::json(200, profile_response()),
        ];
        responses.truncate(failing_stage);
        responses.push(Response::json(401,json!({"error":"rejected","error_description":MS_SECRET,"Token":XBL_SECRET,"XErr":2148916233_u64,"Message":XSTS_SECRET})));
        let server = Server::new(responses);
        assert_error(
            server
                .client()
                .minecraft_session(&Secret::new(MS_SECRET.into()))
                .unwrap_err(),
            "service rejected request",
        );
        for path in [XBL_PATH, XSTS_PATH, LOGIN_PATH, PROFILE_PATH]
            .into_iter()
            .take(failing_stage + 1)
        {
            server
                .take()
                .assert_target(if path == PROFILE_PATH { "GET" } else { "POST" }, path);
        }
        server.finish();
    }
}

#[test]
fn profile_uuid_and_name_validation_is_strict() {
    for valid in [
        UUID,
        "00112233-4455-6677-8899-aabbccddeeff",
        "00112233-4455-6677-8899-AABBCCDDEEFF",
    ] {
        let server = Server::new(vec![Response::json(
            200,
            json!({"id":valid,"name":"TestPlayer"}),
        )]);
        let profile = server
            .client()
            .profile(&Secret::new(MC_SECRET.into()))
            .unwrap();
        assert_eq!(uuid_hex(&profile.uuid), UUID);
        server.take().assert_target("GET", PROFILE_PATH);
        server.finish();
    }
    for invalid in [
        "-00112233445566778899aabbccddeeff",
        "00112233445566778899aabbccddeeff-",
        "0011223344556677----8899aabbccddeeff",
        "0011223-34455-6677-8899a-abbccddeeff",
        "00112233445566778899aabbccddeefg",
        "00112233445566778899aabbccddeef",
        "00112233445566778899aabbccddeeé",
    ] {
        let server = Server::new(vec![Response::json(
            200,
            json!({"id":invalid,"name":"TestPlayer"}),
        )]);
        assert_error(
            server
                .client()
                .profile(&Secret::new(MC_SECRET.into()))
                .unwrap_err(),
            "UUID",
        );
        server.take().assert_target("GET", PROFILE_PATH);
        server.finish();
    }
    for name in ["", "seventeen-chars!!x"] {
        let server = Server::new(vec![Response::json(200, json!({"id":UUID,"name":name}))]);
        assert_redacted(
            server
                .client()
                .profile(&Secret::new(MC_SECRET.into()))
                .unwrap_err(),
        );
        server.take().assert_target("GET", PROFILE_PATH);
        server.finish();
    }
}

#[test]
fn session_join_accepts_only_204_and_never_returns_error_body() {
    for status in [200, 201, 202, 204, 302, 400, 401, 403, 429, 500] {
        let server = Server::new(vec![Response::json(status, json!({"error":MC_SECRET}))]);
        let result = server.client().join_server(&session(), "0");
        if status == 204 {
            result.unwrap();
        } else {
            assert_error(result.unwrap_err(), "Minecraft session join rejected");
        }
        server.take().assert_target("POST", JOIN_PATH);
        server.finish();
    }
}

#[test]
fn invalid_server_hash_is_rejected_without_network_io() {
    let server = Server::new(vec![]);
    for hash in [
        "",
        "-",
        "--1",
        "hello",
        "1/2",
        " 12",
        "12\n",
        "+1",
        "12345678901234567890123456789012345678901",
        "123456789012345678901234567890123456789012",
    ] {
        assert_error(
            server.client().join_server(&session(), hash).unwrap_err(),
            "server hash",
        );
    }
    server.finish();
}

#[test]
fn malformed_empty_and_non_json_http_bodies_are_redacted_errors() {
    for (status, body) in [
        (200, b"".to_vec()),
        (
            200,
            format!("{{\"access_token\":\"{MS_SECRET}\"").into_bytes(),
        ),
        (400, format!("<html>{REFRESH_SECRET}</html>").into_bytes()),
        (500, [DEVICE_SECRET.as_bytes(), &[0xff]].concat()),
        (200, b"{} {}".to_vec()),
    ] {
        let server = Server::new(vec![Response::raw(status, body)]);
        assert_error(
            server
                .client()
                .refresh(&Secret::new(REFRESH_SECRET.into()))
                .unwrap_err(),
            "invalid authentication JSON",
        );
        server.take().assert_target("POST", TOKEN_PATH);
        server.finish();
    }
}

#[test]
fn body_limit_is_enforced_for_success_errors_and_chunked_transfer() {
    for chunked in [false, true] {
        for status in [200, 400] {
            let mut response = Response::raw(status, vec![b' '; MAX_HTTP_BODY as usize + 1]);
            response.chunked = chunked;
            let server = Server::new(vec![response]);
            assert_error(
                server
                    .client()
                    .refresh(&Secret::new(REFRESH_SECRET.into()))
                    .unwrap_err(),
                "authentication HTTP body",
            );
            server.take().assert_target("POST", TOKEN_PATH);
            server.finish();
        }
        let mut body = serde_json::to_vec(&token_response()).unwrap();
        body.resize(MAX_HTTP_BODY as usize, b' ');
        let mut response = Response::raw(200, body);
        response.chunked = chunked;
        let server = Server::new(vec![response]);
        assert_eq!(
            server
                .client()
                .refresh(&Secret::new(REFRESH_SECRET.into()))
                .unwrap()
                .access_token
                .expose(),
            MS_SECRET
        );
        server.take().assert_target("POST", TOKEN_PATH);
        server.finish();
    }
}

#[test]
fn truncated_http_body_is_a_redacted_read_error() {
    let mut response = Response::json(200, token_response());
    response.claimed_length = Some(response.body.len() + 10);
    let server = Server::new(vec![response]);
    assert_error(
        server
            .client()
            .refresh(&Secret::new(REFRESH_SECRET.into()))
            .unwrap_err(),
        "HTTP body read failed",
    );
    server.take().assert_target("POST", TOKEN_PATH);
    server.finish();
}

#[test]
fn credential_bearing_redirects_are_not_followed() {
    for status in [301, 302, 303, 307, 308] {
        let target = Server::new(vec![]);
        let mut response = Response::json(status, json!({"error":MS_SECRET}));
        response.headers.push((
            "Location".into(),
            format!("http://{}/redirect-target", target.address),
        ));
        let server = Server::new(vec![response]);
        assert_error(
            server
                .client()
                .refresh(&Secret::new(REFRESH_SECRET.into()))
                .unwrap_err(),
            "service rejected request",
        );
        server.take().assert_target("POST", TOKEN_PATH);
        server.finish();
        target.finish();
    }
}

#[test]
fn response_deadline_is_enforced_without_exposing_credentials() {
    let mut response = Response::json(200, token_response());
    response.delay = Duration::from_millis(350);
    let server = Server::new(vec![response]);
    let mut client = server.client();
    // Only this test shortens the unchanged production 30-second deadline.
    client.http = ureq::AgentBuilder::new()
        .timeout(Duration::from_millis(100))
        .redirects(0)
        .build();
    let error = client
        .refresh(&Secret::new(REFRESH_SECRET.into()))
        .unwrap_err();
    // A heavily scheduled host may time out before headers arrive instead.
    assert!(matches!(
        error,
        Error::Auth("HTTP body read failed" | "HTTP transport failed")
    ));
    assert_redacted(error);
    server.take().assert_target("POST", TOKEN_PATH);
    server.finish();
}

#[test]
fn signed_server_hash_accepts_up_to_40_hex_digits() {
    for hash in ["f".repeat(40), format!("-{}", "a".repeat(40))] {
        let server = Server::new(vec![Response::raw(204, vec![])]);
        server.client().join_server(&session(), &hash).unwrap();
        let request = server.take();
        request.assert_target("POST", JOIN_PATH);
        assert_eq!(request.json()["serverId"], hash);
        server.finish();
    }
}

#[test]
fn malformed_authentication_chain_stops_at_the_affected_stage() {
    let cases = [
        (
            0,
            json!({"Token":"", "DisplayClaims":{"xui":[{"uhs":"123"}]}}),
        ),
        (
            0,
            json!({"Token":42, "DisplayClaims":{"xui":[{"uhs":"123"}]}}),
        ),
        (
            1,
            json!({"Token":"", "DisplayClaims":{"xui":[{"uhs":"123"}]}}),
        ),
        (1, json!({"Token":XSTS_SECRET, "DisplayClaims":{"xui":[]}})),
        (2, json!({"access_token":"", "expires_in":3600})),
        (2, json!({"access_token":MC_SECRET, "expires_in":0})),
        (3, json!({"id":123, "name":"TestPlayer"})),
        (3, json!({"id":UUID, "name":null})),
    ];
    for (stage, body) in cases {
        let mut responses = vec![
            Response::json(200, xbox_response(XBL_SECRET, "123")),
            Response::json(200, xbox_response(XSTS_SECRET, "123")),
            Response::json(200, json!({"access_token":MC_SECRET,"expires_in":3600})),
            Response::json(200, profile_response()),
        ];
        responses.truncate(stage);
        responses.push(Response::json(200, body));
        let server = Server::new(responses);
        assert_redacted(
            server
                .client()
                .minecraft_session(&Secret::new(MS_SECRET.into()))
                .unwrap_err(),
        );
        for path in [XBL_PATH, XSTS_PATH, LOGIN_PATH, PROFILE_PATH]
            .into_iter()
            .take(stage + 1)
        {
            server
                .take()
                .assert_target(if path == PROFILE_PATH { "GET" } else { "POST" }, path);
        }
        server.finish();
    }
}

#[test]
fn transport_failure_is_redacted_for_tokens_profile_and_join() {
    let server = Server::new(
        (0..3)
            .map(|_| {
                let mut response = Response::raw(200, vec![]);
                response.disconnect = true;
                response
            })
            .collect(),
    );
    let client = server.client();
    assert_error(
        client
            .refresh(&Secret::new(REFRESH_SECRET.into()))
            .unwrap_err(),
        "HTTP transport failed",
    );
    assert_error(
        client.profile(&Secret::new(MC_SECRET.into())).unwrap_err(),
        "HTTP transport failed",
    );
    assert_error(
        client.join_server(&session(), "abc").unwrap_err(),
        "Minecraft session join rejected",
    );
    for (method, path) in [
        ("POST", TOKEN_PATH),
        ("GET", PROFILE_PATH),
        ("POST", JOIN_PATH),
    ] {
        server.take().assert_target(method, path);
    }
    server.finish();
}
