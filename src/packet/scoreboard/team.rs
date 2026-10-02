use super::*;

macro_rules! team_rule {
    ($t:ident, $what:literal, $($variant:ident = $id:literal => $name:literal),+ $(,)?) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum $t {
            $($variant,)+
            /// Unknown string retained verbatim; only protocols 763–769.
            OtherName(String),
            /// Unknown numeric ID retained verbatim; protocols 770+ use the
            /// official codec's default-to-zero lookup, without rejecting it.
            OtherId(i32),
        }
        impl $t {
            fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
                if version.protocol() < 770 {
                    Ok(match r.string(40)? { $($name => Self::$variant,)+ s => Self::OtherName(s.to_owned()) })
                } else {
                    Ok(match r.var_i32()? { $($id => Self::$variant,)+ n => Self::OtherId(n) })
                }
            }
            fn write(&self, w: &mut Writer, version: Version, b: &Budget) -> Result<()> {
                if version.protocol() < 770 {
                    let value = match self {
                        $(Self::$variant => $name,)+
                        Self::OtherName(s) => {
                            if matches!(s.as_str(), $($name)|+) {
                                return Err(Error::Invalid(concat!("known ", $what, " in OtherName variant")));
                            }
                            s
                        },
                        Self::OtherId(_) => return Err(Error::Invalid(concat!($what, " representation before protocol 770"))),
                    };
                    string(w, value, 40, b)
                } else {
                    let id = match self {
                        $(Self::$variant => $id,)+
                        Self::OtherId(n) => {
                            if [$($id),+].contains(n) {
                                return Err(Error::Invalid(concat!("known ", $what, " in OtherId variant")));
                            }
                            *n
                        },
                        Self::OtherName(_) => return Err(Error::Invalid(concat!($what, " representation from protocol 770"))),
                    };
                    w.var_i32(id);
                    b.check_bytes(w)
                }
            }
        }
    }
}
team_rule! { TeamVisibility, "team visibility", Always = 0 => "always", Never = 1 => "never", HideForOtherTeams = 2 => "hideForOtherTeams", HideForOwnTeam = 3 => "hideForOwnTeam" }
team_rule! { TeamCollisionRule, "team collision", Always = 0 => "always", Never = 1 => "never", PushOtherTeams = 2 => "pushOtherTeams", PushOwnTeam = 3 => "pushOwnTeam" }

/// The two distinct release representations of team color.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamColor {
    /// Protocols 763–775 use the complete ChatFormatting ordinal (0–21):
    /// colors 0–15, obfuscated/bold/strikethrough/underline/italic 16–20,
    /// and reset 21. Values outside this enum domain are invalid.
    Formatting(i32),
    /// Protocol 776 uses an optional TeamColor VarInt. Canonical IDs are 0–15;
    /// other signed IDs are retained, although the official lookup uses black.
    Color(Option<i32>),
}
impl TeamColor {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        if version.protocol() >= 776 {
            Ok(Self::Color(if r.bool()? {
                Some(r.var_i32()?)
            } else {
                None
            }))
        } else {
            let n = r.var_i32()?;
            validate_formatting(n)?;
            Ok(Self::Formatting(n))
        }
    }
    fn write(&self, w: &mut Writer, version: Version) -> Result<()> {
        match (self, version.protocol() >= 776) {
            (Self::Formatting(n), false) => {
                validate_formatting(*n)?;
                w.var_i32(*n);
            }
            (Self::Color(value), true) => {
                w.bool(value.is_some());
                if let Some(n) = value {
                    w.var_i32(*n);
                }
            }
            _ => return Err(Error::Invalid("team color representation for protocol")),
        }
        Ok(())
    }
}
fn validate_formatting(n: i32) -> Result<()> {
    if (0..=21).contains(&n) {
        Ok(())
    } else {
        Err(Error::Invalid("team formatting ordinal"))
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct TeamParameters {
    pub display_name: ChatComponent,
    pub prefix: ChatComponent,
    pub suffix: ChatComponent,
    pub visibility: TeamVisibility,
    pub collision_rule: TeamCollisionRule,
    pub color: TeamColor,
    /// Full wire byte retained. Bits 0/1 mean friendly fire/see invisible allies.
    pub flags: u8,
}
impl TeamParameters {
    fn read(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self> {
        let display_name = read_component(r, version, b)?;
        if version.protocol() >= 776 {
            let prefix = read_component(r, version, b)?;
            let suffix = read_component(r, version, b)?;
            let visibility = TeamVisibility::read(r, version)?;
            let collision_rule = TeamCollisionRule::read(r, version)?;
            let color = TeamColor::read(r, version)?;
            let flags = r.u8()?;
            Ok(Self {
                display_name,
                prefix,
                suffix,
                visibility,
                collision_rule,
                color,
                flags,
            })
        } else {
            let flags = r.u8()?;
            let visibility = TeamVisibility::read(r, version)?;
            let collision_rule = TeamCollisionRule::read(r, version)?;
            let color = TeamColor::read(r, version)?;
            let prefix = read_component(r, version, b)?;
            let suffix = read_component(r, version, b)?;
            Ok(Self {
                display_name,
                prefix,
                suffix,
                visibility,
                collision_rule,
                color,
                flags,
            })
        }
    }
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        write_component(&self.display_name, w, version, b)?;
        if version.protocol() >= 776 {
            write_component(&self.prefix, w, version, b)?;
            write_component(&self.suffix, w, version, b)?;
            self.visibility.write(w, version, b)?;
            self.collision_rule.write(w, version, b)?;
            self.color.write(w, version)?;
            w.u8(self.flags);
        } else {
            w.u8(self.flags);
            self.visibility.write(w, version, b)?;
            self.collision_rule.write(w, version, b)?;
            self.color.write(w, version)?;
            write_component(&self.prefix, w, version, b)?;
            write_component(&self.suffix, w, version, b)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum TeamAction {
    Create {
        parameters: TeamParameters,
        members: Vec<String>,
    },
    Remove,
    Update(TeamParameters),
    AddMembers(Vec<String>),
    RemoveMembers(Vec<String>),
    /// Header-only raw mode accepted by the official packet codec. This must
    /// not be one of the defined modes 0–4.
    Other(i8),
}
#[derive(Clone, Debug, PartialEq)]
pub struct Teams {
    pub name: String,
    pub action: TeamAction,
}
fn read_members(r: &mut Reader<'_>, b: &mut Budget) -> Result<Vec<String>> {
    let count = b.count(r)?;
    let mut members = Vec::with_capacity(count.min(r.remaining().len()));
    for _ in 0..count {
        members.push(r.string(32767)?.to_owned());
    }
    Ok(members)
}
fn write_members(members: &[String], w: &mut Writer, b: &mut Budget) -> Result<()> {
    b.write_count(members.len(), w)?;
    for member in members {
        string(w, member, 32767, b)?;
    }
    Ok(())
}
impl Body for Teams {
    fn read(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self> {
        let name = r.string(32767)?.to_owned();
        let action = match r.u8()? as i8 {
            0 => TeamAction::Create {
                parameters: TeamParameters::read(r, version, b)?,
                members: read_members(r, b)?,
            },
            1 => TeamAction::Remove,
            2 => TeamAction::Update(TeamParameters::read(r, version, b)?),
            3 => TeamAction::AddMembers(read_members(r, b)?),
            4 => TeamAction::RemoveMembers(read_members(r, b)?),
            other => TeamAction::Other(other),
        };
        Ok(Self { name, action })
    }
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        string(w, &self.name, 32767, b)?;
        match &self.action {
            TeamAction::Create {
                parameters,
                members,
            } => {
                w.u8(0);
                parameters.write(w, version, b)?;
                write_members(members, w, b)?;
            }
            TeamAction::Remove => w.u8(1),
            TeamAction::Update(parameters) => {
                w.u8(2);
                parameters.write(w, version, b)?;
            }
            TeamAction::AddMembers(members) => {
                w.u8(3);
                write_members(members, w, b)?;
            }
            TeamAction::RemoveMembers(members) => {
                w.u8(4);
                write_members(members, w, b)?;
            }
            TeamAction::Other(n) if (0..=4).contains(n) => {
                return Err(Error::Invalid("known team mode in Other variant"))
            }
            TeamAction::Other(n) => w.u8(*n as u8),
        }
        Ok(())
    }
}
