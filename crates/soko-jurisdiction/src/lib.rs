//! # soko-jurisdiction — the four anchors (TRACT §11)
//!
//! The most common commerce-tax error is conflating where the parties are with where the supply
//! happens. This crate keeps **four** anchors separate, derived from four different places, so the
//! mistake is not expressible.
//!
//! | Anchor | Derived from | Governs |
//! |---|---|---|
//! | seller establishment | seller identity | licensing, seller-side registration |
//! | buyer residence | buyer disclosure at order | consumer protection (generally non-waivable) |
//! | **place of supply** | **the fulfilment axis** | VAT/GST, especially services and events |
//! | delivery destination | the shipping leg | customs, duty, product-safety regimes |
//!
//! The forcing case: an event held in one country, sold by a seller in another, to a buyer in a
//! third. Admission to events is generally taxed where the event physically takes place, so
//! knowing both parties' countries tells you nothing useful.
//!
//! ## Responsibility follows the money
//!
//! Every regime asks "who is responsible?", and a protocol with no operator has no default answer.
//! [`ResponsibleParties`] makes the answer explicit and signed, so compliance is a field rather
//! than a hole. A self-hosted seller taking direct payment is an ordinary distance seller; a
//! gateway that settled is the facilitator, and the order says so.
//!
//! **This is not legal advice, and the crate cannot make a deployment compliant.** What it
//! guarantees is that the facts a regulator asks for are present and attributable.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use soko_core::{Country, IdentityKey};
use soko_offer::{Fulfilment, PlaceOfSupplyKind};

/// The four jurisdictional anchors of one trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anchors {
    /// Where the seller is established.
    pub seller_establishment: Country,
    /// Where the buyer resides.
    pub buyer_residence: Country,
    /// Where the supply happens.
    pub place_of_supply: Country,
    /// Where goods are delivered, if any move.
    pub delivery_destination: Option<Country>,
}

/// Resolve place of supply from the fulfilment axis and the parties.
///
/// `stated_place` is the venue or collection point, when the fulfilment mode names one. Returns
/// `None` when the mode requires a stated place and none was supplied — a missing venue is a
/// malformed offer, not something to guess at by falling back to a party's country.
pub fn place_of_supply(
    f: &Fulfilment,
    buyer_residence: Country,
    delivery_destination: Option<Country>,
    stated_place: Option<Country>,
) -> Option<Country> {
    match f.place_of_supply_kind() {
        PlaceOfSupplyKind::DeliveryDestination => delivery_destination,
        PlaceOfSupplyKind::StatedPlace => stated_place,
        PlaceOfSupplyKind::BuyerResidence => Some(buyer_residence),
    }
}

/// Who is answerable, and for what.
///
/// The `facilitator` field is the marketplace-facilitator hook: present when a gateway settled the
/// payment, absent when the seller took it directly. Which of those is true changes who owes tax
/// collection and reporting duties in several regimes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponsibleParties {
    /// Who contracts with the buyer.
    pub seller_of_record: IdentityKey,
    /// The gateway, if it settled the payment.
    pub facilitator: Option<IdentityKey>,
    /// For cross-border movement.
    pub importer_of_record: Option<IdentityKey>,
    /// Required in-region by some product-safety regimes. An offer into such a region without one
    /// is invalid rather than merely risky.
    pub responsible_person: Option<InRegionRepresentative>,
}

/// A representative established inside a region that requires one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InRegionRepresentative {
    /// The region this representative covers.
    pub region: Country,
    /// Their identity.
    pub who: IdentityKey,
}

/// Why an offer may not lawfully be made into a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferBlocked {
    /// The region requires an in-region responsible person and none was named.
    MissingResponsiblePerson,
    /// The seller did not list this region as one it sells to.
    NotOffered,
}

impl ResponsibleParties {
    /// Whether an offer may be made into `region`.
    ///
    /// Expressed as a check rather than left to policy, so the constraint surfaces when the offer
    /// is constructed instead of when a regulator writes.
    pub fn may_offer_into(
        &self,
        region: Country,
        sell_to: &[Country],
        requires_representative: bool,
    ) -> Result<(), OfferBlocked> {
        if !sell_to.contains(&region) {
            return Err(OfferBlocked::NotOffered);
        }
        if requires_representative {
            let ok = self
                .responsible_person
                .as_ref()
                .is_some_and(|r| r.region == region);
            if !ok {
                return Err(OfferBlocked::MissingResponsiblePerson);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soko_offer::PlaceRef;

    const ZA: Country = Country(*b"ZA");
    const DE: Country = Country(*b"DE");
    const NZ: Country = Country(*b"NZ");

    /// The case the whole four-anchor model exists for: seller in one country, buyer in another,
    /// event in a third. Place of supply is the venue and nothing else.
    #[test]
    fn event_abroad_is_taxed_at_the_venue() {
        let f = Fulfilment::PerformAtPlace {
            at: PlaceRef {
                country: DE,
                locality: "Berlin".into(),
            },
        };
        assert_eq!(place_of_supply(&f, NZ, None, Some(DE)), Some(DE));
    }

    /// Shipped goods follow the destination, not the buyer's residence — a buyer may ship to
    /// somewhere they do not live.
    #[test]
    fn shipped_goods_follow_the_destination_not_the_buyer() {
        let f = Fulfilment::Ship { to: vec![DE] };
        assert_eq!(place_of_supply(&f, NZ, Some(DE), None), Some(DE));
    }

    #[test]
    fn remote_service_follows_the_buyer() {
        assert_eq!(
            place_of_supply(&Fulfilment::PerformRemote, NZ, None, None),
            Some(NZ)
        );
    }

    /// A venue-based mode with no venue must not silently fall back to a party's country. Guessing
    /// here produces a plausible, wrong tax treatment.
    #[test]
    fn missing_stated_place_refuses_to_guess() {
        let f = Fulfilment::PerformAtPlace {
            at: PlaceRef {
                country: DE,
                locality: "Berlin".into(),
            },
        };
        assert_eq!(place_of_supply(&f, NZ, Some(NZ), None), None);
    }

    #[test]
    fn region_requiring_a_representative_blocks_an_offer_without_one() {
        let r = ResponsibleParties {
            seller_of_record: IdentityKey(vec![1]),
            facilitator: None,
            importer_of_record: None,
            responsible_person: None,
        };
        assert_eq!(
            r.may_offer_into(DE, &[DE], true),
            Err(OfferBlocked::MissingResponsiblePerson)
        );
        assert_eq!(
            r.may_offer_into(ZA, &[DE], false),
            Err(OfferBlocked::NotOffered)
        );
        assert_eq!(r.may_offer_into(ZA, &[ZA], false), Ok(()));
    }

    /// A representative for the wrong region does not satisfy the requirement.
    #[test]
    fn representative_must_cover_the_region_in_question() {
        let r = ResponsibleParties {
            seller_of_record: IdentityKey(vec![1]),
            facilitator: None,
            importer_of_record: None,
            responsible_person: Some(InRegionRepresentative {
                region: NZ,
                who: IdentityKey(vec![2]),
            }),
        };
        assert_eq!(
            r.may_offer_into(DE, &[DE], true),
            Err(OfferBlocked::MissingResponsiblePerson)
        );
    }
}
