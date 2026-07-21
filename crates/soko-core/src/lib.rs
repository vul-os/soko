//! # soko-core — TRACT object model and wire format
//!
//! The signed and content-addressed objects of TRACT §16: product records, offers, rate cards,
//! reviews, and the sealed order body.
//!
//! ## What this crate does not do
//!
//! It implements **no cryptography of its own**. Identity, signing, content addressing, feeds
//! and sync are the DMTAP substrate's job (`dmtap-core`, `dmtap-sync`); TRACT adopts them
//! unchanged under the substrate's à-la-carte rule and adds only the commerce objects on top.
//! If you find a hash construction or a signature framing being invented in this crate, that is
//! a bug — the substrate governs those bytes.
//!
//! ## The one invariant worth stating first
//!
//! TRACT §0.5.1: **no personal data may enter the public quadrant.** Published objects are
//! content-addressed and irrevocable, so a right to erasure cannot be satisfied against them.
//! Product records, offers and rate cards are public; orders, addresses and contact details are
//! sealed, always. This crate keeps the two in separate types so the distinction cannot be lost
//! by accident — a public object should be structurally incapable of carrying an address.
//!
//! ## Status
//!
//! Scaffold. Types land as TRACT §16 settles them.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Public objects (TRACT §2, §8, §10) — signed, content-addressed, globally deduplicated,
/// and **irrevocable**. Nothing in this module may carry personal data (§0.5.1).
pub mod public {}

/// Sealed objects (TRACT §7) — encrypted to the counterparties, never published, deletable at
/// the edges that hold them. Orders and everything identifying a person live here.
pub mod sealed {}
