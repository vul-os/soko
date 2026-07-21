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
use unicode_normalization::UnicodeNormalization;

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
/// what canonicalisation manages to make identical. What it does — Unicode-normalise, trim,
/// casefold keys, sort attributes, collapse internal whitespace, resolve same-key conflicts — is
/// the easy part. What it cannot do is reconcile two publishers who genuinely describe a product
/// differently, and no amount of normalisation will.
///
/// ## Why Unicode normalisation, specifically
///
/// Two publishers' input methods can produce the same visible text as different byte sequences —
/// `"é"` as one precomposed code point (U+00E9) versus `"e"` followed by a combining acute accent
/// (U+0065 U+0301) is the canonical example, and it is common in practice: some keyboards and
/// OSes normalise on input, some don't, and text copied between them mixes freely. Without
/// resolving that, two publishers typing the visibly identical product name would hash to two
/// different addresses, which defeats this function's entire stated purpose — convergence — for
/// exactly the case (non-ASCII names, most of the world) where it matters most. NFC (Normalization
/// Form C, the composed form) is applied before whitespace collapsing, on `name`, `description`
/// and every attribute value.
///
/// ## Why a same-key conflict resolves by last-sorted-value rather than an error
///
/// Sorting on the whole `Attribute` struct and then removing only byte-identical adjacent entries
/// — the previous behaviour — let two attributes that share a key but disagree on value (`colour:
/// blue` and `colour: navy`) both survive: a canonicalised record could carry two contradictory
/// claims about the same product with no error and no signal. That is malformed input, and the
/// honest fix would be for this function to refuse it. It cannot: `canonicalise` returns
/// `ProductRecord` rather than `Result`, every caller across the workspace already relies on that,
/// and widening the signature here would ripple into crates this fix does not touch. So the
/// conflict is instead resolved **deterministically**: attributes are sorted by `(key, value)`, so
/// for a repeated key the last entry in that run is the lexicographically greatest value, and
/// keeping only it is a rule that converges regardless of the input's original order — two
/// publishers who each hit this bug independently still produce the same resolved record, rather
/// than one that depends on which value happened to sort where by accident. It does not make the
/// input valid; it makes the function total and its output predictable in the face of invalid
/// input. Rejecting a self-contradictory record outright is a publish-time validator's job, not
/// this function's.
pub fn canonicalise(mut r: ProductRecord) -> ProductRecord {
    fn norm(s: &str) -> String {
        s.nfc()
            .collect::<String>()
            .split_whitespace()
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
    // Fold same-key runs down to the last (lexicographically greatest) value. See the doc comment
    // above for why this is last-wins rather than an error.
    let mut deduped: Vec<Attribute> = Vec::with_capacity(r.attributes.len());
    for a in r.attributes {
        match deduped.last_mut() {
            Some(prev) if prev.key == a.key => *prev = a,
            _ => deduped.push(a),
        }
    }
    r.attributes = deduped;
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

    /// The Unicode-normalisation gap this function had: two visibly identical names built from
    /// differently-composed code points must converge. Against the OLD implementation (ASCII
    /// whitespace collapsing only, no NFC) `a.name` and `b.name` were different byte sequences and
    /// this assertion failed — the exact failure mode of "two publishers describing the same shoe
    /// never converge" that canonicalisation exists to prevent.
    #[test]
    fn differently_composed_unicode_converges_after_normalisation() {
        // "café" with the é as one precomposed code point, U+00E9.
        let precomposed = "Caf\u{00E9}";
        // "café" with the é as "e" (U+0065) followed by a combining acute accent, U+0301.
        let decomposed = "Cafe\u{0301}";
        assert_ne!(
            precomposed.as_bytes(),
            decomposed.as_bytes(),
            "the two source strings must actually differ at the byte level for this test to mean \
             anything"
        );
        let a = canonicalise(rec(precomposed, &[]));
        let b = canonicalise(rec(decomposed, &[]));
        assert_eq!(
            a, b,
            "NFC normalisation must fold both compositions of \u{e9} to the same bytes"
        );
        assert_eq!(a.name, "Caf\u{00E9}");
    }

    /// Unicode normalisation must also reach attribute values, not just name/description — a
    /// colour or size given in a differently-composed form is the same failure mode.
    #[test]
    fn differently_composed_unicode_converges_in_attribute_values() {
        let a = canonicalise(rec("Beret", &[("colour", "Bordeaux \u{00E9}dition")]));
        let b = canonicalise(rec("Beret", &[("colour", "Bordeaux e\u{0301}dition")]));
        assert_eq!(a, b);
    }

    /// The duplicate-key finding: a record with two attributes sharing a key but disagreeing on
    /// value must not silently keep both. Against the OLD implementation
    /// (`attributes.sort(); attributes.dedup();`, which only collapses byte-identical
    /// (key, value) pairs) this record kept both `colour` entries and `attributes.len()` was 2.
    #[test]
    fn conflicting_duplicate_attribute_keys_resolve_to_one_deterministic_value() {
        let a = canonicalise(rec("Shoe", &[("colour", "blue"), ("colour", "navy")]));
        assert_eq!(
            a.attributes.len(),
            1,
            "a record must not carry two contradictory values for the same key"
        );
        // Sorted by (key, value): "blue" < "navy" lexicographically, so "navy" is the last entry
        // in the run and wins.
        assert_eq!(a.attributes[0].value, "navy");

        // The resolution must not depend on which order the conflicting values were supplied in —
        // that determinism is the whole point, since two publishers who each hit this bug must
        // still converge on an identical resolved record.
        let b = canonicalise(rec("Shoe", &[("colour", "navy"), ("colour", "blue")]));
        assert_eq!(a, b);
    }
}

/// Property-based tests for [`canonicalise`].
///
/// The unit tests above are specific, hand-picked failure modes (a particular Unicode
/// composition, a particular duplicate key). These properties instead check what has to be true
/// of *every* input for `canonicalise` to do its job at all — no finite set of examples
/// establishes either property, because a counterexample could hide anywhere in the input space.
#[cfg(test)]
mod canonicalise_proptests {
    use super::*;
    use proptest::prelude::*;

    /// A short, printable string — long enough to exercise whitespace/case/Unicode handling,
    /// short enough that proptest can explore thousands of them per second.
    fn text_strategy() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 ]{0,16}"
    }

    fn attribute_strategy() -> impl Strategy<Value = Attribute> {
        (text_strategy(), text_strategy()).prop_map(|(key, value)| Attribute { key, value })
    }

    fn product_record_strategy() -> impl Strategy<Value = ProductRecord> {
        (
            text_strategy(),
            text_strategy(),
            prop::collection::vec(attribute_strategy(), 0..8),
        )
            .prop_map(|(name, description, attributes)| ProductRecord {
                name,
                description,
                attributes,
                identity: vec![],
                group: None,
                components: vec![],
            })
    }

    /// A Fisher–Yates shuffle driven by a proptest-generated seed rather than by pulling from an
    /// external `rand` crate at test time — this keeps `proptest` the only new dev-dependency and
    /// keeps the permutation reproducible from the seed proptest already reports on shrink.
    fn shuffled(items: &[Attribute], seed: u64) -> Vec<Attribute> {
        let mut out = items.to_vec();
        let mut state = seed | 1; // avoid the fixed point at 0
        for i in (1..out.len()).rev() {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let j = (state as usize) % (i + 1);
            out.swap(i, j);
        }
        out
    }

    proptest! {
        /// `canonicalise` must be idempotent: applying it a second time to its own output must be
        /// a no-op. This is essential rather than a nicety — a publisher canonicalises before
        /// publishing, but nothing stops re-canonicalisation happening again downstream (a mirror,
        /// a re-index, a second publisher re-deriving the record from a feed). If a second pass
        /// changed the record, its content address would change too, and two publishers who both
        /// dutifully canonicalise would still diverge — exactly the failure this function exists
        /// to prevent, just moved one step later.
        #[test]
        fn canonicalisation_is_idempotent(r in product_record_strategy()) {
            let once = canonicalise(r);
            let twice = canonicalise(once.clone());
            prop_assert_eq!(once, twice);
        }

        /// `canonicalise` must be insensitive to the order attributes were supplied in: shuffling
        /// the input attribute list must not change the output. Two sellers who describe the same
        /// product but populate their attribute maps in different orders (a difference with no
        /// substantive meaning) must still converge on the same address — an order-sensitive
        /// result would fail to deduplicate for a reason that has nothing to do with the product
        /// actually differing.
        #[test]
        fn canonicalisation_is_order_insensitive_for_attributes(
            name in text_strategy(),
            description in text_strategy(),
            attributes in prop::collection::vec(attribute_strategy(), 0..8),
            seed in any::<u64>(),
        ) {
            let base = ProductRecord {
                name: name.clone(),
                description: description.clone(),
                attributes: attributes.clone(),
                identity: vec![],
                group: None,
                components: vec![],
            };
            let reordered = ProductRecord {
                name,
                description,
                attributes: shuffled(&attributes, seed),
                identity: vec![],
                group: None,
                components: vec![],
            };
            prop_assert_eq!(canonicalise(base), canonicalise(reordered));
        }
    }
}
