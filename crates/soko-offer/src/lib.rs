//! # soko-offer — the four axes (TRACT §3–§5)
//!
//! Commerce specifications usually model retail well and everything else badly, then accrete a
//! module per category: a bookings module, a rentals module, a subscriptions module. TRACT instead
//! expresses **every** trade as one object with four orthogonal axes:
//!
//! > An offer commits to a [`Fulfilment`] against [`Consideration`], bounded by [`Availability`],
//! > for an [`Item`].
//!
//! Each axis is a small closed set. Combined, they cover shapes that mainstream platforms need
//! plugins for — and the test is whether real cases collapse without special-casing:
//!
//! | Trade | Item | Availability | Fulfilment | Consideration |
//! |---|---|---|---|---|
//! | tin of beans | `Product` | `Count` | `Ship` | `Fixed` |
//! | haircut | `Service` | `TimeSlots` | `PerformAtPlace` | `Fixed` |
//! | scaffolding hire | `Product` | `TimeSlots` | `ReturnRequired` | `DepositBalance` |
//! | restaurant table | `Capacity` | `CapacityPerInterval` | `PerformAtPlace` | `Fixed` |
//! | metered API | `Right` | `Unlimited` | `AccessGrant` | `Metered` |
//! | made-to-measure suit | `Product` | `MadeToOrder` | `Ship` | `DepositBalance` |
//! | B2B contract supply | `Product` | `Count` | `Ship` | `QuoteRequired` |
//!
//! None of those needs a category-specific code path.
//!
//! ## Why [`Fulfilment`] is load-bearing beyond logistics
//!
//! The fulfilment axis also determines **place of supply** for tax (§11.2), which is a different
//! thing from where either party lives. Admission to an event is generally taxed where the event
//! physically happens, so a seller in one country running an event in another owes tax in the
//! second — and only this axis knows that. See [`Fulfilment::place_of_supply_kind`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use soko_core::{ContentAddress, Country, Money, Timestamp};

/// What is being sold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Item {
    /// A single physical or digital product.
    Product(ContentAddress),
    /// One variant of a product group — that shirt, in medium, in blue (§2.4).
    VariantOfGroup {
        /// The group this variant belongs to.
        group: ContentAddress,
        /// The variant's own record.
        variant: ContentAddress,
    },
    /// Work performed by a party: a haircut, a consulting hour.
    Service(ContentAddress),
    /// A right or licence: a ticket, a font licence, a membership.
    Right(ContentAddress),
    /// Reservable capacity: a seat, a room-night, a table.
    Capacity(ContentAddress),
}

/// When, and how much, is on offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Availability {
    /// A discrete stock count.
    ///
    /// A seller MAY publish a band rather than an exact number — exact stock is commercially
    /// sensitive, and the protocol does not require disclosing it to browse.
    Count(StockSignal),
    /// Bookable slots, profiling RFC 5545 `VAVAILABILITY` rather than inventing a calendar format,
    /// so a seller can publish from calendar software they already run.
    TimeSlots {
        /// Opaque RFC 5545 payload describing the availability pattern.
        ical: String,
        /// Slot length in minutes.
        slot_minutes: u32,
    },
    /// N units available per interval — 40 seats per sitting.
    CapacityPerInterval {
        /// Units available in each interval.
        capacity: u32,
        /// Opaque RFC 5545 recurrence describing the intervals.
        ical: String,
    },
    /// No bound — a digital download.
    Unlimited,
    /// Built on demand. Distinguished from out-of-stock: the thing is available, it just takes
    /// time, and conflating the two is why lead-time products display badly on retail platforms.
    MadeToOrder {
        /// Working days from order to dispatch.
        lead_days: u16,
    },
}

/// A published stock level. Bands exist so a seller can be honest about availability without
/// publishing an exact number a competitor can watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StockSignal {
    /// An exact count, when the seller chooses to publish one.
    Exact(u32),
    /// Available, quantity undisclosed.
    InStock,
    /// Available but limited.
    Low,
    /// None available.
    OutOfStock,
}

/// How the thing reaches the buyer — and therefore where the supply happens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fulfilment {
    /// Carried to an address.
    Ship {
        /// Countries the seller will ship to.
        to: Vec<Country>,
    },
    /// Collected by the buyer.
    Collect {
        /// Where collection happens.
        at: PlaceRef,
    },
    /// A download, licence key or similar.
    DigitalGrant,
    /// Performed at a stated place — a haircut, an event. **This is the case that breaks any
    /// two-anchor tax model** (§11.2).
    PerformAtPlace {
        /// The venue.
        at: PlaceRef,
    },
    /// Performed remotely — consulting over video.
    PerformRemote,
    /// Access to a facility or service over a period — a gym membership.
    AccessGrant {
        /// Where the access applies, if it is physical.
        at: Option<PlaceRef>,
    },
    /// The buyer takes temporary custody and must return the item — a rental or hire.
    ReturnRequired {
        /// Where the item is collected and returned.
        at: PlaceRef,
        /// Days the buyer may hold it.
        term_days: u16,
    },
}

/// Which anchor determines place of supply for a given fulfilment mode (§11.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceOfSupplyKind {
    /// Where the goods are delivered.
    DeliveryDestination,
    /// Where the service is physically performed, or the item collected.
    StatedPlace,
    /// Where the buyer resides.
    BuyerResidence,
}

impl Fulfilment {
    /// Which jurisdictional anchor governs place of supply for this fulfilment mode.
    ///
    /// This mapping is the reason the fulfilment axis is not merely a logistics detail. Getting it
    /// wrong is the most common commerce-tax error, and it is not recoverable from the parties'
    /// countries alone.
    pub fn place_of_supply_kind(&self) -> PlaceOfSupplyKind {
        match self {
            Fulfilment::Ship { .. } => PlaceOfSupplyKind::DeliveryDestination,
            Fulfilment::Collect { .. }
            | Fulfilment::PerformAtPlace { .. }
            | Fulfilment::ReturnRequired { .. } => PlaceOfSupplyKind::StatedPlace,
            Fulfilment::AccessGrant { at: Some(_) } => PlaceOfSupplyKind::StatedPlace,
            Fulfilment::AccessGrant { at: None }
            | Fulfilment::DigitalGrant
            | Fulfilment::PerformRemote => PlaceOfSupplyKind::BuyerResidence,
        }
    }
}

/// A reference to a physical place.
///
/// Coarse by design: a country and a locality are enough for tax and routing. A full street address
/// is personal data and belongs in a sealed order, never in a published offer (§0.5.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceRef {
    /// Country of the place.
    pub country: Country,
    /// Locality or postal region — coarse, never a street address.
    pub locality: String,
}

/// What is paid, and on what schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Consideration {
    /// One price.
    Fixed(Money),
    /// Cheaper per unit above a threshold.
    Tiered(Vec<PriceTier>),
    /// Charged per period. Recurrence reuses RFC 5545 rather than a second schedule format.
    Recurring {
        /// Price per period.
        amount: Money,
        /// RFC 5545 recurrence rule.
        rrule: String,
    },
    /// Charged per unit consumed, after the fact.
    Metered {
        /// What is being counted — calls, kWh, gigabytes.
        dimension: String,
        /// Price per unit.
        unit_price: Money,
    },
    /// Part now, remainder later — the shape a deposit-taking trade actually has.
    DepositBalance {
        /// Payable at order.
        deposit: Money,
        /// Payable on delivery or completion.
        balance: Money,
    },
    /// No published price; the buyer requests a quote and the seller counter-signs one. This is
    /// also how B2B contract pricing works — a per-buyer price is an offer addressed to one key.
    QuoteRequired,
}

/// One band of a volume price.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceTier {
    /// Minimum quantity for this band.
    pub min_qty: u32,
    /// Unit price within the band.
    pub unit_price: Money,
}

/// One seller's commitment to supply, on stated terms.
///
/// An offer is always **one seller's claim**. The product record it references belongs to nobody
/// (§2.2) — which is what lets many sellers offer the same thing without a registrar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    /// What is on offer.
    pub item: Item,
    /// When and how much.
    pub availability: Availability,
    /// How it reaches the buyer.
    pub fulfilment: Fulfilment,
    /// What is paid.
    pub consideration: Consideration,
    /// Countries this offer may be accepted from. Empty means unrestricted, which a seller should
    /// rarely intend — see §11 on in-region responsible persons.
    pub sell_to: Vec<Country>,
    /// When this offer was published.
    pub published: Timestamp,
}

impl soko_core::public::Publishable for Offer {}

#[cfg(test)]
mod tests {
    use super::*;

    fn place() -> PlaceRef {
        PlaceRef {
            country: Country(*b"DE"),
            locality: "Berlin".into(),
        }
    }

    /// The forcing case from §11.2: an event held in one country, sold by a seller elsewhere, to a
    /// buyer somewhere else again. Place of supply is the venue, and neither party's country
    /// reveals it.
    #[test]
    fn event_place_of_supply_is_the_venue_not_either_party() {
        let f = Fulfilment::PerformAtPlace { at: place() };
        assert_eq!(f.place_of_supply_kind(), PlaceOfSupplyKind::StatedPlace);
    }

    #[test]
    fn shipped_goods_follow_the_destination() {
        let f = Fulfilment::Ship {
            to: vec![Country(*b"ZA")],
        };
        assert_eq!(
            f.place_of_supply_kind(),
            PlaceOfSupplyKind::DeliveryDestination
        );
    }

    /// Remote performance and digital grants follow the buyer, not the seller — the case that
    /// catches out sellers who assume their own establishment governs.
    #[test]
    fn remote_and_digital_follow_the_buyer() {
        assert_eq!(
            Fulfilment::PerformRemote.place_of_supply_kind(),
            PlaceOfSupplyKind::BuyerResidence
        );
        assert_eq!(
            Fulfilment::DigitalGrant.place_of_supply_kind(),
            PlaceOfSupplyKind::BuyerResidence
        );
    }

    /// An access grant is physical or not depending on whether it names a place; both branches
    /// must resolve, because "gym membership" and "streaming subscription" are the same variant.
    #[test]
    fn access_grant_splits_on_whether_it_names_a_place() {
        assert_eq!(
            Fulfilment::AccessGrant { at: Some(place()) }.place_of_supply_kind(),
            PlaceOfSupplyKind::StatedPlace
        );
        assert_eq!(
            Fulfilment::AccessGrant { at: None }.place_of_supply_kind(),
            PlaceOfSupplyKind::BuyerResidence
        );
    }
}
