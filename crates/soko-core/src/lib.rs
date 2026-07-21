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
}
