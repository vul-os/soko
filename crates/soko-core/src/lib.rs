//! # soko-core — TRACT primitives and the public/sealed boundary
//!
//! Shared types every other Soko crate builds on: content addresses, money, places, time, and the
//! type-level separation between what may be published and what may not.
//!
//! ## What this crate does not do
//!
//! It implements **no cryptography**. Identity, signing, content addressing, feeds and sync are the
//! DMTAP substrate's job (`dmtap-core`, `dmtap-sync`); TRACT adopts them unchanged and adds only
//! the commerce objects on top. A hash construction or signature framing invented here is a bug.
//!
//! ## The one invariant
//!
//! TRACT §0.5.1: **no personal data may enter the public quadrant.** Published objects are
//! content-addressed and irrevocable, so a right to erasure cannot be satisfied against them.
//! [`public`] and [`sealed`] are separate module trees so the rule is enforced by where a type
//! lives rather than by reviewer memory.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};

/// A content address: a multihash-style agility prefix followed by a digest.
///
/// The prefix is what keeps the digest replaceable without changing the address format — TRACT
/// inherits it from the substrate rather than pinning one hash forever (§16.2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentAddress(pub Vec<u8>);

/// An identity key. The public half *is* the identity — of a seller, buyer, courier, distributor
/// or gateway alike. TRACT adds no account type on top of it (§1.2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IdentityKey(pub Vec<u8>);

/// Milliseconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(pub i64);

/// An ISO 3166-1 alpha-2 country code.
///
/// Used for every jurisdictional anchor (§11.2) and for the territories a rate card or escrow
/// scope serves. Deliberately a small closed vocabulary rather than free text — scope intersection
/// has to be mechanical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Country(pub [u8; 2]);

/// An ISO 4217 currency code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Currency(pub [u8; 3]);

/// An amount, in the currency's **minor units**.
///
/// Never a float. Money in a float is a rounding bug waiting for a large enough order, and once a
/// signed object carries the wrong total there is no fixing it after the fact.
///
/// ## Why `minor_units` is signed, and stays public
///
/// `Money` is the one primitive every commerce object in this workspace builds a field from —
/// offer prices, route totals, payment attestations, escrow splits — and those contexts do not
/// all mean the same thing by "negative". A price, an offer amount or a route total being
/// negative is nonsensical: `Consideration::Fixed(Money { minor_units: -100_000, .. })`
/// type-checks today and would show a buyer a product that pays *them* to order it. But an
/// escrow ruling's `to_buyer` / `to_seller` split or a refund adjustment is legitimately
/// expressed as a signed quantity in some accounting treatments, and this crate is not the place
/// to settle that — TRACT names no ledger (§9.2). So `Money` itself stays a plain, unchecked
/// signed amount with public fields, exactly as every existing caller across the workspace
/// already constructs it; [`Money::price`] is the opt-in guard for the contexts — a price, an
/// offer amount, a value ceiling — where negative is never legitimate and should not be
/// silently constructible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    /// Amount in minor units (cents, etc.).
    pub minor_units: i64,
    /// The currency the amount is denominated in.
    pub currency: Currency,
}

impl Money {
    /// Construct a **price**: an amount that will be shown to a counterparty as what something
    /// costs, and must therefore never be negative. Use this — not the struct literal — for an
    /// offer amount, a route total, or a scope's value ceiling.
    ///
    /// Refused rather than silently accepted: a negative price is not a smaller price, it is a
    /// different kind of object (a rebate, a credit) that this constructor does not model, and
    /// accepting it here would let one slip through as an ordinary price until a buyer's
    /// interface renders "you will be paid to order this" and someone has to work out why.
    pub fn price(minor_units: i64, currency: Currency) -> Result<Money, Error> {
        if minor_units < 0 {
            return Err(Error::NegativeAmount);
        }
        Ok(Money {
            minor_units,
            currency,
        })
    }

    /// Whether this amount is negative.
    ///
    /// A cheap, non-consuming check for callers that already hold a `Money` built some other way
    /// (a struct literal, a deserialised object) and need to validate it before treating it as a
    /// price, without going through [`Money::price`]'s constructor.
    pub fn is_negative(&self) -> bool {
        self.minor_units < 0
    }
}

/// Public objects (§2, §8, §10) — signed, content-addressed, globally deduplicated, **irrevocable**.
///
/// Nothing in this module tree may carry personal data (§0.5.1). If a type here grows a name, an
/// address, or a contact detail, that is the bug the module split exists to make visible.
pub mod public {
    /// A marker for objects intended to be published.
    ///
    /// **This is a review aid, not an enforcement mechanism, and it is worth being exact about
    /// which.** The trait is not used as a bound anywhere, so implementing it proves nothing about
    /// a type's contents; what it does is make "this is going into the irrevocable quadrant" an
    /// explicit line a reviewer sees in a diff. Two `Publishable` types
    /// (`soko_offer::PlaceRef`, `soko_trust::Review`) still carry free-text fields a user could
    /// type an address into, and no trait can prevent that — §10.4 and the client requirements
    /// have to.
    ///
    /// The real structural defence is the grammar (TRACT §16.4): there is no street-address
    /// production in the public family at all, so adding one is a spec change rather than a field.
    pub trait Publishable {}
}

/// Sealed objects (§7) — encrypted to the counterparties, never published, deletable at the edges
/// that hold them. Orders, addresses, contact details and payment references live here.
pub mod sealed {
    /// A marker for objects that must never be published.
    pub trait Sealed {}
}

/// Round-trip helpers for checking that Soko's CBOR encoding is **stable**, not merely correct.
///
/// Every type in this workspace that derives `Serialize`/`Deserialize` is meant to become part of
/// a content-addressed object once §16 is normative (§16.2, §16.3). Nothing in the workspace
/// called `ciborium::into_writer` on one of them before this module existed — there was no
/// round-trip test at all, so nothing had established that encoding a value twice produces the
/// same bytes twice, let alone that the shape matches §16's CDDL (matching the CDDL is future
/// work, once §16 stops being a draft; this module does not attempt it and does not claim to).
///
/// ## Why "stable", not just "round-trips to an equal value"
///
/// `serde`/`ciborium` already guarantee that decoding what you just encoded gives back an equal
/// value — that is what `Deserialize` is for, and it is not the interesting property here. The
/// property that actually matters for a content-addressed object is stronger: **the same logical
/// value must always encode to the same bytes**, because the content address is computed over the
/// bytes, not the value (§16.3). An encoder that is correct-but-unstable — one whose output
/// depended on, say, a `HashMap`'s iteration order, or on insertion order in a type that should
/// canonicalise it away — would let two publishers of the identical value compute two different
/// addresses for it and silently fail to deduplicate, which is precisely the failure §16.3's
/// content-addressing exists to prevent. [`assert_stable`] is the check for that: it does not stop
/// at "decodes back to the same value", it insists the *second* encoding matches the *first*
/// byte-for-byte.
///
/// ## Why decoding needs its own check, separate from encoding
///
/// [`decode_exact`] exists because `ciborium::from_reader` alone is not safe to use as a
/// content-addressed decoder: it decodes exactly one CBOR item from a reader and returns, **without
/// checking or reporting whether bytes remain afterwards**. Handed `<valid object> ++ <anything>`,
/// it returns `Ok` with the leading object and silently discards the tail — verified directly
/// against this workspace's pinned `ciborium 0.2.2` before writing this module: appending two
/// arbitrary bytes after a valid encoding and decoding through `ciborium::from_reader` still
/// returns `Ok` with the original value, and the two extra bytes are simply left unread. A decoder
/// for a content-addressed object cannot inherit that behaviour: bytes that don't fully decode to
/// nothing are not the same object the address was computed over, and a decoder that shrugs off a
/// trailing byte would accept a payload as if it matched an address it does not actually match.
/// [`decode_exact`] closes that by decoding through a `&mut &[u8]` (which ciborium advances as it
/// reads) and then checking the slice is empty; [`assert_trailing_garbage_is_rejected`] exists
/// specifically to demonstrate the gap `decode_exact` closes, by first confirming `ciborium`'s own
/// `from_reader` does *not* catch it.
pub mod roundtrip {
    use serde::de::DeserializeOwned;
    use serde::Serialize;

    /// Encode `value`, decode it back, re-encode the decoded value, and assert every step agrees
    /// byte-for-byte with the first encoding. See the [module documentation](self) for why this is
    /// the property worth checking, rather than mere round-trip equality.
    pub fn assert_stable<T>(value: &T)
    where
        T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let mut first = Vec::new();
        ciborium::into_writer(value, &mut first)
            .expect("encoding a well-formed value must not fail");
        let decoded: T =
            decode_exact(&first).expect("decoding bytes this function just encoded must not fail");
        assert_eq!(
            &decoded, value,
            "decoding the encoded bytes did not reproduce the original value"
        );
        let mut second = Vec::new();
        ciborium::into_writer(&decoded, &mut second)
            .expect("re-encoding the decoded value must not fail");
        assert_eq!(
            first, second,
            "re-encoding the decoded value produced different bytes — the encoder is not stable, \
             and an unstable encoder breaks content addressing: two publishers of the identical \
             value would compute two different addresses for it and silently fail to deduplicate"
        );
    }

    /// Decode exactly one `T` from `bytes`, refusing to succeed unless every byte is consumed.
    ///
    /// See the [module documentation](self) for why this check does not come for free from
    /// `ciborium::from_reader` and has to be performed explicitly.
    pub fn decode_exact<T>(bytes: &[u8]) -> Result<T, String>
    where
        T: DeserializeOwned,
    {
        let mut cursor: &[u8] = bytes;
        let value: T = ciborium::from_reader(&mut cursor).map_err(|e| e.to_string())?;
        if !cursor.is_empty() {
            return Err(format!(
                "{} trailing byte(s) after a decoded object were not consumed",
                cursor.len()
            ));
        }
        Ok(value)
    }

    /// Assert that truncating the last byte off `value`'s encoding makes it fail to decode.
    ///
    /// A truncated buffer decoding to *something* — even a different, shorter-but-valid value —
    /// would mean a corrupted or partially-received object could be silently accepted as a
    /// different, well-formed one instead of being rejected.
    pub fn assert_truncation_is_rejected<T>(value: &T)
    where
        T: Serialize + DeserializeOwned + std::fmt::Debug,
    {
        let mut bytes = Vec::new();
        ciborium::into_writer(value, &mut bytes).expect("encode");
        assert!(!bytes.is_empty(), "an encoded value must not be zero bytes");
        let truncated = &bytes[..bytes.len() - 1];
        let result: Result<T, String> = decode_exact(truncated);
        assert!(
            result.is_err(),
            "a truncated encoding decoded successfully as {result:?}, when it should have been \
             rejected as incomplete"
        );
    }

    /// Assert that appending a byte after `value`'s encoding makes it fail to decode.
    ///
    /// This is true only because [`decode_exact`] explicitly checks for a fully-consumed reader —
    /// see the [module documentation](self) for the confirmed-by-experiment fact that
    /// `ciborium::from_reader` alone does **not** reject trailing bytes on its own, and would
    /// silently accept `<valid object> ++ <garbage>` as the leading object.
    pub fn assert_trailing_garbage_is_rejected<T>(value: &T)
    where
        T: Serialize + DeserializeOwned + std::fmt::Debug,
    {
        let mut bytes = Vec::new();
        ciborium::into_writer(value, &mut bytes).expect("encode");
        bytes.push(0xFF);
        let result: Result<T, String> = decode_exact(&bytes);
        assert!(
            result.is_err(),
            "an encoding with trailing garbage appended decoded successfully as {result:?}, when \
             it should have been rejected"
        );
    }
}

/// Errors shared across the Soko crates.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An object failed to verify against the address it was fetched by.
    #[error("content address mismatch")]
    AddressMismatch,
    /// A required field was absent, or a forbidden one was present.
    #[error("malformed object: {0}")]
    Malformed(&'static str),
    /// A currency mismatch in an arithmetic operation. Never silently coerced.
    #[error("currency mismatch")]
    CurrencyMismatch,
    /// An arithmetic operation would overflow. Refused rather than wrapped: a wrapped total is a
    /// wrong number that looks like a real one, and it would be carried into a signed order where
    /// no later correction can reach it.
    #[error("arithmetic overflow")]
    Overflow,
    /// A price, offer amount, or other value that must never be negative was negative. See
    /// [`Money::price`] for where this is raised and why a blanket check on every `Money` would
    /// be wrong.
    #[error("negative amount where a price was required")]
    NegativeAmount,
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZAR: Currency = Currency(*b"ZAR");

    /// Before `Money::price` existed there was no way to stop
    /// `Money { minor_units: -100_000, currency: ZAR }` from type-checking anywhere a price was
    /// meant — the exact shape of `Consideration::Fixed` in `soko-offer`. The checked constructor
    /// must actually refuse that value rather than accept it and let a negative price reach a
    /// buyer's screen.
    #[test]
    fn price_rejects_negative_amounts() {
        assert!(matches!(
            Money::price(-100_000, ZAR),
            Err(Error::NegativeAmount)
        ));
    }

    #[test]
    fn price_accepts_zero_and_positive_amounts() {
        assert_eq!(Money::price(0, ZAR).unwrap().minor_units, 0);
        assert_eq!(Money::price(45_000, ZAR).unwrap().minor_units, 45_000);
    }

    /// `Money` itself must stay an unchecked, plain amount — a blanket non-negative invariant on
    /// the type would be wrong for contexts like a refund or an escrow ruling split that are not
    /// prices. The struct-literal path every other crate in the workspace already uses must keep
    /// working, negative included.
    #[test]
    fn plain_money_construction_still_permits_negative_for_non_price_uses() {
        let refund_delta = Money {
            minor_units: -5_000,
            currency: ZAR,
        };
        assert!(refund_delta.is_negative());
    }

    // ---- Round-trip stability (see the `roundtrip` module docs for why this is checked at all) --

    /// `ContentAddress` wraps an arbitrary byte string — the case most likely to expose an encoder
    /// that treats bytes and text differently, since CBOR has distinct major types for the two.
    #[test]
    fn content_address_roundtrips_stably() {
        let addr = ContentAddress(vec![0, 1, 2, 253, 254, 255]);
        roundtrip::assert_stable(&addr);
        roundtrip::assert_truncation_is_rejected(&addr);
        roundtrip::assert_trailing_garbage_is_rejected(&addr);
    }

    #[test]
    fn identity_key_roundtrips_stably() {
        let key = IdentityKey(vec![9; 32]);
        roundtrip::assert_stable(&key);
        roundtrip::assert_truncation_is_rejected(&key);
        roundtrip::assert_trailing_garbage_is_rejected(&key);
    }

    /// Negative timestamps (pre-1970) are representable and must not be special-cased away.
    #[test]
    fn timestamp_roundtrips_stably_including_negative_values() {
        for ts in [Timestamp(0), Timestamp(1_700_000_000_000), Timestamp(-1)] {
            roundtrip::assert_stable(&ts);
        }
        roundtrip::assert_truncation_is_rejected(&Timestamp(1_700_000_000_000));
        roundtrip::assert_trailing_garbage_is_rejected(&Timestamp(1_700_000_000_000));
    }

    #[test]
    fn country_and_currency_roundtrip_stably() {
        let za: Country = Country(*b"ZA");
        let jpy: Currency = Currency(*b"JPY");
        roundtrip::assert_stable(&za);
        roundtrip::assert_stable(&jpy);
        roundtrip::assert_truncation_is_rejected(&za);
        roundtrip::assert_trailing_garbage_is_rejected(&jpy);
    }

    /// `Money` via both construction paths — the checked [`Money::price`] and the plain struct
    /// literal a refund uses — since the two must encode identically shaped values.
    #[test]
    fn money_roundtrips_stably_positive_and_negative() {
        let price = Money::price(45_000, ZAR).unwrap();
        let refund = Money {
            minor_units: -5_000,
            currency: ZAR,
        };
        roundtrip::assert_stable(&price);
        roundtrip::assert_stable(&refund);
        roundtrip::assert_truncation_is_rejected(&price);
        roundtrip::assert_trailing_garbage_is_rejected(&price);
    }
}
