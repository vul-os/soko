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

/// Why a leg price could not be computed from a published card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum QuoteError {
    /// The card declares a divisor no real carrier uses (§19.5 `MIN_DIM_DIVISOR`).
    #[error("rate card is unusable: implausible dimensional divisor")]
    UnusableCard,
    /// No zone on this card matches the requested route.
    #[error("no zone on this card covers that route")]
    NoZone,
    /// The parcel is heavier than the heaviest bracket the zone publishes. Refused rather than
    /// extrapolated: a price invented past the end of a published table is not the carrier's price.
    #[error("parcel exceeds the heaviest published weight bracket")]
    OverWeight,
}

impl Zone {
    /// The bracket that prices a parcel of `billable_grams`, if this zone publishes one.
    ///
    /// The smallest bracket whose `max_grams` is greater than or equal to the billable weight —
    /// the boundary is inclusive, so a parcel of exactly 1000g takes the 1000g bracket rather than
    /// the next one up. Brackets need not arrive sorted; a carrier's published table is a claim
    /// and this does not assume it was tidy.
    ///
    /// `None` means the parcel is off the top of the table, which is refused rather than
    /// extrapolated (see [`QuoteError::OverWeight`]).
    pub fn select_bracket(&self, billable_grams: u32) -> Option<&WeightBracket> {
        self.brackets
            .iter()
            .filter(|b| b.max_grams >= billable_grams)
            .min_by_key(|b| b.max_grams)
    }
}

impl RateCard {
    /// Compute a leg's price locally, from this published card.
    ///
    /// This is the operation §8 is built on and it did not exist until the conformance vectors
    /// asked for it: the card was publishable data with nothing that could turn it into a price.
    /// A rate card nobody can compute against is a rate card that has to be phoned in, which is
    /// the quote API the design exists to avoid.
    ///
    /// The surcharge is applied to the bracket price and rounded down, because rounding a price
    /// **up** by default takes money from the buyer for the implementation's convenience.
    pub fn quote(&self, zone_id: u16, parcel: &Parcel) -> Result<(Money, u8), QuoteError> {
        if !self.is_usable() {
            return Err(QuoteError::UnusableCard);
        }
        let zone = self
            .zones
            .iter()
            .find(|z| z.id == zone_id)
            .ok_or(QuoteError::NoZone)?;
        let billable = parcel.billable_grams(self.dim_divisor);
        let bracket = zone
            .select_bracket(billable)
            .ok_or(QuoteError::OverWeight)?;

        let base = bracket.price.minor_units;
        let surcharge = base.saturating_mul(i64::from(self.surcharge_pct)) / 100;
        Ok((
            Money {
                minor_units: base.saturating_add(surcharge),
                currency: bracket.price.currency,
            },
            zone.transit_days,
        ))
    }
}

/// The smallest volumetric divisor any real carrier publishes is in the thousands (5000 and 6000
/// are the common ones). A tiny divisor makes volumetric weight enormous, so a card carrying one
/// is either a mistake or an attempt to inflate every quote computed from it.
pub const MIN_DIM_DIVISOR: u32 = 1_000;

impl RateCard {
    /// Whether this card is safe to compute prices from.
    ///
    /// A card is a **claim by its publisher** (§8.4) and arrives from a stranger, so it is checked
    /// before use rather than trusted. `dim_divisor == 0` is permitted and means "this carrier does
    /// not apply volumetric weight"; anything between 1 and [`MIN_DIM_DIVISOR`] is rejected,
    /// because it is not a divisor any carrier uses and it inflates every parcel it touches.
    pub fn is_usable(&self) -> bool {
        self.dim_divisor == 0 || self.dim_divisor >= MIN_DIM_DIVISOR
    }
}

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
        let volumetric =
            (u64::from(self.length_mm) * u64::from(self.width_mm) * u64::from(self.height_mm))
                / u64::from(dim_divisor);
        // Saturate rather than truncate. An `as u32` here wrapped: a 1700mm cube with a divisor of
        // 1 reported about an eighth of its real volumetric weight, which is the under-quote this
        // function's whole purpose is to prevent. A saturated value is absurd and visible; a
        // wrapped one is plausible and silent.
        let volumetric = u32::try_from(volumetric).unwrap_or(u32::MAX);
        self.weight_grams.max(volumetric)
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
            // Checked, not wrapping. Release builds do not enable overflow checks, so `+=` here
            // silently produced a large negative total once the sum passed i64::MAX — and that
            // total would have gone straight into a signed order, where nothing downstream can
            // correct it. The currency guard above was already refusing the *less* dangerous case.
            sum.minor_units = sum
                .minor_units
                .checked_add(leg.cost.minor_units)
                .ok_or(soko_core::Error::Overflow)?;
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

    /// A truncating cast made a huge parcel look light: 1700mm cubed with a divisor of 1 reported
    /// roughly an eighth of its real volumetric weight. Saturating is absurd and visible; wrapping
    /// was plausible and silent, which is the failure mode this function exists to prevent.
    #[test]
    fn oversized_volumetric_saturates_rather_than_wrapping() {
        let p = Parcel {
            length_mm: 1700,
            width_mm: 1700,
            height_mm: 1700,
            weight_grams: 1_000,
        };
        assert_eq!(p.billable_grams(1), u32::MAX);
    }

    /// A card with an absurd divisor is refused before it can inflate a quote.
    #[test]
    fn rate_cards_with_implausible_divisors_are_rejected() {
        let card = |d| RateCard {
            carrier: IdentityKey(vec![1]),
            serves: vec![],
            zones: vec![],
            dim_divisor: d,
            surcharge_pct: 0,
            excluded_categories: vec![],
            published: soko_core::Timestamp(0),
        };
        assert!(card(5000).is_usable(), "the common real-world divisor");
        assert!(
            card(0).is_usable(),
            "zero means this carrier does not apply volumetric weight"
        );
        assert!(
            !card(1).is_usable(),
            "no carrier uses 1; it inflates every parcel"
        );
        assert!(!card(999).is_usable());
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

    fn card(divisor: u32, pct: u16) -> RateCard {
        RateCard {
            carrier: IdentityKey(vec![1]),
            serves: vec![],
            zones: vec![Zone {
                id: 1,
                // deliberately out of order — a published table is a claim, not a promise of tidiness
                brackets: vec![
                    WeightBracket {
                        max_grams: 20_000,
                        price: money(1_800),
                    },
                    WeightBracket {
                        max_grams: 1_000,
                        price: money(500),
                    },
                    WeightBracket {
                        max_grams: 5_000,
                        price: money(900),
                    },
                ],
                transit_days: 4,
            }],
            dim_divisor: divisor,
            surcharge_pct: pct,
            excluded_categories: vec![],
            published: soko_core::Timestamp(0),
        }
    }

    fn parcel(grams: u32) -> Parcel {
        Parcel {
            length_mm: 10,
            width_mm: 10,
            height_mm: 10,
            weight_grams: grams,
        }
    }

    /// The boundary is inclusive: exactly 1000g takes the 1000g bracket, not the next one up.
    /// An exclusive comparison here would overcharge every parcel that lands exactly on a band
    /// edge, which is the most common weight for anything sold by the kilo.
    #[test]
    fn bracket_selection_is_inclusive_at_the_boundary() {
        let z = &card(5000, 0).zones[0];
        assert_eq!(z.select_bracket(1_000).unwrap().max_grams, 1_000);
        assert_eq!(z.select_bracket(1_001).unwrap().max_grams, 5_000);
        assert_eq!(z.select_bracket(1).unwrap().max_grams, 1_000);
    }

    /// Off the top of the published table is refused, never extrapolated. A price invented past
    /// the end of a carrier's own table is not that carrier's price.
    #[test]
    fn overweight_parcels_are_refused_not_extrapolated() {
        let c = card(5000, 0);
        assert_eq!(c.quote(1, &parcel(20_001)), Err(QuoteError::OverWeight));
        assert!(c.quote(1, &parcel(20_000)).is_ok());
    }

    #[test]
    fn quote_applies_the_surcharge_and_returns_transit_days() {
        let c = card(5000, 10);
        let (price, days) = c.quote(1, &parcel(3_000)).unwrap();
        // bracket 5000 -> 900, +10% = 990
        assert_eq!(price.minor_units, 990);
        assert_eq!(days, 4);
    }

    /// A card with an implausible divisor cannot be quoted against at all — it is refused at the
    /// point of use, not merely flagged, because a usable-looking price computed from it would
    /// silently inflate every parcel.
    #[test]
    fn unusable_cards_cannot_be_quoted_against() {
        assert_eq!(
            card(1, 0).quote(1, &parcel(500)),
            Err(QuoteError::UnusableCard)
        );
    }

    #[test]
    fn unknown_zone_is_refused() {
        assert_eq!(
            card(5000, 0).quote(99, &parcel(500)),
            Err(QuoteError::NoZone)
        );
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

    /// A total that overflows must refuse, not wrap. Release builds have overflow checks off, so
    /// this wrapped to a large negative number before — a wrong total that looks like a real one,
    /// carried into a signed order where nothing downstream can fix it.
    #[test]
    fn overflowing_total_refuses_rather_than_wrapping() {
        let leg = |c| Leg {
            carrier: IdentityKey(vec![1]),
            source: RateSource::LiveQuoteRequired,
            cost: Money {
                minor_units: c,
                currency: ZAR,
            },
            transit_days: 1,
        };
        let r = RouteOption {
            shape: RouteShape::Direct,
            legs: vec![leg(i64::MAX - 100), leg(200)],
            wait_days: 0,
            hub_fees: money(0),
        };
        assert!(matches!(r.total(), Err(soko_core::Error::Overflow)));
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
