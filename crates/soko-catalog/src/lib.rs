//! # soko-catalog — product records and the identity ladder (TRACT §2)
//!
//! A **product record** describes what a thing *is*. An [`soko_offer::Offer`] is one seller's claim
//! to supply it. The split is what lets many sellers reference the same product without a registrar
//! issuing IDs: because the substrate addresses public blobs over plaintext, two sellers publishing
//! the same record converge on the same address, and the swarm stores it once.
//!
//! ## How strong that claim actually is
//!
//! Weaker than it sounds, and the crate says so rather than implying otherwise. Convergence is
//! trivially true for identical bytes and says nothing about the real case: two shops describing
//! the same shoe. A 2026 literature pass found **no deployed system achieving cross-publisher
//! product identity without a licensed registry**, and the one candidate for permissionless
//! crawl-derived resolution was refuted under adversarial verification. So the content address is a
//! sound *mechanism* carrying an *unproven* claim, and [`canonicalise`] — not the hashing — is
//! where the work is.
//!
//! ## The ladder
//!
//! [`IdentityRung`] is deliberately the same shape as DMTAP's naming ladder: a zero-authority floor
//! that always works, an unverified convenience layer, and an authority rung that requires someone
//! to participate.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use soko_core::{ContentAddress, IdentityKey};

/// How strongly a product's identity is established.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentityRung {
    /// The floor: the content address of the canonicalised record. Always available, needs no
    /// registrar — but only collapses records that are byte-identical after canonicalisation.
    ContentAddress(ContentAddress),
    /// A claimed external identifier — GTIN, MPN, ISBN. **Anyone can claim any value**; an index
    /// must treat these as advisory join keys, never as identity. Squatting is expected.
    ClaimedExternal {
        /// The scheme, e.g. `gtin` or `mpn`.
        scheme: String,
        /// The claimed value.
        value: String,
    },
    /// The record is signed by the manufacturer's own key.
    ///
    /// This is the rung with real authority, and it inverts the platform model: on a centralized
    /// marketplace the *marketplace* is authoritative about a product's specifications, whereas
    /// here a reseller may add an offer but cannot misdescribe a product it did not author.
    ManufacturerSigned(IdentityKey),
}

/// What a thing is — never who sells it, or for how much.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductRecord {
    /// Human-readable name.
    pub name: String,
    /// Free-form description.
    pub description: String,
    /// Structured attributes. Canonicalised before addressing.
    pub attributes: Vec<Attribute>,
    /// Identity claims, strongest last.
    pub identity: Vec<IdentityRung>,
    /// If this is a variant, the group it belongs to.
    pub group: Option<ContentAddress>,
    /// Components, for a bundle or kit. May reference records published by *other* sellers, which
    /// is what makes a cross-seller bundle expressible at all.
    pub components: Vec<ContentAddress>,
}

impl soko_core::public::Publishable for ProductRecord {}

/// One structured attribute of a product.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Attribute {
    /// Attribute key, e.g. `colour`.
    pub key: String,
    /// Attribute value, e.g. `blue`.
    pub value: String,
}

/// A product group and the axes its variants vary along.
///
/// Follows the schema.org `ProductGroup` / `variesBy` model rather than a bespoke shape, so
/// existing merchant feeds map in by translation instead of redesign.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductGroup {
    /// Group name.
    pub name: String,
    /// The attribute keys that vary across variants — `size`, `colour`.
    pub varies_by: Vec<String>,
    /// The variant records in this group.
    pub variants: Vec<ContentAddress>,
}

impl soko_core::public::Publishable for ProductGroup {}

/// Normalise a record so independent publishers have a chance of converging.
///
/// This is the load-bearing operation of the whole section: the content address only deduplicates
/// what canonicalisation manages to make identical. What it does — trim, casefold keys, sort
/// attributes, collapse internal whitespace — is the easy part. What it cannot do is reconcile two
/// publishers who genuinely describe a product differently, and no amount of normalisation will.
pub fn canonicalise(mut r: ProductRecord) -> ProductRecord {
    fn norm(s: &str) -> String {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    }
    r.name = norm(&r.name);
    r.description = norm(&r.description);
    for a in &mut r.attributes {
        a.key = norm(&a.key).to_lowercase();
        a.value = norm(&a.value);
    }
    r.attributes.sort();
    r.attributes.dedup();
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(name: &str, attrs: &[(&str, &str)]) -> ProductRecord {
        ProductRecord {
            name: name.into(),
            description: "  a   shoe ".into(),
            attributes: attrs
                .iter()
                .map(|(k, v)| Attribute {
                    key: (*k).into(),
                    value: (*v).into(),
                })
                .collect(),
            identity: vec![],
            group: None,
            components: vec![],
        }
    }

    /// Attribute order and key casing must not change the record, or two sellers listing the same
    /// product in a different order would fail to converge for no substantive reason.
    #[test]
    fn canonicalisation_is_order_and_case_insensitive_for_keys() {
        let a = canonicalise(rec("Shoe", &[("Colour", "blue"), ("size", "42")]));
        let b = canonicalise(rec("Shoe", &[("size", "42"), ("colour", "blue")]));
        assert_eq!(a, b);
    }

    #[test]
    fn canonicalisation_collapses_whitespace() {
        let a = canonicalise(rec("  Running   Shoe ", &[]));
        assert_eq!(a.name, "Running Shoe");
        assert_eq!(a.description, "a shoe");
    }

    /// The honest limit, asserted so nobody mistakes the floor for a solution: a substantive
    /// difference in description does NOT converge, and canonicalisation cannot fix that.
    #[test]
    fn substantively_different_descriptions_do_not_converge() {
        let a = canonicalise(rec("Running Shoe", &[("colour", "blue")]));
        let mut b = rec("Running Shoe", &[("colour", "navy")]);
        b.description = "a shoe".into();
        assert_ne!(a, canonicalise(b));
    }
}
