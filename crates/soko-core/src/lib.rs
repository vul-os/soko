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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    /// Amount in minor units (cents, etc.).
    pub minor_units: i64,
    /// The currency the amount is denominated in.
    pub currency: Currency,
}

/// Public objects (§2, §8, §10) — signed, content-addressed, globally deduplicated, **irrevocable**.
///
/// Nothing in this module tree may carry personal data (§0.5.1). If a type here grows a name, an
/// address, or a contact detail, that is the bug the module split exists to make visible.
pub mod public {
    /// A marker for objects that are safe to publish.
    ///
    /// Implemented only by types that carry no personal data. It is a discipline, not a proof —
    /// but a type that has to opt in is one a reviewer will notice.
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
}
