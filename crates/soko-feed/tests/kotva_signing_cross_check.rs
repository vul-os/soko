//! Cross-repo backlog A11 (signing half): does `soko_feed`'s own Ed25519 signing agree with the
//! substrate's own signing primitive, or only with this crate's restatement of the
//! domain-separation convention?
//!
//! `Feed::head` (`crates/soko-feed/src/lib.rs`, ~line 162) signs a feed head directly with
//! `ed25519_dalek::SigningKey::sign`, over a preimage that is hand-assembled by
//! `FeedHead::preimage`: `HEAD_DS ‖ author ‖ seq(be) ‖ objects ‖ at(be)`, where `HEAD_DS =
//! b"TRACT-v0/feed-head\x00"` is soko's own domain-separation tag for this TRACT object type.
//!
//! `kotva_core::identity::IdentityKey::sign_domain(domain, msg)`
//! (`kotva/crates/kotva-core/src/identity.rs`) exists for exactly this pattern elsewhere in the
//! substrate: it signs `Ed25519(key, domain ‖ msg)`. If soko's hand-rolled
//! `key.sign(HEAD_DS ‖ msg)` and kotva's `sign_domain(HEAD_DS, msg)` are the *same construction*,
//! they must produce byte-identical signatures for the same key and the same `(domain, msg)`.
//! This is not a statistical claim: Ed25519 signing is deterministic per RFC 8032 (neither side
//! uses the randomized/prehashed variant), so if the two disagree about what bytes actually get
//! signed, they disagree on essentially every input, not occasionally.
//!
//! `HEAD_DS` is a private constant in `soko_feed` — deliberately: it is soko's own tag for a TRACT
//! object kotva has no notion of, not one of kotva's own DS-tagged object types (`identity`,
//! `device-cert`, `recovery-policy`, `move-record`). So this test hardcodes soko's literal DS
//! bytes rather than importing them, the same way `kotva_content_address_cross_check.rs` hardcodes
//! `0x1e` instead of importing the private `BLAKE3_PREFIX`. What is being cross-checked is the
//! *mechanism* — concatenate a domain tag onto a message, then Ed25519-sign the result — not the
//! tag string itself, which kotva has no matching entry for by design.
//!
//! `kotva-core` is a **dev-dependency only** (see `Cargo.toml`), matching the content-address
//! cross-check: nothing in soko's production dependency graph changes.
//!
//! **Result: they agree.** Every case below is byte-for-byte identical between soko's hand-rolled
//! signing and `kotva_core::identity::IdentityKey::sign_domain`.

use ed25519_dalek::SigningKey;
use kotva_core::identity::IdentityKey;
use soko_core::Timestamp;
use soko_feed::{Feed, FeedHead};

/// Soko's own domain-separation tag for a feed head (`crates/soko-feed/src/lib.rs`, `HEAD_DS`).
/// Private in that crate, so pinned here literally — see the module doc above.
const HEAD_DS: &[u8] = b"TRACT-v0/feed-head\x00";

/// Reconstruct the part of `FeedHead::preimage` that comes *after* `HEAD_DS`, from `FeedHead`'s
/// public fields: `author ‖ seq(be) ‖ objects ‖ at(be)`. `sign_domain(HEAD_DS, this)` must then
/// equal `head.sig` if soko's signing and kotva's agree on the construction.
fn message_after_domain_tag(head: &FeedHead) -> Vec<u8> {
    let mut m = Vec::new();
    m.extend_from_slice(&head.author);
    m.extend_from_slice(&head.seq.to_be_bytes());
    for o in &head.objects {
        m.extend_from_slice(&o.0);
    }
    m.extend_from_slice(&head.at.0.to_be_bytes());
    m
}

/// For a fixed key and a real, produced `FeedHead`, `head.sig` (soko's own signing path) must be
/// byte-identical to `kotva_core::identity::IdentityKey::sign_domain(HEAD_DS, msg)` run
/// independently over the reconstructed message.
#[test]
fn feed_head_signature_matches_kotva_sign_domain_byte_for_byte() {
    let seed = [0x42u8; 32];
    let soko_key = SigningKey::from_bytes(&seed);
    let kotva_key = IdentityKey::from_seed(&seed);

    let mut feed = Feed::new(soko_key);
    feed.add(b"the very same product record".to_vec());
    feed.add(b"an offer over it".to_vec());
    let head = feed.head(Timestamp(1_700_000_000));

    let msg = message_after_domain_tag(&head);
    let kotva_sig = kotva_key.sign_domain(HEAD_DS, &msg);

    assert_eq!(
        head.sig, kotva_sig,
        "soko_feed's key.sign(HEAD_DS || msg) and kotva_core::identity::IdentityKey::sign_domain \
         disagree for the same key and message"
    );
}

/// Multiple feeds/keys/object-counts/timestamps, not just one lucky sample — mirrors the
/// "checked" coverage discipline of `kotva_content_address_cross_check.rs`. Ed25519 signing is
/// deterministic, so this is not a statistical hedge; it guards against a construction bug that a
/// single case could miss, e.g. an empty object list, an empty object's zero-length bytes
/// vanishing into the concatenation, a `seq`/`at` value whose big-endian encoding has leading zero
/// bytes, or a bumped `seq` (`Feed::head` is called more than once on one case, to get `seq != 1`).
#[test]
fn agreement_holds_across_several_feeds_not_one_lucky_sample() {
    let cases: Vec<(u8, Vec<&[u8]>, i64, u32)> = vec![
        (0x00, vec![], 0, 0),
        (0x01, vec![b"solo object"], 1, 0),
        (0x42, vec![b"a product record", b"an offer over it"], 1_700_000_000, 0),
        (0xff, vec![b"", b"\x00\x00\x00", b"three objects, one empty"], -1, 0),
        (0x07, vec![b"bumped sequence"], 5, 2), // two discarded head() calls first: seq ends at 3
    ];
    assert_eq!(cases.len(), 5, "coverage: every listed case must actually run");

    let mut checked = 0;
    for (seed_byte, objects, at, extra_head_calls) in &cases {
        let seed = [*seed_byte; 32];
        let soko_key = SigningKey::from_bytes(&seed);
        let kotva_key = IdentityKey::from_seed(&seed);

        let mut feed = Feed::new(soko_key);
        for bytes in objects {
            feed.add(bytes.to_vec());
        }
        for _ in 0..*extra_head_calls {
            feed.head(Timestamp(*at)); // discarded, just to advance `seq`
        }
        let head = feed.head(Timestamp(*at));

        let msg = message_after_domain_tag(&head);
        let kotva_sig = kotva_key.sign_domain(HEAD_DS, &msg);

        assert_eq!(
            head.sig, kotva_sig,
            "disagreement for seed {seed_byte:#04x}, {} object(s), at={at}, seq={}",
            objects.len(),
            head.seq
        );
        checked += 1;
    }
    assert_eq!(
        checked,
        cases.len(),
        "every case must have been compared, not skipped"
    );
}

/// Pin why the equality above is meaningful rather than coincidental: swapping in the wrong
/// domain-separation tag (or dropping it) must change the signature. If `sign_domain` silently
/// ignored its `domain` argument, the byte-for-byte match above would hold for the wrong reason —
/// it would just mean both sides happened to sign the bare message.
#[test]
fn the_domain_separation_tag_actually_changes_the_signature() {
    let seed = [0x99u8; 32];
    let soko_key = SigningKey::from_bytes(&seed);
    let kotva_key = IdentityKey::from_seed(&seed);

    let mut feed = Feed::new(soko_key);
    feed.add(b"a product record".to_vec());
    let head = feed.head(Timestamp(1));
    let msg = message_after_domain_tag(&head);

    let correct = kotva_key.sign_domain(HEAD_DS, &msg);
    let wrong_tag = kotva_key.sign_domain(b"TRACT-v0/not-feed-head\x00", &msg);
    let no_tag = kotva_key.sign_domain(b"", &msg);

    assert_eq!(
        head.sig, correct,
        "sanity check: the correct tag must still agree (see the primary test above)"
    );
    assert_ne!(
        head.sig, wrong_tag,
        "a different domain tag must not sign the same as soko's real head signature"
    );
    assert_ne!(
        head.sig, no_tag,
        "signing with no domain tag must not sign the same as soko's real head signature"
    );
}
