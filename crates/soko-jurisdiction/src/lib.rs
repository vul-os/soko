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
use soko_offer::Fulfilment;

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

/// Why a place of supply could not be resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SupplyError {
    /// A shipped offer needs a chosen destination and none was supplied.
    #[error("shipped fulfilment requires a delivery destination")]
    NoDestination,
    /// The chosen destination is not one the offer ships to. Resolving anyway would compute tax
    /// for a delivery the seller never agreed to make.
    #[error("delivery destination is not among the territories this offer ships to")]
    DestinationNotOffered,
}

/// Resolve place of supply from the fulfilment axis.
///
/// **The venue is read out of the [`Fulfilment`] itself, never accepted as a separate argument.**
/// An earlier signature took a `stated_place: Option<Country>` alongside the fulfilment, which
/// meant a caller could hand it `PerformAtPlace { at: Berlin }` together with `Some(ZA)` and get
/// back a confident, plausible, wrong `ZA`. That is precisely the error §11.2 claims the four
/// anchors make *not expressible* — and it was expressible, because nothing forced the argument to
/// agree with the object. Deriving it here makes the mismatch unrepresentable rather than merely
/// discouraged.
///
/// `delivery_destination` survives as an argument only for [`Fulfilment::Ship`], where the buyer
/// genuinely chooses among the territories the offer serves — and it is checked against that list
/// rather than trusted.
pub fn place_of_supply(
    f: &Fulfilment,
    buyer_residence: Country,
    delivery_destination: Option<Country>,
) -> Result<Country, SupplyError> {
    match f {
        // The buyer picks a destination, but only from what the seller offered.
        Fulfilment::Ship { to } => {
            let d = delivery_destination.ok_or(SupplyError::NoDestination)?;
            if to.is_empty() || to.contains(&d) {
                Ok(d)
            } else {
                Err(SupplyError::DestinationNotOffered)
            }
        }
        // The place is part of the offer. There is no argument that can contradict it.
        Fulfilment::Collect { at }
        | Fulfilment::PerformAtPlace { at }
        | Fulfilment::ReturnRequired { at, .. } => Ok(at.country),
        Fulfilment::AccessGrant { at: Some(at) } => Ok(at.country),
        // Nothing physical happens anywhere in particular; the buyer's residence governs.
        Fulfilment::AccessGrant { at: None }
        | Fulfilment::DigitalGrant
        | Fulfilment::PerformRemote => Ok(buyer_residence),
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
        assert_eq!(place_of_supply(&f, NZ, None), Ok(DE));
    }

    /// The regression that motivated deriving the venue instead of accepting it. Previously a
    /// caller could pass a country that disagreed with the fulfilment object and get it back —
    /// a confident, plausible, wrong answer. There is now no argument capable of expressing it.
    #[test]
    fn no_argument_can_contradict_the_venue_in_the_offer() {
        let f = Fulfilment::PerformAtPlace {
            at: PlaceRef {
                country: DE,
                locality: "Berlin".into(),
            },
        };
        // buyer in NZ, a delivery destination of ZA — neither can move a German event
        assert_eq!(place_of_supply(&f, NZ, Some(ZA)), Ok(DE));
    }

    /// Shipped goods follow the destination, not the buyer's residence — a buyer may ship to
    /// somewhere they do not live.
    #[test]
    fn shipped_goods_follow_the_destination_not_the_buyer() {
        let f = Fulfilment::Ship { to: vec![DE, NZ] };
        assert_eq!(place_of_supply(&f, NZ, Some(DE)), Ok(DE));
    }

    /// A destination the seller never offered is refused rather than resolved. Computing tax for
    /// a delivery the seller did not agree to make would be confidently wrong in both directions.
    #[test]
    fn destination_outside_the_offer_is_refused() {
        let f = Fulfilment::Ship { to: vec![NZ] };
        assert_eq!(
            place_of_supply(&f, NZ, Some(DE)),
            Err(SupplyError::DestinationNotOffered)
        );
    }

    #[test]
    fn remote_service_follows_the_buyer() {
        assert_eq!(
            place_of_supply(&Fulfilment::PerformRemote, NZ, None),
            Ok(NZ)
        );
    }

    /// A shipped offer with no chosen destination refuses rather than falling back to a party's
    /// country. Guessing here produces a plausible, wrong tax treatment.
    #[test]
    fn shipping_without_a_destination_refuses_to_guess() {
        let f = Fulfilment::Ship { to: vec![NZ] };
        assert_eq!(
            place_of_supply(&f, NZ, None),
            Err(SupplyError::NoDestination)
        );
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
