//! # soko-feed — publish a catalogue, and verify one without trusting the server
//!
//! This is the crate the whole pitch rests on. "Your catalogue is a signed feed you publish" and
//! "leaving costs a DNS change" are both claims about *this*, and until now nothing in the
//! workspace signed, published, fetched or verified anything.
//!
//! ## The property that makes exit real
//!
//! A feed is a signed head over a content-addressed set of objects. A fetcher recomputes every
//! address and checks the signature itself, so **the server is a convenience and never a trust
//! root**: it can withhold, stall, or serve nothing, and it cannot forge an object or substitute
//! one under a matching address without the seller's key.
//!
//! That is what makes moving between gateways a DNS change rather than a migration. The bytes are
//! identical wherever they are served from, and a buyer can prove it — which is the difference
//! between "you own your catalogue" as a promise and as a fact.
//!
//! ## What this does not do
//!
//! It publishes to a directory and fetches from one. There is no network transport here: serving
//! the directory over HTTPS, or over a mesh, or from a USB stick, is a binding and deliberately
//! out of scope — verification is identical whichever delivered the bytes, and that indifference
//! is the point.
//!
//! No cryptography is invented. Ed25519 (RFC 8032) and BLAKE3 are the substrate's primitives
//! (TRACT §16.2); this implements them rather than devising alternatives.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use soko_core::{ContentAddress, Timestamp};
use std::collections::BTreeMap;
use std::path::Path;

/// Domain-separation tag for a feed head's signing preimage.
///
/// Every signature commits to what it is a signature *of*. Without this, a head signature could be
/// replayed as some other object's signature under the same key — the class of bug that domain
/// separation exists to make unrepresentable rather than merely unlikely.
const HEAD_DS: &[u8] = b"TRACT-v0/feed-head\x00";

/// Prefix marking a BLAKE3-256 digest, per the substrate's multihash-style agility convention
/// (§16.2). It is carried so the digest can be migrated later without changing the address format.
const BLAKE3_PREFIX: u8 = 0x1e;

/// What went wrong verifying a feed. Every variant is a refusal — there is no partial trust.
#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    /// The head's signature does not verify under the claimed key.
    #[error("feed head signature is invalid")]
    BadSignature,
    /// An object's bytes do not hash to the address it was fetched by. The server substituted
    /// content, or something corrupted it; either way the bytes are not what was asked for.
    #[error("object does not match its content address")]
    AddressMismatch,
    /// The head references an object the fetcher could not obtain.
    #[error("feed references an object that is missing")]
    MissingObject,
    /// A head older than one already accepted for this author. Feeds only grow, so a lower
    /// sequence is a rollback — an attempt to un-publish by serving stale state.
    #[error("feed head is older than one already seen (rollback)")]
    Rollback,
    /// The stored bytes could not be decoded.
    #[error("malformed: {0}")]
    Malformed(&'static str),
    /// Storage failed.
    #[error("io: {0}")]
    Io(String),
}

/// Content-address some bytes.
///
/// The prefix is part of the address, not decoration: an address that did not say which digest
/// produced it could not be migrated to a different one without ambiguity.
pub fn address(bytes: &[u8]) -> ContentAddress {
    let mut out = Vec::with_capacity(33);
    out.push(BLAKE3_PREFIX);
    out.extend_from_slice(blake3::hash(bytes).as_bytes());
    ContentAddress(out)
}

/// The signed tip of a seller's feed.
///
/// Signing the head is enough to authenticate everything reachable from it, because the head
/// commits to each object by content address. That is why individual objects carry no signature of
/// their own — one signature, transitively covering the set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedHead {
    /// The publishing identity's verifying key.
    pub author: Vec<u8>,
    /// Monotonic. A head with a lower sequence than one already accepted is a rollback.
    pub seq: u64,
    /// Content addresses of every object in the feed at this sequence.
    pub objects: Vec<ContentAddress>,
    /// When this head was published.
    pub at: Timestamp,
    /// Signature over the domain-separated encoding of everything above.
    pub sig: Vec<u8>,
}

impl FeedHead {
    /// The bytes a signature commits to: the DS tag, then the head with `sig` excluded.
    ///
    /// Excluding the signature is not a stylistic choice — a field cannot contain a signature over
    /// itself, and including a placeholder would make the preimage depend on what is being signed.
    fn preimage(author: &[u8], seq: u64, objects: &[ContentAddress], at: Timestamp) -> Vec<u8> {
        let mut p = Vec::from(HEAD_DS);
        p.extend_from_slice(author);
        p.extend_from_slice(&seq.to_be_bytes());
        for o in objects {
            p.extend_from_slice(&o.0);
        }
        p.extend_from_slice(&at.0.to_be_bytes());
        p
    }
}

/// A seller's local feed: the objects they have published, and the key that signs for them.
pub struct Feed {
    key: SigningKey,
    objects: BTreeMap<Vec<u8>, Vec<u8>>, // address bytes -> object bytes
    seq: u64,
}

impl Feed {
    /// Start a feed under `key`.
    pub fn new(key: SigningKey) -> Self {
        Self {
            key,
            objects: BTreeMap::new(),
            seq: 0,
        }
    }

    /// The identity this feed publishes under. A name is a pointer to this, never the reverse.
    pub fn author(&self) -> VerifyingKey {
        self.key.verifying_key()
    }

    /// Add an object, returning the address it will be known by.
    ///
    /// Addressing happens here rather than at publish time so a caller can reference an object
    /// from another one before the head exists.
    pub fn add(&mut self, bytes: Vec<u8>) -> ContentAddress {
        let a = address(&bytes);
        self.objects.insert(a.0.clone(), bytes);
        a
    }

    /// Sign a head over everything currently in the feed.
    pub fn head(&mut self, at: Timestamp) -> FeedHead {
        self.seq += 1;
        let objects: Vec<ContentAddress> = self
            .objects
            .keys()
            .map(|k| ContentAddress(k.clone()))
            .collect();
        let author = self.key.verifying_key().to_bytes().to_vec();
        let pre = FeedHead::preimage(&author, self.seq, &objects, at);
        let sig: Signature = self.key.sign(&pre);
        FeedHead {
            author,
            seq: self.seq,
            objects,
            at,
            sig: sig.to_bytes().to_vec(),
        }
    }

    /// Write the feed to a directory as plain files — one per object, plus the signed head.
    ///
    /// Deliberately a directory. Anything that can serve a directory can serve a catalogue: a web
    /// server, a mesh node, a USB stick. The verifier does not care which, and that indifference is
    /// what makes moving between them free.
    pub fn publish(&mut self, dir: &Path, at: Timestamp) -> Result<FeedHead, VerifyError> {
        let objdir = dir.join("objects");
        std::fs::create_dir_all(&objdir).map_err(|e| VerifyError::Io(e.to_string()))?;
        for (addr, bytes) in &self.objects {
            std::fs::write(objdir.join(hex(addr)), bytes)
                .map_err(|e| VerifyError::Io(e.to_string()))?;
        }
        let head = self.head(at);
        let mut buf = Vec::new();
        ciborium::into_writer(&head, &mut buf)
            .map_err(|_| VerifyError::Malformed("head would not encode"))?;
        std::fs::write(dir.join("head"), &buf).map_err(|e| VerifyError::Io(e.to_string()))?;
        Ok(head)
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// A verified catalogue: objects whose addresses and signature a fetcher checked itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedFeed {
    /// The identity that signed it.
    pub author: Vec<u8>,
    /// The head's sequence.
    pub seq: u64,
    /// Objects, keyed by their verified content address.
    pub objects: BTreeMap<Vec<u8>, Vec<u8>>,
}

/// Fetch a feed from a directory and verify it, **trusting the source for nothing**.
///
/// Every object is re-addressed from its own bytes and the head's signature is checked under the
/// key the head claims. A source can therefore withhold or stall — both detectable — and cannot
/// forge, substitute, or silently alter anything.
///
/// `last_seen_seq` guards against rollback: a source replaying an older head to hide a published
/// object is refused. Pass `None` on first contact.
pub fn fetch_and_verify(
    dir: &Path,
    last_seen_seq: Option<u64>,
) -> Result<VerifiedFeed, VerifyError> {
    let head_bytes = std::fs::read(dir.join("head")).map_err(|e| VerifyError::Io(e.to_string()))?;
    let head: FeedHead = ciborium::from_reader(head_bytes.as_slice())
        .map_err(|_| VerifyError::Malformed("head would not decode"))?;

    if let Some(seen) = last_seen_seq {
        if head.seq < seen {
            return Err(VerifyError::Rollback);
        }
    }

    let key_bytes: [u8; 32] = head
        .author
        .clone()
        .try_into()
        .map_err(|_| VerifyError::Malformed("author key size"))?;
    let vk = VerifyingKey::from_bytes(&key_bytes)
        .map_err(|_| VerifyError::Malformed("author key invalid"))?;
    let sig_bytes: [u8; 64] = head
        .sig
        .clone()
        .try_into()
        .map_err(|_| VerifyError::Malformed("signature size"))?;
    let sig = Signature::from_bytes(&sig_bytes);

    let pre = FeedHead::preimage(&head.author, head.seq, &head.objects, head.at);
    vk.verify(&pre, &sig)
        .map_err(|_| VerifyError::BadSignature)?;

    let objdir = dir.join("objects");
    let mut objects = BTreeMap::new();
    for addr in &head.objects {
        let bytes =
            std::fs::read(objdir.join(hex(&addr.0))).map_err(|_| VerifyError::MissingObject)?;
        // Re-address from the bytes actually received. This is the step that makes the server
        // untrusted: a substituted object cannot hash to the address the signed head names.
        if address(&bytes) != *addr {
            return Err(VerifyError::AddressMismatch);
        }
        objects.insert(addr.0.clone(), bytes);
    }

    Ok(VerifiedFeed {
        author: head.author,
        seq: head.seq,
        objects,
    })
}

/// Generate a signing key. Convenience for tests and first-run setup.
pub fn generate_key() -> SigningKey {
    SigningKey::generate(&mut rand_core::OsRng)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("soko-feed-test-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn seeded_feed() -> (Feed, Vec<ContentAddress>) {
        let mut f = Feed::new(generate_key());
        let a = f.add(b"a product record".to_vec());
        let b = f.add(b"an offer over it".to_vec());
        (f, vec![a, b])
    }

    /// The whole claim, end to end: publish a catalogue, fetch it back, and verify it without
    /// trusting whatever served it.
    #[test]
    fn a_published_catalogue_verifies_without_trusting_the_source() {
        let dir = tmp("roundtrip");
        let (mut feed, addrs) = seeded_feed();
        let head = feed.publish(&dir, Timestamp(1)).unwrap();

        let v = fetch_and_verify(&dir, None).unwrap();
        assert_eq!(v.author, head.author);
        assert_eq!(v.objects.len(), 2);
        for a in &addrs {
            assert!(
                v.objects.contains_key(&a.0),
                "a published object is missing after verify"
            );
        }
    }

    /// A source that alters an object is caught, because the fetcher re-addresses the bytes it
    /// actually received rather than believing the name it asked under. This is the property that
    /// makes the server a convenience instead of a trust root.
    #[test]
    fn a_tampered_object_is_caught() {
        let dir = tmp("tamper");
        let (mut feed, addrs) = seeded_feed();
        feed.publish(&dir, Timestamp(1)).unwrap();

        let victim = dir.join("objects").join(hex(&addrs[0].0));
        std::fs::write(&victim, b"substituted content").unwrap();

        assert!(matches!(
            fetch_and_verify(&dir, None),
            Err(VerifyError::AddressMismatch)
        ));
    }

    /// A forged head does not verify under the author's key. Without this, anyone serving the
    /// directory could publish on the seller's behalf.
    #[test]
    fn a_forged_head_is_rejected() {
        let dir = tmp("forge");
        let (mut feed, _) = seeded_feed();
        let mut head = feed.publish(&dir, Timestamp(1)).unwrap();

        head.seq = 99; // rewrite a signed field
        let mut buf = Vec::new();
        ciborium::into_writer(&head, &mut buf).unwrap();
        std::fs::write(dir.join("head"), buf).unwrap();

        assert!(matches!(
            fetch_and_verify(&dir, None),
            Err(VerifyError::BadSignature)
        ));
    }

    /// Replaying an older head to hide something since published is a rollback, not a stale cache,
    /// and is refused. A source that cannot forge can still try to withhold by serving old state.
    #[test]
    fn an_older_head_is_refused_as_a_rollback() {
        let dir = tmp("rollback");
        let (mut feed, _) = seeded_feed();
        feed.publish(&dir, Timestamp(1)).unwrap();
        let second = feed.publish(&dir, Timestamp(2)).unwrap();
        assert_eq!(second.seq, 2);

        assert!(matches!(
            fetch_and_verify(&dir, Some(3)),
            Err(VerifyError::Rollback)
        ));
    }

    /// A withheld object is detected rather than silently producing a short catalogue — the head
    /// names what should be there, so absence is visible.
    #[test]
    fn a_withheld_object_is_detected() {
        let dir = tmp("withhold");
        let (mut feed, addrs) = seeded_feed();
        feed.publish(&dir, Timestamp(1)).unwrap();
        std::fs::remove_file(dir.join("objects").join(hex(&addrs[1].0))).unwrap();

        assert!(matches!(
            fetch_and_verify(&dir, None),
            Err(VerifyError::MissingObject)
        ));
    }

    /// **The exit claim, executed.** The same feed published to two independent locations produces
    /// byte-identical objects and the same author — so moving between gateways is repointing a
    /// name, not migrating data, and a buyer can verify the destination serves the same catalogue
    /// the origin did.
    #[test]
    fn the_same_feed_served_from_two_places_is_byte_identical() {
        let one = tmp("gateway-one");
        let two = tmp("gateway-two");
        let (mut feed, _) = seeded_feed();

        feed.publish(&one, Timestamp(1)).unwrap();
        feed.publish(&two, Timestamp(1)).unwrap();

        let a = fetch_and_verify(&one, None).unwrap();
        let b = fetch_and_verify(&two, None).unwrap();

        assert_eq!(
            a.author, b.author,
            "same seller identity from either source"
        );
        assert_eq!(
            a.objects, b.objects,
            "byte-identical catalogue from either source"
        );
    }

    /// Two different sellers publishing identical bytes converge on the same content address —
    /// the mechanism §2 rests on, demonstrated rather than asserted. (Its limits are §2.2a's
    /// problem, not this one's: identical bytes converge, and getting two publishers to produce
    /// identical bytes is the unsolved part.)
    #[test]
    fn identical_bytes_converge_on_one_address_across_publishers() {
        let mut alice = Feed::new(generate_key());
        let mut bob = Feed::new(generate_key());
        let a = alice.add(b"the very same product record".to_vec());
        let b = bob.add(b"the very same product record".to_vec());
        assert_eq!(a, b, "content addressing is publisher-independent");
    }
}
