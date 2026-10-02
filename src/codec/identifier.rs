//! Shared resource-location syntax, without registry lookup or normalization.
use crate::{Error, Result, Version};

pub(crate) fn parts(value: &str) -> (&str, &str) {
    let (namespace, path) = value.split_once(':').unwrap_or(("minecraft", value));
    (
        if namespace.is_empty() {
            "minecraft"
        } else {
            namespace
        },
        path,
    )
}

pub(crate) fn validate(value: &str, version: Version) -> Result<()> {
    let (namespace, path) = parts(value);
    let valid = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b);
    // Empty paths are accepted by the actual resource-location parser. The
    // special namespace ".." became invalid in protocol 775.
    if (version.protocol() >= 775 && namespace == "..")
        || !namespace.bytes().all(valid)
        || !path.bytes().all(|b| valid(b) || b == b'/')
    {
        return Err(Error::Invalid("resource identifier"));
    }
    Ok(())
}
