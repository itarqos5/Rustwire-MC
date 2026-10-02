//! Run only when ready to sign in, using your own authorized Microsoft client ID.
use rustwire_mc::{
    auth::AuthClient,
    connection::{Connection, Event},
    crypto,
    packet::ClientSettings,
    Limits, Version,
};
use std::time::Duration;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3 {
        return Err("usage: online_login MICROSOFT_CLIENT_ID HOST [PORT] [VERSION]".into());
    }
    let auth = AuthClient::new(&args[1])?;
    let code = auth.begin_device_code()?;
    println!("{}", code.message);
    let microsoft = auth.wait_for_device(&code, || false)?;
    let session = auth.minecraft_session(&microsoft.access_token)?;
    let host = &args[2];
    let port = args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(25565);
    let version = args
        .get(4)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(Version::V1_21);
    let mut c = Connection::connect(
        (host.as_str(), port),
        version,
        Duration::from_secs(15),
        Limits::default(),
    )?;
    c.start_login(host, port, &session.profile.name, session.profile.uuid)?;
    for _ in 0..4096 {
        match c.next_event()? {
            Event::EncryptionRequested(request) => {
                let response =
                    crypto::encryption_response(&request.public_key, &request.verify_token)?;
                if request.should_authenticate {
                    auth.join_server(
                        &session,
                        &crypto::server_hash(
                            &request.server_id,
                            &response.shared_secret,
                            &request.public_key,
                        ),
                    )?;
                }
                c.complete_encryption(&response)?;
            }
            Event::LoginSuccess(profile) => {
                println!("Logged in as {}", profile.username);
                if version.has_configuration() { c.send_settings(&ClientSettings::default())?; }
                if !version.has_configuration() {
                    return Ok(());
                }
            }
            Event::KnownPacks(_) => c.select_known_packs(&[])?,
            Event::CookieRequest(key) => c.answer_cookie(&key, None)?,
            Event::LoginPluginRequest { id, .. } => c.answer_login_plugin(id, None)?,
            Event::Ready => {
                println!("Configuration finished; play protocol active");
                return Ok(());
            }
            Event::Disconnected(_) => {
                return Err(
                    "server disconnected (inspect raw disconnect component in your application)"
                        .into(),
                )
            }
            Event::Packet {name: Some("code_of_conduct" | "add_resource_pack" | "resource_pack_send"), ..} => return Err("server requires an application decision about conduct or a resource pack; handle this packet explicitly".into()),
            _ => {}
        }
    }
    Err("handshake exceeded 4096 packets".into())
}
