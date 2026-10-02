//! Optional Microsoft device flow → Xbox Live → XSTS → Minecraft Services.
//!
//! Supply your own properly registered/authorized public-client application ID.
//! No borrowed launcher IDs, passwords, token persistence, or token logging.
//! Account-backed interoperability has not been exercised by this repository's tests.
use crate::{Error, Result};
use serde_json::{json, Value};
use std::{
    io::Read,
    time::{Duration, Instant},
};
use zeroize::Zeroize;
const OAUTH: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const SCOPE: &str = "XboxLive.signin offline_access";
const MAX_HTTP_BODY: u64 = 1_048_576;
/// A token that zeroizes its owned storage and deliberately redacts Debug output.
pub struct Secret(String);
impl Secret {
    pub fn new(value: String) -> Self {
        Self(value)
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}
#[derive(Debug)]
pub struct MicrosoftToken {
    pub access_token: Secret,
    pub refresh_token: Option<Secret>,
    pub expires_in: Duration,
}
pub struct DeviceCode {
    device_code: Secret,
    pub user_code: String,
    pub verification_uri: String,
    pub message: String,
    pub expires_in: Duration,
    pub interval: Duration,
    created: Instant,
}
impl std::fmt::Debug for DeviceCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceCode")
            .field("verification_uri", &self.verification_uri)
            .field("expires_in", &self.expires_in)
            .finish_non_exhaustive()
    }
}
#[derive(Debug)]
pub enum DevicePoll {
    Pending,
    SlowDown,
    Complete(MicrosoftToken),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinecraftProfile {
    pub uuid: [u8; 16],
    pub name: String,
}
#[derive(Debug)]
pub struct MinecraftSession {
    pub access_token: Secret,
    pub profile: MinecraftProfile,
    pub expires_in: Duration,
}
/// Blocking HTTP agent with TLS, bounded response bodies, deadlines, and no redirects.
pub struct AuthClient {
    client_id: String,
    http: ureq::Agent,
}
impl std::fmt::Debug for AuthClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthClient")
            .field("client_id", &self.client_id)
            .finish_non_exhaustive()
    }
}
impl AuthClient {
    pub fn new(client_id: impl Into<String>) -> Result<Self> {
        let client_id = client_id.into();
        if client_id.is_empty() || client_id.len() > 128 {
            return Err(Error::Invalid("Microsoft application client ID"));
        }
        Ok(Self {
            client_id,
            http: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(30))
                .redirects(0)
                .build(),
        })
    }
    pub fn begin_device_code(&self) -> Result<DeviceCode> {
        let v = self.form(
            &format!("{OAUTH}/devicecode"),
            &[("client_id", &self.client_id), ("scope", SCOPE)],
        )?;
        let interval = seconds(&v, "interval")?.max(Duration::from_secs(1));
        let expires_in = seconds(&v, "expires_in")?;
        if expires_in > Duration::from_secs(3600) || interval > Duration::from_secs(300) {
            return Err(Error::Auth("invalid device flow timing"));
        }
        Ok(DeviceCode {
            device_code: Secret::new(field(&v, "device_code")?.into()),
            user_code: field(&v, "user_code")?.into(),
            verification_uri: field(&v, "verification_uri")?.into(),
            message: field(&v, "message")?.into(),
            expires_in,
            interval,
            created: Instant::now(),
        })
    }
    /// Call no more frequently than code.interval; use wait_for_device for automatic timing.
    pub fn poll_device(&self, code: &DeviceCode) -> Result<DevicePoll> {
        if code.created.elapsed() >= code.expires_in {
            return Err(Error::Auth("device code expired"));
        }
        let response = self.http.post(&format!("{OAUTH}/token")).send_form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", &self.client_id),
            ("device_code", code.device_code.expose()),
        ]);
        let (status, v) = response_json(response)?;
        if status == 200 {
            return Ok(DevicePoll::Complete(parse_token(v)?));
        }
        match v.get("error").and_then(Value::as_str) {
            Some("authorization_pending") => Ok(DevicePoll::Pending),
            Some("slow_down") => Ok(DevicePoll::SlowDown),
            Some("authorization_declined" | "access_denied") => {
                Err(Error::Auth("device authorization denied"))
            }
            Some("expired_token") => Err(Error::Auth("device code expired")),
            _ => Err(Error::Auth("device token request rejected")),
        }
    }
    /// Blocks until completion/cancellation/expiry, honoring interval and slow_down.
    pub fn wait_for_device(
        &self,
        code: &DeviceCode,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<MicrosoftToken> {
        let mut interval = code.interval;
        loop {
            if cancelled() {
                return Err(Error::Auth("device authorization cancelled"));
            }
            let remaining = code.expires_in.saturating_sub(code.created.elapsed());
            if remaining.is_zero() {
                return Err(Error::Auth("device code expired"));
            }
            std::thread::sleep(interval.min(remaining));
            if cancelled() {
                return Err(Error::Auth("device authorization cancelled"));
            }
            match self.poll_device(code)? {
                DevicePoll::Complete(t) => return Ok(t),
                DevicePoll::Pending => {}
                DevicePoll::SlowDown => interval = interval.saturating_add(Duration::from_secs(5)),
            }
        }
    }
    pub fn refresh(&self, refresh_token: &Secret) -> Result<MicrosoftToken> {
        parse_token(self.form(
            &format!("{OAUTH}/token"),
            &[
                ("grant_type", "refresh_token"),
                ("client_id", &self.client_id),
                ("refresh_token", refresh_token.expose()),
                ("scope", SCOPE),
            ],
        )?)
    }
    pub fn minecraft_session(&self, microsoft_access_token: &Secret) -> Result<MinecraftSession> {
        let xbl=self.post("https://user.auth.xboxlive.com/user/authenticate",json!({"Properties":{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":format!("d={}",microsoft_access_token.expose())},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"}))?;
        let xsts=self.post("https://xsts.auth.xboxlive.com/xsts/authorize",json!({"Properties":{"SandboxId":"RETAIL","UserTokens":[field(&xbl,"Token")?]},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"}))?;
        let uhs = xsts
            .pointer("/DisplayClaims/xui/0/uhs")
            .and_then(Value::as_str)
            .ok_or(Error::Auth("missing Xbox user hash"))?;
        if xbl
            .pointer("/DisplayClaims/xui/0/uhs")
            .and_then(Value::as_str)
            != Some(uhs)
        {
            return Err(Error::Auth("Xbox identity mismatch"));
        }
        let minecraft = self.post(
            "https://api.minecraftservices.com/authentication/login_with_xbox",
            json!({"identityToken":format!("XBL3.0 x={uhs};{}",field(&xsts,"Token")?)}),
        )?;
        let access_token = Secret::new(field(&minecraft, "access_token")?.into());
        let expires_in = seconds(&minecraft, "expires_in")?;
        let profile = self.profile(&access_token)?;
        Ok(MinecraftSession {
            access_token,
            profile,
            expires_in,
        })
    }
    /// Also accepts a Minecraft access token acquired by an application-owned auth layer.
    pub fn profile(&self, access_token: &Secret) -> Result<MinecraftProfile> {
        let v = success_json(
            self.http
                .get("https://api.minecraftservices.com/minecraft/profile")
                .set(
                    "Authorization",
                    &format!("Bearer {}", access_token.expose()),
                )
                .call(),
        )?;
        let name = field(&v, "name")?.to_owned();
        if name.is_empty() || name.len() > 16 {
            return Err(Error::Auth("invalid Minecraft profile name"));
        }
        Ok(MinecraftProfile {
            uuid: parse_uuid(field(&v, "id")?)?,
            name,
        })
    }
    /// Authorize joining this exact server hash before sending the encryption response.
    pub fn join_server(&self, session: &MinecraftSession, server_hash: &str) -> Result<()> {
        if server_hash == "-"
            || server_hash.starts_with("--")
            || server_hash.len() > 41
            || server_hash.is_empty()
            || !server_hash
                .strip_prefix('-')
                .unwrap_or(server_hash)
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err(Error::Invalid("server hash"));
        }
        let response=self.http.post("https://sessionserver.mojang.com/session/minecraft/join").send_json(json!({"accessToken":session.access_token.expose(),"selectedProfile":uuid_hex(&session.profile.uuid),"serverId":server_hash}));
        match response {
            Ok(r) if r.status() == 204 => Ok(()),
            _ => Err(Error::Auth("Minecraft session join rejected")),
        }
    }
    fn form(&self, url: &str, fields: &[(&str, &str)]) -> Result<Value> {
        success_json(self.http.post(url).send_form(fields))
    }
    fn post(&self, url: &str, body: Value) -> Result<Value> {
        success_json(self.http.post(url).send_json(body))
    }
}
fn field<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(Error::Auth("missing authentication field"))
}
fn seconds(v: &Value, key: &str) -> Result<Duration> {
    let seconds = v
        .get(key)
        .and_then(Value::as_u64)
        .ok_or(Error::Auth("missing expiration"))?;
    if seconds == 0 || seconds > 31_536_000 {
        return Err(Error::Auth("invalid expiration"));
    }
    Ok(Duration::from_secs(seconds))
}
fn parse_token(v: Value) -> Result<MicrosoftToken> {
    Ok(MicrosoftToken {
        access_token: Secret::new(field(&v, "access_token")?.into()),
        refresh_token: v
            .get("refresh_token")
            .and_then(Value::as_str)
            .map(|s| Secret::new(s.into())),
        expires_in: seconds(&v, "expires_in")?,
    })
}
fn response_json(
    response: std::result::Result<ureq::Response, ureq::Error>,
) -> Result<(u16, Value)> {
    let response = match response {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(_) => return Err(Error::Auth("HTTP transport failed")),
    };
    let status = response.status();
    let mut body = Vec::new();
    response
        .into_reader()
        .take(MAX_HTTP_BODY + 1)
        .read_to_end(&mut body)
        .map_err(|_| Error::Auth("HTTP body read failed"))?;
    if body.len() as u64 > MAX_HTTP_BODY {
        body.zeroize();
        return Err(Error::Limit("authentication HTTP body"));
    }
    let value =
        serde_json::from_slice(&body).map_err(|_| Error::Auth("invalid authentication JSON"));
    body.zeroize();
    Ok((status, value?))
}
fn success_json(response: std::result::Result<ureq::Response, ureq::Error>) -> Result<Value> {
    let (status, v) = response_json(response)?;
    if !(200..300).contains(&status) {
        return Err(Error::Auth("service rejected request; check app registration, account eligibility, and token expiry"));
    }
    Ok(v)
}
pub fn parse_uuid(s: &str) -> Result<[u8; 16]> {
    let s = s.replace('-', "");
    if s.len() != 32 || !s.is_ascii() {
        return Err(Error::Invalid("UUID"));
    }
    let mut out = [0; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte =
            u8::from_str_radix(&s[2 * i..2 * i + 2], 16).map_err(|_| Error::Invalid("UUID hex"))?;
    }
    Ok(out)
}
pub fn uuid_hex(uuid: &[u8; 16]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(32);
    for b in uuid {
        write!(out, "{b:02x}").unwrap();
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn secrets_are_redacted() {
        assert!(!format!("{:?}", Secret::new("very-secret".into())).contains("very-secret"));
    }
    #[test]
    fn uuid_roundtrip() {
        let id = parse_uuid("00112233-4455-6677-8899-aabbccddeeff").unwrap();
        assert_eq!(uuid_hex(&id), "00112233445566778899aabbccddeeff");
        assert!(parse_uuid("invalid").is_err());
    }
    #[test]
    fn oauth_errors_do_not_expose_tokens() {
        assert!(parse_token(json!({"access_token":"secret","expires_in":0})).is_err());
    }
}
