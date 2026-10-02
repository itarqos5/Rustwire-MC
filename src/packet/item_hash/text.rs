//! Verified literal text subset of ComponentSerialization's persistent codec.
//! Unknown content/style fields fail closed, rather than hashing their raw NBT.
use super::*;

struct Text<'a> {
    literal: &'a NbtString,
    style: Vec<(&'static str, u32)>,
    extra: Vec<Text<'a>>,
}
impl Text<'_> {
    fn hash(&self) -> u32 {
        let literal = string_units(self.literal.0.iter().copied());
        if self.style.is_empty() && self.extra.is_empty() {
            return literal;
        }
        let mut out = self.style.clone();
        out.push(("text", literal));
        if !self.extra.is_empty() {
            out.push(("extra", list(self.extra.iter().map(Self::hash))));
        }
        fields(out)
    }
}
fn color(value: &NbtString) -> Result<u32> {
    let value = value
        .to_string()
        .map_err(|_| Error::Invalid("text color UTF-16"))?;
    if let Some(hex) = value.strip_prefix('#') {
        if hex.len() == 6 && hex.bytes().all(|v| v.is_ascii_hexdigit()) {
            return Ok(string(&format!("#{}", hex.to_ascii_uppercase())));
        }
    } else if matches!(
        value.as_str(),
        "black"
            | "dark_blue"
            | "dark_green"
            | "dark_aqua"
            | "dark_red"
            | "dark_purple"
            | "gold"
            | "gray"
            | "dark_gray"
            | "blue"
            | "green"
            | "aqua"
            | "red"
            | "light_purple"
            | "yellow"
            | "white"
    ) {
        return Ok(string(&value));
    }
    Err(Error::Invalid("text color"))
}
fn parse(tag: &Tag) -> Result<Text<'_>> {
    match tag {
        Tag::String(literal) => Ok(Text {
            literal,
            style: vec![],
            extra: vec![],
        }),
        Tag::List { elements, .. } => {
            let (first, rest) = elements
                .split_first()
                .ok_or(Error::Invalid("empty text list"))?;
            let mut out = parse(first)?;
            for child in rest {
                out.extra.push(parse(child)?);
            }
            Ok(out)
        }
        Tag::Compound(entries) => {
            let literal = tag
                .get("text")
                .and_then(Tag::as_str)
                .ok_or(Error::Unsupported("nonliteral text component hash"))?;
            let mut out = Text {
                literal,
                style: vec![],
                extra: vec![],
            };
            // Java's CompoundTag retains the last duplicate value.
            let normalized: BTreeMap<&NbtString, &Tag> =
                entries.iter().map(|(k, v)| (k, v)).collect();
            for (key, value) in normalized {
                let key = key
                    .to_string()
                    .map_err(|_| Error::Unsupported("text component field"))?;
                let boolean_key = match key.as_str() {
                    "bold" => Some("bold"),
                    "italic" => Some("italic"),
                    "underlined" => Some("underlined"),
                    "strikethrough" => Some("strikethrough"),
                    "obfuscated" => Some("obfuscated"),
                    _ => None,
                };
                if let Some(key) = boolean_key {
                    // Only the canonical network representation is accepted.
                    let Tag::Byte(value @ (0 | 1)) = value else {
                        return Err(Error::Unsupported("noncanonical text boolean"));
                    };
                    out.style.push((key, boolean(*value != 0)));
                    continue;
                }
                match (key.as_str(), value) {
                    ("text", _) => {}
                    ("color", Tag::String(value)) => out.style.push(("color", color(value)?)),
                    ("font", Tag::String(value)) => {
                        let value = value
                            .to_string()
                            .map_err(|_| Error::Invalid("text font UTF-16"))?;
                        out.style.push(("font", identifier(&value)?));
                    }
                    ("insertion", Tag::String(value)) => out
                        .style
                        .push(("insertion", string_units(value.0.iter().copied()))),
                    ("shadow_color", Tag::Int(value)) => {
                        out.style.push(("shadow_color", integer(*value)))
                    }
                    ("extra", Tag::List { elements, .. }) => {
                        if elements.is_empty() {
                            return Err(Error::Invalid("empty text extra"));
                        }
                        for child in elements {
                            out.extra.push(parse(child)?);
                        }
                    }
                    _ => return Err(Error::Unsupported("text component content or style hash")),
                }
            }
            Ok(out)
        }
        _ => Err(Error::Unsupported("text component root hash")),
    }
}
pub(super) fn hash(tag: &Tag) -> Result<u32> {
    // NBT recursion, nodes, strings and byte budgets were checked by the caller.
    Ok(parse(tag)?.hash())
}
