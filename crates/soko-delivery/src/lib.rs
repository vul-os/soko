//! # soko-delivery — rate cards, consignments, distributors, routing (TRACT §8)
//!
//! Delivery in TRACT is **computed, not brokered**. Couriers and distributors publish signed rate
//! cards as public objects; the buyer's node computes prices locally instead of calling a quote
//! API. That has consequences beyond convenience:
//!
//! - no rate limit, no API key, no per-quote cost;
//! - no third party learns what is being bought, or from whom;
//! - a peer with a van and a multinational carrier are the same object type, so there is no
//!   second-class tier;
//! - because the substrate deduplicates identical public blobs, one carrier's rate card is stored
//!   once across the whole network no matter how many sellers reference it.
//!
//! ## The honest limits
//!
//! - **Republishing rates may be restricted.** Some carriers limit redistribution of negotiated
//!   rates in their API terms. Published list rates, and a seller's own rates they choose to
//!   publish, are fine; republishing a carrier's confidential rates is the publisher's compliance
//!   problem. An offer may declare [`RateSource::LiveQuoteRequired`] instead.
//! - **Transit estimates are estimates.** For consolidation, `wait_days` — when the slowest
//!   seller's parcel actually arrives — is the least reliable input in the whole computation, and
//!   it must be surfaced as an estimate rather than a promise.
//! - **Custody attestations prove transfer, not recoverability.** A distributor holds goods
//!   belonging to strangers. Signed handoff makes custody provable; it does not make loss
//!   recoverable. That is §9's problem, or nobody's.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use soko_core::{ContentAddress, Country, IdentityKey, Money, Timestamp};

/// Where a leg's price comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RateSource {
    /// Computed locally from a published rate card. The default, and the point of the design.
    PublishedCard(ContentAddress),
    /// The carrier requires a live quote — either because its terms forbid republishing rates, or
    /// because the price genuinely is not a function of published inputs.
    LiveQuoteRequired,
}

/// A published price list from which a leg cost is computed locally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateCard {
    /// Who published it — a national carrier or a neighbour with a bicycle, identically typed.
    pub carrier: IdentityKey,
    /// Countries served.
    pub serves: Vec<Country>,
    /// Zone bands, resolved from origin and destination.
    pub zones: Vec<Zone>,
    /// Divisor for volumetric weight. Billable weight is `max(actual, L*W*H / divisor)`.
    pub dim_divisor: u32,
    /// Percentage surcharge applied to the base rate (fuel and similar).
    pub surcharge_pct: u16,
    /// Categories this carrier will not take — hazardous goods, live animals, age-restricted.
    pub excluded_categories: Vec<String>,
    /// When this card was published. Cards drift; a stale one must not be silently replayed.
    pub published: Timestamp,
}

impl soko_core::public::Publishable for RateCard {}

/// One price band within a rate card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Zone {
    /// Zone identifier, as the carrier numbers it.
    pub id: u16,
    /// Weight brackets and their prices.
    pub brackets: Vec<WeightBracket>,
    /// Estimated transit time. An estimate, never a guarantee.
    pub transit_days: u8,
}

/// A weight band and its price.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeightBracket {
    /// Upper bound of the bracket, in grams.
    pub max_grams: u32,
    /// Price for a parcel in this bracket.
    pub price: Money,
}

/// Physical dimensions and weight of a parcel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parcel {
    /// Length in millimetres.
    pub length_mm: u32,
    /// Width in millimetres.
    pub width_mm: u32,
    /// Height in millimetres.
    pub height_mm: u32,
    /// Actual weight in grams.
    pub weight_grams: u32,
}

impl Parcel {
    /// Billable weight: the greater of actual and volumetric weight.
    ///
    /// Every major carrier prices this way, and a router that used actual weight alone would
    /// under-quote large light parcels — the buyer discovers the shortfall at the counter, which
    /// is exactly the surprise this design exists to remove.
    pub fn billable_grams(&self, dim_divisor: u32) -> u32 {
        if dim_divisor == 0 {
            return self.weight_grams;
        }
        let volumetric = (self.length_mm as u64 * self.width_mm as u64 * self.height_mm as u64)
            / (dim_divisor as u64);
        self.weight_grams.max(volumetric as u32)
    }
}

/// One movement of goods between two places, priced by one rate card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leg {
    /// Who carries it.
    pub carrier: IdentityKey,
    /// Where the price came from.
    pub source: RateSource,
    /// Computed or quoted cost.
    pub cost: Money,
    /// Estimated days in transit.
    pub transit_days: u8,
}

/// Goods in someone else's custody — a courier leg or a distributor hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Consignment {
    /// The order this consignment belongs to.
    pub order: ContentAddress,
    /// Who currently holds the goods.
    pub custodian: IdentityKey,
    /// Signed custody transfers, oldest first. Proves the chain; does not insure it.
    pub handoffs: Vec<CustodyHandoff>,
}

/// A signed transfer of physical custody.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustodyHandoff {
    /// Party relinquishing custody.
    pub from: IdentityKey,
    /// Party accepting custody.
    pub to: IdentityKey,
    /// When the transfer happened.
    pub at: Timestamp,
}

/// A distributor's published capacity — the consolidation role.
///
/// A distributor receives consignments from several sellers, holds them, combines them, and either
/// hands off to a courier or delivers locally. Economically this is the freight-forwarder pattern
/// that already exists worldwide; what TRACT adds is that entry is permissionless and the terms are
/// a published object rather than a bilaterally negotiated contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributorCapacity {
    /// Who is offering space.
    pub distributor: IdentityKey,
    /// Where the space is.
    pub country: Country,
    /// Coarse locality — never a street address (§0.5.1).
    pub locality: String,
    /// Charge per item per day held.
    pub storage_per_day: Money,
    /// Charge for receiving, combining and dispatching one consolidated shipment.
    pub handling_fee: Money,
    /// Items that can be held at once.
    pub slots: u32,
    /// Categories refused — a distributor's own policy, not a protocol rule.
    pub excluded_categories: Vec<String>,
}

impl soko_core::public::Publishable for DistributorCapacity {}

/// A candidate way to get every line of a cart to the buyer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteOption {
    /// The shape of this route.
    pub shape: RouteShape,
    /// Legs in order.
    pub legs: Vec<Leg>,
    /// Days the slowest item is expected to wait at a hub. The least reliable input here.
    pub wait_days: u16,
    /// Storage and handling charged by the hub, if any.
    pub hub_fees: Money,
}

/// The three consolidation shapes worth comparing.
///
/// The candidate set is deliberately small — a handful of hubs, not a fleet-wide optimisation — so
/// it can be evaluated exhaustively on a buyer's own device without an optimiser or a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteShape {
    /// Every seller ships straight to the buyer. Fastest per item, worst on total cost.
    Direct,
    /// Sellers ship to a distributor near the buyer, who combines. Cheapest last mile; the buyer
    /// waits for the slowest seller.
    HubNearBuyer,
    /// Sellers that are geographically close consolidate first, then one long leg. Wins when the
    /// long haul dominates.
    HubNearSellers,
}

impl RouteOption {
    /// Total cost of this route: every leg, plus storage for the wait, plus handling.
    ///
    /// Returns [`soko_core::Error::CurrencyMismatch`] rather than coercing across currencies. A
    /// silently converted total is a wrong total that looks right.
    pub fn total(&self) -> Result<Money, soko_core::Error> {
        let mut sum = self.hub_fees;
        for leg in &self.legs {
            if leg.cost.currency != sum.currency {
                return Err(soko_core::Error::CurrencyMismatch);
            }
            sum.minor_units += leg.cost.minor_units;
        }
        Ok(sum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZAR: soko_core::Currency = soko_core::Currency(*b"ZAR");
    fn money(n: i64) -> Money {
        Money {
            minor_units: n,
            currency: ZAR,
        }
    }

    /// A large light parcel must price on volumetric weight, or the quote under-reads reality.
    #[test]
    fn volumetric_weight_wins_for_large_light_parcels() {
        let p = Parcel {
            length_mm: 600,
            width_mm: 400,
            height_mm: 400,
            weight_grams: 2_000,
        };
        // 600*400*400 / 5000 = 19_200 volumetric vs 2_000 actual
        assert_eq!(p.billable_grams(5000), 19_200);
    }

    /// A small dense parcel prices on actual weight.
    #[test]
    fn actual_weight_wins_for_small_dense_parcels() {
        let p = Parcel {
            length_mm: 100,
            width_mm: 100,
            height_mm: 100,
            weight_grams: 5_000,
        };
        assert_eq!(p.billable_grams(5000), 5_000);
    }

    /// A zero divisor must not divide by zero; it means "this carrier does not apply volumetric".
    #[test]
    fn zero_divisor_falls_back_to_actual_weight() {
        let p = Parcel {
            length_mm: 600,
            width_mm: 400,
            height_mm: 400,
            weight_grams: 2_000,
        };
        assert_eq!(p.billable_grams(0), 2_000);
    }

    #[test]
    fn route_total_sums_legs_and_hub_fees() {
        let leg = |c| Leg {
            carrier: IdentityKey(vec![1]),
            source: RateSource::PublishedCard(ContentAddress(vec![2])),
            cost: money(c),
            transit_days: 3,
        };
        let r = RouteOption {
            shape: RouteShape::HubNearBuyer,
            legs: vec![leg(4_500), leg(3_200)],
            wait_days: 4,
            hub_fees: money(1_800),
        };
        assert_eq!(r.total().unwrap().minor_units, 9_500);
    }

    /// Mixed currencies must fail loudly. A silently converted total is a wrong total that looks
    /// right, and it would be carried into a signed order.
    #[test]
    fn mixed_currency_route_refuses_to_total() {
        let r = RouteOption {
            shape: RouteShape::Direct,
            legs: vec![Leg {
                carrier: IdentityKey(vec![1]),
                source: RateSource::LiveQuoteRequired,
                cost: Money {
                    minor_units: 100,
                    currency: soko_core::Currency(*b"EUR"),
                },
                transit_days: 2,
            }],
            wait_days: 0,
            hub_fees: money(0),
        };
        assert!(matches!(r.total(), Err(soko_core::Error::CurrencyMismatch)));
    }
}
