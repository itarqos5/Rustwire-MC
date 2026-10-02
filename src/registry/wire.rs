//! Packet-level resource budgets shared across modern registry entries.
use crate::{
    codec::{identifier, Reader, Writer},
    nbt::{Nbt, RootFormat, Tag},
    Error, Limits, Result, Version,
};

pub(super) fn read_identifier(r: &mut Reader<'_>, version: Version) -> Result<String> {
    let value = r.string(32767)?;
    identifier::validate(value, version)?;
    Ok(value.into())
}
pub(super) fn write_identifier(
    w: &mut Writer,
    value: &str,
    version: Version,
    limits: Limits,
) -> Result<()> {
    identifier::validate(value, version)?;
    let mut prefix = 1;
    let mut n = value.len();
    while n > 127 {
        n >>= 7;
        prefix += 1;
    }
    if value.len().saturating_add(prefix) > limits.max_packet.saturating_sub(w.as_slice().len()) {
        return Err(Error::Limit("registry packet bytes"));
    }
    w.string(value, limits.max_string_chars.min(32767))
}
pub(super) fn read_nbt(r: &mut Reader<'_>, nodes: &mut usize) -> Result<Nbt> {
    let original = r.limits.max_nbt_nodes;
    r.limits.max_nbt_nodes = *nodes;
    let result = Nbt::read(r, RootFormat::Anonymous);
    r.limits.max_nbt_nodes = original;
    let nbt = result?.ok_or(Error::Invalid("present registry entry has End root"))?;
    charge_nodes(&nbt.root, nodes, 0, r.limits)?;
    Ok(nbt)
}
pub(super) fn write_nbt(
    w: &mut Writer,
    nbt: &Nbt,
    nodes: &mut usize,
    limits: Limits,
) -> Result<()> {
    charge_nodes(&nbt.root, nodes, 0, limits)?;
    nbt.write(
        w,
        RootFormat::Anonymous,
        Limits {
            max_packet: limits.max_packet.saturating_sub(w.as_slice().len()),
            ..limits
        },
    )
}
fn charge(nodes: &mut usize, n: usize) -> Result<()> {
    *nodes = nodes
        .checked_sub(n)
        .ok_or(Error::Limit("registry aggregate NBT nodes"))?;
    Ok(())
}
fn charge_nodes(tag: &Tag, nodes: &mut usize, depth: usize, limits: Limits) -> Result<()> {
    if depth > limits.max_nbt_depth.min(512) {
        return Err(Error::Limit("NBT depth"));
    }
    charge(nodes, 1)?;
    match tag {
        Tag::ByteArray(v) => charge(nodes, v.len())?,
        Tag::IntArray(v) => charge(nodes, v.len())?,
        Tag::LongArray(v) => charge(nodes, v.len())?,
        Tag::List { elements, .. } => {
            for tag in elements {
                charge_nodes(tag, nodes, depth + 1, limits)?;
            }
        }
        Tag::Compound(v) => {
            for (_, tag) in v {
                charge_nodes(tag, nodes, depth + 1, limits)?;
            }
        }
        _ => {}
    }
    Ok(())
}
