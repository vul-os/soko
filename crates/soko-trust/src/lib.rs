//! # soko-trust — purchase-attested reviews, local ranking (TRACT §10)
//!
//! ## Why there is no global score
//!
//! A single canonical rating requires a party that aggregates and ranks. That party decides what
//! counts, who is visible and who is buried — it is the marketplace, reintroduced under another
//! name. So this crate provides reviews and attestations, and **no global scoring function**.
//! Ranking is derived data any node computes, and different indexes will disagree. That is the
//! intended outcome, and the cost is real: buyers lose the convenience of one authoritative number.
//!
//! ## What attestation does and does not fix
//!
//! OpenBazaar's reputation failure was self-published, unweighted reviews with no banning
//! authority, and **one vendor faked 60% of measured sales value**. A [`PurchaseAttestation`] is
//! strictly stronger than what OpenBazaar had — it binds a review to a real transaction, so
//! ballot-stuffing costs actual trades. But two of the failure conditions survive:
//!
//! 1. **Self-dealing produces genuine attestations.** A seller transacting with itself generates
//!    real proofs. Attestation raises cost; it does not establish counterparty independence.
//! 2. **Opt-in escrow is declined by exactly the actors it constrains**, so escrow-issued
//!    attestations are scarcest where they would matter most.
//!
//! The achievable Sybil-cost floor on a signed-feed substrate is an open question, not a solved
//! one, and [`Weighting`] exists so an index can express its own answer rather than inherit one.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use soko_core::{ContentAddress, IdentityKey, Timestamp};

/// Who vouched that a review's author actually transacted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Attestor {
    /// The seller confirmed the sale. Cheap to obtain — including from yourself.
    Seller,
    /// An escrow operator confirmed completion. Stronger, because a third party observed both
    /// sides; scarcer, because escrow is optional.
    Escrow,
}

/// Proof that a review's author really bought the thing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseAttestation {
    /// Who issued it.
    pub attestor: Attestor,
    /// The identity that issued it.
    pub issuer: IdentityKey,
    /// The order attested. Sealed content; only its address appears here.
    pub order: ContentAddress,
    /// When it was issued.
    pub at: Timestamp,
}

/// What is being reviewed. Couriers and distributors are reviewable too — they are counterparties
/// with the same standing as a seller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Subject {
    /// A product record.
    Product(ContentAddress),
    /// A seller.
    Seller(IdentityKey),
    /// A distributor.
    Distributor(IdentityKey),
    /// A courier.
    Courier(IdentityKey),
}

/// A signed, public review.
///
/// Signed with a **per-subject pseudonymous subkey**, not the author's root identity, so a review
/// is not trivially linkable across sellers. It remains personal data, which is why §10.4 treats
/// reviews as the single bounded exception to "nothing personal is ever published".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    /// What is reviewed.
    pub subject: Subject,
    /// Pseudonymous author subkey.
    pub author: IdentityKey,
    /// Score out of five.
    pub score: u8,
    /// Free text.
    pub body: String,
    /// Proof of purchase, if any. `None` is allowed and should be weighted accordingly.
    pub attestation: Option<PurchaseAttestation>,
    /// When written.
    pub at: Timestamp,
}

impl soko_core::public::Publishable for Review {}

/// An index's own weighting policy.
///
/// There is no default, deliberately. Every index answers "what counts?" for itself, and a seller
/// who dislikes one index's answer can use or build another — which is the property that keeps the
/// network non-capturable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weighting {
    /// Weight for a review with no attestation at all.
    pub unattested: u8,
    /// Weight for a seller-attested review.
    pub seller_attested: u8,
    /// Weight for an escrow-attested review.
    pub escrow_attested: u8,
}

impl Weighting {
    /// A conservative starting point, not a standard: escrow-attested reviews count most,
    /// unattested ones count nothing. An index is free to disagree, and some should.
    pub const CONSERVATIVE: Weighting = Weighting {
        unattested: 0,
        seller_attested: 1,
        escrow_attested: 3,
    };

    /// Weight one review under this policy.
    pub fn weigh(&self, r: &Review) -> u8 {
        match r.attestation.as_ref().map(|a| a.attestor) {
            None => self.unattested,
            Some(Attestor::Seller) => self.seller_attested,
            Some(Attestor::Escrow) => self.escrow_attested,
        }
    }
}

/// Compute a **local** score over reviews this node holds, under a stated policy.
///
/// Local by construction: it takes the reviews you have and the weighting you chose. There is no
/// function here that produces a network-wide number, because there is no network-wide answer.
/// Returns `None` when total weight is zero — an index must render "no basis to judge" rather than
/// invent a neutral score.
pub fn local_score(reviews: &[Review], w: Weighting) -> Option<f32> {
    let (mut num, mut den) = (0u32, 0u32);
    for r in reviews {
        let weight = w.weigh(r) as u32;
        num += weight * r.score.min(5) as u32;
        den += weight;
    }
    if den == 0 {
        None
    } else {
        Some(num as f32 / den as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review(score: u8, attestor: Option<Attestor>) -> Review {
        Review {
            subject: Subject::Seller(IdentityKey(vec![1])),
            author: IdentityKey(vec![2]),
            score,
            body: String::new(),
            attestation: attestor.map(|a| PurchaseAttestation {
                attestor: a,
                issuer: IdentityKey(vec![3]),
                order: ContentAddress(vec![4]),
                at: Timestamp(0),
            }),
            at: Timestamp(0),
        }
    }

    /// Unattested reviews carry no weight under the conservative policy — this is the direct
    /// answer to OpenBazaar's self-published-review failure.
    #[test]
    fn unattested_reviews_do_not_move_a_conservative_score() {
        let rs = vec![review(5, None), review(1, Some(Attestor::Escrow))];
        assert_eq!(local_score(&rs, Weighting::CONSERVATIVE), Some(1.0));
    }

    /// Escrow attestation outweighs seller attestation, because a third party observed both sides.
    #[test]
    fn escrow_attestation_outweighs_seller_attestation() {
        let rs = vec![
            review(5, Some(Attestor::Seller)),
            review(1, Some(Attestor::Escrow)),
        ];
        let s = local_score(&rs, Weighting::CONSERVATIVE).unwrap();
        assert!(
            s < 3.0,
            "escrow-attested 1 should dominate seller-attested 5, got {s}"
        );
    }

    /// No basis to judge must be distinguishable from a neutral score. An index that renders 0 or
    /// 2.5 here is asserting something it does not know.
    #[test]
    fn no_weighted_reviews_yields_no_score_rather_than_a_neutral_one() {
        assert_eq!(
            local_score(&[review(5, None)], Weighting::CONSERVATIVE),
            None
        );
        assert_eq!(local_score(&[], Weighting::CONSERVATIVE), None);
    }

    /// Different indexes legitimately reach different answers from the same reviews. This test
    /// exists to assert that divergence is the design, not a bug to be fixed later.
    #[test]
    fn different_weightings_produce_different_scores_by_design() {
        let rs = vec![review(5, None), review(1, Some(Attestor::Escrow))];
        let permissive = Weighting {
            unattested: 1,
            seller_attested: 1,
            escrow_attested: 1,
        };
        assert_ne!(
            local_score(&rs, Weighting::CONSERVATIVE),
            local_score(&rs, permissive)
        );
    }
}
