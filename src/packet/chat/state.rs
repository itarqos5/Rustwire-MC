//! Bounded client-side chat acknowledgement and packed-signature state.
//!
//! Signatures are opaque here: inserting, resolving or acknowledging one does
//! **not** authenticate its author or verify its cryptography. An application
//! must make that trust decision and decide whether a message was actually shown.
//! Each connection owns independent state; reset both structures on reconnect.
use super::{LastSeenUpdate, PlayerChat, PreviousMessage};
use crate::{Error, Result, Version};

/// Minecraft's fixed 256-byte message signature. No validity is implied.
pub type MessageSignature = [u8; 256];
pub const LAST_SEEN_CAPACITY: usize = 20;
pub const SIGNATURE_CACHE_CAPACITY: usize = 128;

#[derive(Clone, Debug)]
struct Tracked {
    signature: Box<MessageSignature>,
    pending: bool,
}

/// The signed-message history included in a generated acknowledgement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Acknowledgement {
    pub update: LastSeenUpdate,
    /// Oldest to newest acknowledged signatures, at most twenty.
    pub last_seen: Vec<MessageSignature>,
}

/// Fixed twenty-entry acknowledgement window, independent of the signature cache.
#[derive(Clone, Debug)]
pub struct LastSeenTracker {
    entries: [Option<Tracked>; LAST_SEEN_CAPACITY],
    cursor: usize,
    offset: u32,
    last_received: Option<Box<MessageSignature>>,
}
impl Default for LastSeenTracker {
    fn default() -> Self {
        Self {
            entries: std::array::from_fn(|_| None),
            cursor: 0,
            offset: 0,
            last_received: None,
        }
    }
}
impl LastSeenTracker {
    /// Track a signed message after the application has made its display/trust
    /// decision. `acknowledge` is true only for a message the application accepts
    /// as seen. An ignored message still advances the receive offset/window.
    /// Consecutive duplicate signatures do not advance anything.
    ///
    /// Unsigned messages do not belong in this tracker. Failures leave state
    /// unchanged; periodically send an acknowledgement to avoid offset overflow.
    pub fn track(&mut self, signature: &MessageSignature, acknowledge: bool) -> Result<bool> {
        if self.last_received.as_deref() == Some(signature) {
            return Ok(false);
        }
        let offset = self
            .offset
            .checked_add(1)
            .filter(|n| *n <= i32::MAX as u32)
            .ok_or(Error::Limit("chat acknowledgement offset"))?;
        self.entries[self.cursor] = acknowledge.then(|| Tracked {
            signature: Box::new(*signature),
            pending: true,
        });
        self.last_received = Some(Box::new(*signature));
        self.cursor = (self.cursor + 1) % LAST_SEEN_CAPACITY;
        self.offset = offset;
        Ok(true)
    }
    /// Forget a pending message, for example after a server deletion. Already
    /// acknowledged entries cannot be retroactively removed by this operation.
    pub fn ignore_pending(&mut self, signature: &MessageSignature) -> bool {
        for entry in &mut self.entries {
            if entry
                .as_ref()
                .is_some_and(|e| e.pending && e.signature.as_ref() == signature)
            {
                *entry = None;
                return true;
            }
        }
        false
    }
    pub fn pending_offset(&self) -> u32 {
        self.offset
    }
    /// Generate the last-seen payload and mark pending entries acknowledged.
    /// This consumes the receive offset. Call only for an outbound message that
    /// will actually be sent; clone the tracker first if a preview is needed.
    /// A failed/partially written connection must not reuse its chat state.
    pub fn generate_update(&mut self, version: Version) -> Acknowledgement {
        let mut acknowledged = [0; 3];
        let mut last_seen = Vec::with_capacity(LAST_SEEN_CAPACITY);
        for position in 0..LAST_SEEN_CAPACITY {
            if let Some(entry) = &mut self.entries[(self.cursor + position) % LAST_SEEN_CAPACITY] {
                acknowledged[position / 8] |= 1 << (position % 8);
                last_seen.push(*entry.signature);
                entry.pending = false;
            }
        }
        let checksum = (version.protocol() >= 770).then(|| checksum(&last_seen));
        Acknowledgement {
            update: LastSeenUpdate {
                offset: std::mem::take(&mut self.offset),
                acknowledged,
                checksum,
            },
            last_seen,
        }
    }
    /// Consume only the receive offset for a standalone acknowledgement packet.
    /// This does not acknowledge pending display entries or clear their history.
    pub fn take_offset(&mut self) -> u32 {
        std::mem::take(&mut self.offset)
    }
}

/// Protocol-770+ last-seen checksum. This is Java's ordered signed-byte hash,
/// reduced to a nonzero byte; it is a synchronization check, not authentication.
fn checksum(signatures: &[MessageSignature]) -> u8 {
    let mut sum = 1u32;
    for signature in signatures {
        let mut value = 1u32;
        for byte in signature.iter() {
            value = value
                .wrapping_mul(31)
                .wrapping_add(*byte as i8 as i32 as u32);
        }
        sum = sum.wrapping_mul(31).wrapping_add(value);
    }
    match sum as u8 {
        0 => 1,
        value => value,
    }
}

/// The fixed 128-entry cache used by packed previous-message signatures.
/// Cache updates and last-seen/display acknowledgements are separate operations.
#[derive(Clone, Debug)]
pub struct SignatureCache {
    entries: Vec<MessageSignature>,
}
impl Default for SignatureCache {
    fn default() -> Self {
        Self {
            entries: Vec::with_capacity(SIGNATURE_CACHE_CAPACITY),
        }
    }
}
impl SignatureCache {
    /// Lookup an unshifted wire-cache index, rejecting absent/out-of-range slots.
    pub fn get(&self, index: u32) -> Result<&MessageSignature> {
        self.entries
            .get(index as usize)
            .ok_or(Error::Invalid("unresolved chat signature cache index"))
    }
    /// Return the first cache index for a signature, or None to transmit it fully.
    pub fn index_of(&self, signature: &MessageSignature) -> Option<u32> {
        self.entries
            .iter()
            .position(|entry| entry.as_ref() == signature)
            .map(|i| i as u32)
    }
    /// Resolve all references without changing the cache. References are bounded
    /// by the protocol's twenty-entry previous-message list.
    pub fn resolve(&self, previous: &[PreviousMessage]) -> Result<Vec<MessageSignature>> {
        if previous.len() > LAST_SEEN_CAPACITY {
            return Err(Error::Limit("previous chat signatures"));
        }
        previous
            .iter()
            .map(|entry| match entry {
                PreviousMessage::Signature(signature) => Ok(**signature),
                PreviousMessage::Cached(index) => self.get(*index).copied(),
            })
            .collect()
    }
    /// Resolve a received player's history against the old cache, then apply its
    /// cache update. Invalid references leave the cache untouched. Neither this
    /// method nor its result authenticates the packet or marks it as displayed.
    pub fn ingest(&mut self, message: &PlayerChat) -> Result<Vec<MessageSignature>> {
        let previous = self.resolve(&message.previous_messages)?;
        self.push(&previous, message.signature.as_deref())?;
        Ok(previous)
    }
    /// Apply an already resolved message. The new signature, then previous
    /// signatures newest-first, precede old entries not present in this message.
    /// Incoming repeated signatures retain their protocol-defined positions.
    pub fn push(
        &mut self,
        previous: &[MessageSignature],
        signature: Option<&MessageSignature>,
    ) -> Result<()> {
        if previous.len() > LAST_SEEN_CAPACITY {
            return Err(Error::Limit("previous chat signatures"));
        }
        let mut new = Vec::with_capacity(SIGNATURE_CACHE_CAPACITY);
        if let Some(signature) = signature {
            new.push(*signature);
        }
        new.extend(previous.iter().rev().cloned());
        let incoming = new.len();
        if incoming == 0 {
            return Ok(());
        }
        for old in &self.entries {
            if !new[..incoming].contains(old) {
                new.push(*old);
                if new.len() == SIGNATURE_CACHE_CAPACITY {
                    break;
                }
            }
        }
        self.entries = new;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receive_offset_overflow_does_not_mutate_state() {
        let mut tracker = LastSeenTracker {
            offset: i32::MAX as u32,
            ..Default::default()
        };
        assert!(tracker.track(&[7; 256], true).is_err());
        assert_eq!(tracker.cursor, 0);
        assert!(tracker.last_received.is_none());
        assert!(tracker.entries.iter().all(Option::is_none));
        assert_eq!(tracker.take_offset(), i32::MAX as u32);
        assert!(tracker.track(&[7; 256], true).unwrap());
    }
}
