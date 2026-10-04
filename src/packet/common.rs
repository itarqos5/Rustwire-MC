//! Shared configuration/play packets. Decoding never downloads URLs, follows
//! transfers, accepts conduct, or decides resource-pack consent.
use crate::{
    codec::{Reader, Writer},
    frame::RawPacket,
    packet::{chat::ChatComponent, named},
    version::State,
    Error, Limits, Result, Version,
};
fn reader(bytes: &[u8], limits: Limits) -> Result<Reader<'_>> {
    if bytes.len() > limits.max_packet {
        return Err(Error::Limit("common packet bytes"));
    }
    Ok(Reader::new(bytes, limits))
}
#[derive(Debug, Clone, PartialEq)]
pub struct ResourcePackOffer {
    pub uuid: Option<[u8; 16]>,
    pub url: String,
    pub hash: String,
    pub required: bool,
    pub prompt: Option<ChatComponent>,
}
impl ResourcePackOffer {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let uuid = if version.protocol() >= 765 {
            Some(r.uuid()?)
        } else {
            None
        };
        let url = r.string(32767)?.into();
        let hash = r.string(40)?.into();
        let required = r.bool()?;
        let prompt = if r.bool()? {
            Some(ChatComponent::read(&mut r, version)?)
        } else {
            None
        };
        r.finish()?;
        Ok(Self {
            uuid,
            url,
            hash,
            required,
            prompt,
        })
    }
    pub fn response(
        &self,
        version: Version,
        state: State,
        status: ResourcePackStatus,
    ) -> Result<RawPacket> {
        if !matches!(state, State::Configuration | State::Play) {
            return Err(Error::State("resource pack response"));
        }
        if version.protocol() < 765 && (status as i32) > 3 {
            return Err(Error::Unsupported(
                "extended resource pack status before 1.20.3",
            ));
        }
        let mut w = Writer::new();
        if version.protocol() >= 765 {
            w.raw(
                &self
                    .uuid
                    .ok_or(Error::Invalid("resource pack UUID required"))?,
            );
        } else if self.uuid.is_some() {
            return Err(Error::Invalid("resource pack UUID before 1.20.3"));
        }
        w.var_i32(status as i32);
        named(version, state, "resource_pack_receive", w.into_inner())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ResourcePackStatus {
    Loaded = 0,
    Declined = 1,
    DownloadFailed = 2,
    Accepted = 3,
    Downloaded = 4,
    InvalidUrl = 5,
    ReloadFailed = 6,
    Discarded = 7,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transfer {
    pub host: String,
    pub port: u16,
}
impl Transfer {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if version.protocol() < 766 {
            return Err(Error::Unsupported("server transfer before 1.20.5"));
        }
        let mut r = reader(bytes, limits)?;
        let host = r.string(32767)?.into();
        let port = r.var_i32()?;
        if !(1..=65535).contains(&port) {
            return Err(Error::Invalid("transfer port"));
        }
        r.finish()?;
        Ok(Self {
            host,
            port: port as u16,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCookie {
    pub key: String,
    pub value: Vec<u8>,
}
impl StoredCookie {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if version.protocol() < 766 {
            return Err(Error::Unsupported("cookies before 1.20.5"));
        }
        let mut r = reader(bytes, limits)?;
        let key = r.string(32767)?.into();
        let value = r.bytes(5120)?.to_vec();
        r.finish()?;
        Ok(Self { key, value })
    }
}
pub use super::server_metadata::{CustomReportDetails, ReportDetail, ResetChat};
// Preserve the original public paths while the complete codecs live separately.
pub use super::tags::{RegistryTag, TaggedRegistry, UpdateTags};
#[derive(Debug, Clone, PartialEq)]
pub enum ServerLinkLabel {
    BuiltIn(u32),
    Custom(ChatComponent),
}
#[derive(Debug, Clone, PartialEq)]
pub struct ServerLink {
    pub label: ServerLinkLabel,
    pub url: String,
}
#[derive(Debug, Clone, PartialEq)]
pub enum CommonPacket {
    ResetChat(ResetChat),
    CustomReportDetails(CustomReportDetails),
    ResourcePack(ResourcePackOffer),
    RemoveResourcePack(Option<[u8; 16]>),
    Transfer(Transfer),
    StoreCookie(StoredCookie),
    Tags(UpdateTags),
    FeatureFlags(Vec<String>),
    CodeOfConduct(String),
    ServerLinks(Vec<ServerLink>),
    ChunkBatchStarted,
    ChunkBatchFinished(u32),
    BundleDelimiter,
}
impl CommonPacket {
    pub fn decode(
        name: &str,
        bytes: &[u8],
        version: Version,
        limits: Limits,
    ) -> Result<Option<Self>> {
        let packet = match name {
            "reset_chat" => Self::ResetChat(ResetChat::decode(bytes, version, limits)?),
            "custom_report_details" => {
                Self::CustomReportDetails(CustomReportDetails::decode(bytes, version, limits)?)
            }
            "resource_pack_send" | "add_resource_pack" => {
                Self::ResourcePack(ResourcePackOffer::decode(bytes, version, limits)?)
            }
            "transfer" => Self::Transfer(Transfer::decode(bytes, version, limits)?),
            "store_cookie" => Self::StoreCookie(StoredCookie::decode(bytes, version, limits)?),
            "tags" => Self::Tags(UpdateTags::decode(bytes, version, limits)?),
            "remove_resource_pack" => {
                if version.protocol() < 765 {
                    return Err(Error::Unsupported("remove resource pack before 1.20.3"));
                }
                let mut r = reader(bytes, limits)?;
                let uuid = if r.bool()? { Some(r.uuid()?) } else { None };
                r.finish()?;
                Self::RemoveResourcePack(uuid)
            }
            "feature_flags" => {
                let mut r = reader(bytes, limits)?;
                let count = r.count(limits.max_collection)?;
                let mut values = Vec::with_capacity(count);
                for _ in 0..count {
                    values.push(r.string(32767)?.into());
                }
                r.finish()?;
                Self::FeatureFlags(values)
            }
            "code_of_conduct" => {
                if version.protocol() < 773 {
                    return Err(Error::Unsupported("code of conduct before 1.21.9"));
                }
                let mut r = reader(bytes, limits)?;
                let text = r.string(32767)?.into();
                r.finish()?;
                Self::CodeOfConduct(text)
            }
            "server_links" => {
                if version.protocol() < 767 {
                    return Err(Error::Unsupported("server links before 1.21"));
                }
                let mut r = reader(bytes, limits)?;
                let count = r.count(limits.max_collection)?;
                let mut links = Vec::with_capacity(count);
                for _ in 0..count {
                    let label = if r.bool()? {
                        let id = r.var_i32()?;
                        if id < 0 {
                            return Err(Error::Invalid("server link type"));
                        }
                        ServerLinkLabel::BuiltIn(id as u32)
                    } else {
                        ServerLinkLabel::Custom(ChatComponent::read(&mut r, version)?)
                    };
                    links.push(ServerLink {
                        label,
                        url: r.string(32767)?.into(),
                    });
                }
                r.finish()?;
                Self::ServerLinks(links)
            }
            "chunk_batch_start" | "bundle_delimiter" => {
                reader(bytes, limits)?.finish()?;
                if name == "chunk_batch_start" {
                    Self::ChunkBatchStarted
                } else {
                    Self::BundleDelimiter
                }
            }
            "chunk_batch_finished" => {
                let mut r = reader(bytes, limits)?;
                let count = r.var_i32()?;
                if count < 0 {
                    return Err(Error::Invalid("negative chunk batch size"));
                }
                r.finish()?;
                Self::ChunkBatchFinished(count as u32)
            }
            _ => return Ok(None),
        };
        Ok(Some(packet))
    }
}
/// Build only after the application/user explicitly accepts this server's conduct.
pub fn accept_code_of_conduct(version: Version) -> Result<RawPacket> {
    named(
        version,
        State::Configuration,
        "accept_code_of_conduct",
        vec![],
    )
}
pub fn chunk_batch_received(version: Version, desired_chunks_per_tick: f32) -> Result<RawPacket> {
    if !desired_chunks_per_tick.is_finite() || desired_chunks_per_tick <= 0. {
        return Err(Error::Invalid("chunk batch rate"));
    }
    let mut w = Writer::new();
    w.f32(desired_chunks_per_tick);
    named(version, State::Play, "chunk_batch_received", w.into_inner())
}
