use super::*;

#[derive(Clone, Debug, PartialEq)]
pub enum ScoreAction {
    Set {
        /// Signed VarInt, including negative scores.
        value: i32,
        /// Requires protocol 765 or newer.
        display_name: Option<ChatComponent>,
        /// Requires protocol 765 or newer.
        number_format: Option<NumberFormat>,
    },
    /// Protocols 763–764 only; newer releases use [`ResetScore`].
    Remove,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreboardScore {
    pub owner: String,
    /// For legacy removal, an empty objective removes all this owner's scores.
    pub objective: String,
    pub action: ScoreAction,
}
impl Body for ScoreboardScore {
    fn read(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self> {
        let owner = r.string(32767)?.to_owned();
        let remove = if version.protocol() < 765 {
            match r.var_i32()? {
                0 => false,
                1 => true,
                _ => return Err(Error::Invalid("legacy score action")),
            }
        } else {
            false
        };
        let objective = r.string(32767)?.to_owned();
        let action = if remove {
            ScoreAction::Remove
        } else {
            let value = r.var_i32()?;
            let display_name = if version.protocol() >= 765 && r.bool()? {
                Some(read_component(r, version, b)?)
            } else {
                None
            };
            let number_format = if version.protocol() >= 765 {
                NumberFormat::read_optional(r, version, b)?
            } else {
                None
            };
            ScoreAction::Set {
                value,
                display_name,
                number_format,
            }
        };
        Ok(Self {
            owner,
            objective,
            action,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        string(w, &self.owner, 32767, b)?;
        if version.protocol() < 765 {
            w.var_i32(i32::from(matches!(self.action, ScoreAction::Remove)));
        }
        string(w, &self.objective, 32767, b)?;
        match &self.action {
            ScoreAction::Remove if version.protocol() >= 765 => Err(Error::Unsupported(
                "legacy score removal after protocol 764",
            )),
            ScoreAction::Remove => Ok(()),
            ScoreAction::Set {
                value,
                display_name,
                number_format,
            } => {
                if version.protocol() < 765 && (display_name.is_some() || number_format.is_some()) {
                    return Err(Error::Unsupported(
                        "score display or number format before protocol 765",
                    ));
                }
                w.var_i32(*value);
                if version.protocol() >= 765 {
                    w.bool(display_name.is_some());
                    if let Some(component) = display_name {
                        write_component(component, w, version, b)?;
                    }
                    NumberFormat::write_optional(number_format, w, version, b)?;
                }
                Ok(())
            }
        }
    }
}
/// Dedicated score removal, introduced in protocol 765.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResetScore {
    pub owner: String,
    /// `None` removes this owner's scores from all objectives. `Some("")` is
    /// retained distinctly as an explicit empty objective name.
    pub objective: Option<String>,
}
impl Body for ResetScore {
    fn read(r: &mut Reader<'_>, version: Version, _: &mut Budget) -> Result<Self> {
        if version.protocol() < 765 {
            return Err(Error::Unsupported("reset score before protocol 765"));
        }
        Ok(Self {
            owner: r.string(32767)?.to_owned(),
            objective: if r.bool()? {
                Some(r.string(32767)?.to_owned())
            } else {
                None
            },
        })
    }
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        if version.protocol() < 765 {
            return Err(Error::Unsupported("reset score before protocol 765"));
        }
        string(w, &self.owner, 32767, b)?;
        w.bool(self.objective.is_some());
        if let Some(name) = &self.objective {
            string(w, name, 32767, b)?;
        }
        Ok(())
    }
}
