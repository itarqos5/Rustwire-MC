use super::*;

/// How a score is rendered, independent of any number-format override.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectiveRenderType {
    Integer,
    Hearts,
}
impl ObjectiveRenderType {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        match r.var_i32()? {
            0 => Ok(Self::Integer),
            1 => Ok(Self::Hearts),
            _ => Err(Error::Invalid("objective render type")),
        }
    }
}
/// Optional score-number formatting introduced in protocol 765.
#[derive(Clone, Debug, PartialEq)]
pub enum NumberFormat {
    Blank,
    /// Compound-root anonymous NBT style, retained without interpreting its fields.
    Styled(Nbt),
    Fixed(ChatComponent),
}
impl NumberFormat {
    pub(super) fn read_optional(
        r: &mut Reader<'_>,
        version: Version,
        b: &mut Budget,
    ) -> Result<Option<Self>> {
        if !r.bool()? {
            return Ok(None);
        }
        Ok(Some(match r.var_i32()? {
            0 => Self::Blank,
            1 => {
                let nbt = read_nbt(r, b)?;
                validate_style(&nbt)?;
                Self::Styled(nbt)
            }
            2 => Self::Fixed(read_component(r, version, b)?),
            _ => return Err(Error::Invalid("score number format")),
        }))
    }
    pub(super) fn write_optional(
        value: &Option<Self>,
        w: &mut Writer,
        version: Version,
        b: &mut Budget,
    ) -> Result<()> {
        w.bool(value.is_some());
        if let Some(value) = value {
            match value {
                Self::Blank => w.var_i32(0),
                Self::Styled(nbt) => {
                    validate_style(nbt)?;
                    w.var_i32(1);
                    inventory::write_nbt(Some(nbt), w, RootFormat::Anonymous, b)?;
                }
                Self::Fixed(component) => {
                    w.var_i32(2);
                    write_component(component, w, version, b)?;
                }
            }
        }
        b.check_bytes(w)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectiveProperties {
    pub display_name: ChatComponent,
    pub render_type: ObjectiveRenderType,
    /// Must be absent before protocol 765.
    pub number_format: Option<NumberFormat>,
}
impl ObjectiveProperties {
    fn read(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self> {
        Ok(Self {
            display_name: read_component(r, version, b)?,
            render_type: ObjectiveRenderType::read(r)?,
            number_format: if version.protocol() >= 765 {
                NumberFormat::read_optional(r, version, b)?
            } else {
                None
            },
        })
    }
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        if version.protocol() < 765 && self.number_format.is_some() {
            return Err(Error::Unsupported(
                "objective number format before protocol 765",
            ));
        }
        write_component(&self.display_name, w, version, b)?;
        w.var_i32(self.render_type as i32);
        if version.protocol() >= 765 {
            NumberFormat::write_optional(&self.number_format, w, version, b)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectiveAction {
    Create(ObjectiveProperties),
    Remove,
    Update(ObjectiveProperties),
    /// Header-only action accepted by the official packet codec. Must be
    /// outside the defined action IDs 0–2.
    Other(i8),
}
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreboardObjective {
    pub name: String,
    pub action: ObjectiveAction,
}
impl Body for ScoreboardObjective {
    fn read(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self> {
        let name = r.string(32767)?.to_owned();
        let action = match r.u8()? {
            0 => ObjectiveAction::Create(ObjectiveProperties::read(r, version, b)?),
            1 => ObjectiveAction::Remove,
            2 => ObjectiveAction::Update(ObjectiveProperties::read(r, version, b)?),
            n => ObjectiveAction::Other(n as i8),
        };
        Ok(Self { name, action })
    }
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        string(w, &self.name, 32767, b)?;
        match &self.action {
            ObjectiveAction::Create(p) => {
                w.u8(0);
                p.write(w, version, b)?;
            }
            ObjectiveAction::Remove => w.u8(1),
            ObjectiveAction::Update(p) => {
                w.u8(2);
                p.write(w, version, b)?;
            }
            ObjectiveAction::Other(n) if (0..=2).contains(n) => {
                return Err(Error::Invalid("known objective action in Other variant"))
            }
            ObjectiveAction::Other(n) => w.u8(*n as u8),
        }
        Ok(())
    }
}

/// The numeric display slot. Protocol 763 retains a signed-byte wire value;
/// newer protocols use a VarInt. Canonical IDs are 0–18; other IDs are
/// retained although the official enum lookup falls back to the list slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayObjective {
    pub slot: i32,
    /// The empty string clears this display slot.
    pub objective: String,
}
impl Body for DisplayObjective {
    fn read(r: &mut Reader<'_>, version: Version, _: &mut Budget) -> Result<Self> {
        let slot = if version.protocol() == 763 {
            i32::from(r.u8()? as i8)
        } else {
            r.var_i32()?
        };
        validate_slot(slot, version)?;
        Ok(Self {
            slot,
            objective: r.string(32767)?.to_owned(),
        })
    }
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        validate_slot(self.slot, version)?;
        if version.protocol() == 763 {
            w.u8(self.slot as u8);
        } else {
            w.var_i32(self.slot);
        }
        string(w, &self.objective, 32767, b)
    }
}
fn validate_slot(slot: i32, version: Version) -> Result<()> {
    if (version.protocol() == 763 && i8::try_from(slot).is_ok()) || version.protocol() > 763 {
        Ok(())
    } else {
        Err(Error::Invalid("scoreboard display slot"))
    }
}

fn validate_style(nbt: &Nbt) -> Result<()> {
    if matches!(nbt.root, crate::nbt::Tag::Compound(_)) {
        Ok(())
    } else {
        Err(Error::Invalid("score number style must be compound NBT"))
    }
}
