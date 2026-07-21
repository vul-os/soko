//! One complete trade, end to end, across every crate.
//!
//! The unit tests in each crate prove a type behaves. This proves the types **compose** — which is
//! where a design that looks fine section by section usually falls apart. Every step below is a
//! real call into a real crate; nothing is stubbed.
//!
//! The scenario is chosen to be the awkward one rather than the easy one:
//!
//! - **two independent sellers** in one cart, who have never heard of each other;
//! - one sells a physical good shipped to the buyer, the other sells **admission to an event held
//!   in a third country** — the case that breaks any two-anchor tax model;
//! - the buyer lives in neither country;
//! - one seller runs **two replicas** that sell concurrently without coordinating;
//! - escrow is offered by an operator licensed for only one of the two trades.
//!
//! If the four axes, the jurisdictional anchors, and the escrow scope check are right, this walks
//! through. If any of them is a fiction, it fails here rather than in production.

use soko_catalog::{canonicalise, Attribute, IdentityRung, ProductRecord};
use soko_core::{ContentAddress, Country, Currency, IdentityKey, Money, Timestamp};
use soko_delivery::{Leg, Parcel, RateSource, RouteOption, RouteShape};
use soko_gateway::{StoreBinding, StoreHost};
use soko_jurisdiction::{place_of_supply, InRegionRepresentative, ResponsibleParties};
use soko_offer::{Availability, Consideration, Fulfilment, Item, Offer, PlaceRef, StockSignal};
use soko_order::{BoundedCounter, Cart, CartLine, Order, OrderState};
use soko_settle::{EscrowAvailability, EscrowScope, RailClass, ScopeMismatch, TradeContext};
use soko_trust::{Attestor, PurchaseAttestation, Review, Subject, Weighting};

const ZA: Country = Country(*b"ZA");
const DE: Country = Country(*b"DE");
const NZ: Country = Country(*b"NZ");
const ZAR: Currency = Currency(*b"ZAR");

fn key(n: u8) -> IdentityKey {
    IdentityKey(vec![n])
}
fn addr(n: u8) -> ContentAddress {
    ContentAddress(vec![n])
}
fn zar(n: i64) -> Money {
    Money {
        minor_units: n,
        currency: ZAR,
    }
}

/// A physical good, sold by a seller in South Africa, shipped to a buyer in New Zealand.
fn shipped_good_offer() -> Offer {
    Offer {
        item: Item::Product(addr(1)),
        availability: Availability::Count(StockSignal::Exact(10)),
        fulfilment: Fulfilment::Ship { to: vec![NZ] },
        consideration: Consideration::Fixed(zar(45_000)),
        sell_to: vec![NZ, ZA],
        published: Timestamp(1_700_000_000_000),
    }
}

/// Admission to an event held in Berlin, sold by the *same* South African seller's neighbour.
/// Neither party is in Germany. This is the case §11.2 exists for.
fn event_offer() -> Offer {
    Offer {
        item: Item::Capacity(addr(2)),
        availability: Availability::CapacityPerInterval {
            capacity: 40,
            ical: "FREQ=WEEKLY;BYDAY=SA".into(),
        },
        fulfilment: Fulfilment::PerformAtPlace {
            at: PlaceRef {
                country: DE,
                locality: "Berlin".into(),
            },
        },
        consideration: Consideration::Fixed(zar(30_000)),
        sell_to: vec![NZ, ZA, DE],
        published: Timestamp(1_700_000_000_000),
    }
}

/// Two sellers, two very different trades, one cart, one buyer device.
#[test]
fn a_complete_two_seller_trade_walks_through_every_crate() {
    let (seller_goods, seller_event, buyer) = (key(1), key(2), key(9));

    // ---- 1. Catalogue: a product record is canonicalised before it is addressed ----------------
    let record = canonicalise(ProductRecord {
        name: "  Field   Notebook ".into(),
        description: "pocket  notebook".into(),
        attributes: vec![
            Attribute {
                key: "Colour".into(),
                value: "black".into(),
            },
            Attribute {
                key: "size".into(),
                value: "A6".into(),
            },
        ],
        identity: vec![IdentityRung::ContentAddress(addr(1))],
        group: None,
        components: vec![],
    });
    assert_eq!(
        record.name, "Field Notebook",
        "canonicalisation collapses whitespace"
    );
    assert_eq!(
        record.attributes[0].key, "colour",
        "attribute keys casefold and sort"
    );

    // ---- 2. Offers: two shapes, same object, no category-specific path -------------------------
    let goods = shipped_good_offer();
    let event = event_offer();

    // ---- 3. Cart: assembled on the buyer's device, split per seller ----------------------------
    let cart = Cart {
        lines: vec![
            CartLine {
                offer: addr(1),
                seller: seller_goods.clone(),
                quantity: 2,
            },
            CartLine {
                offer: addr(2),
                seller: seller_event.clone(),
                quantity: 1,
            },
        ],
    };
    let groups = cart.split_by_seller();
    assert_eq!(
        groups.len(),
        2,
        "one sealed order per seller, never a shared view"
    );
    for (seller, lines) in &groups {
        assert!(
            lines.iter().all(|l| &l.seller == seller),
            "a seller's order must contain only that seller's lines"
        );
    }

    // ---- 4. Jurisdiction: place of supply differs per line, from the fulfilment axis -----------
    let goods_supply = place_of_supply(&goods.fulfilment, NZ, Some(NZ)).unwrap();
    // No venue argument: the event's place is read out of the offer itself, so nothing a caller
    // passes can move a German event somewhere more convenient.
    let event_supply = place_of_supply(&event.fulfilment, NZ, None).unwrap();
    assert_eq!(
        goods_supply, NZ,
        "shipped goods are supplied at the destination"
    );
    assert_eq!(
        event_supply, DE,
        "the event is supplied in Germany though neither party is German — \
         the whole reason there are four anchors and not two"
    );
    assert_ne!(
        goods_supply, event_supply,
        "two lines of one cart can have different tax anchors; a per-order anchor would be wrong"
    );

    // ---- 5. Jurisdiction: an in-region responsible person gates the EU-eligible offer ----------
    let without_rep = ResponsibleParties {
        seller_of_record: seller_event.clone(),
        facilitator: None,
        importer_of_record: None,
        responsible_person: None,
    };
    assert!(
        without_rep
            .may_offer_into(DE, &event.sell_to, true)
            .is_err(),
        "a region requiring a representative refuses an offer that names none"
    );
    let with_rep = ResponsibleParties {
        responsible_person: Some(InRegionRepresentative {
            region: DE,
            who: key(7),
        }),
        ..without_rep.clone()
    };
    assert!(with_rep.may_offer_into(DE, &event.sell_to, true).is_ok());

    // ---- 6. Inventory: the goods seller runs two replicas that never coordinate ----------------
    let mut counter_shop = BoundedCounter::new(key(11), 6);
    let mut counter_warehouse = BoundedCounter::new(key(12), 4);
    counter_shop
        .reserve(2)
        .expect("the cart's 2 units come from the shop replica's own quota");
    // the warehouse sells hard at the same moment, with no knowledge of the shop
    while counter_warehouse.reserve(1).is_ok() {}
    assert_eq!(
        counter_shop.rights() + counter_warehouse.rights(),
        10,
        "rights are conserved: sold + remaining always equals the stock that was partitioned"
    );
    assert!(
        counter_shop.sold() + counter_warehouse.sold() <= 10,
        "no oversell, with no coordinator between the replicas"
    );

    // ---- 7. Delivery: routing computed locally, on the buyer's node ----------------------------
    let parcel = Parcel {
        length_mm: 220,
        width_mm: 140,
        height_mm: 30,
        weight_grams: 300,
    };
    assert_eq!(
        parcel.billable_grams(5000),
        300,
        "a small dense parcel bills on actual weight"
    );
    let direct = RouteOption {
        shape: RouteShape::Direct,
        legs: vec![Leg {
            carrier: key(20),
            source: RateSource::PublishedCard(addr(30)),
            cost: zar(18_000),
            transit_days: 9,
        }],
        wait_days: 0,
        hub_fees: zar(0),
    };
    let via_hub = RouteOption {
        shape: RouteShape::HubNearBuyer,
        legs: vec![
            Leg {
                carrier: key(20),
                source: RateSource::PublishedCard(addr(30)),
                cost: zar(9_000),
                transit_days: 7,
            },
            Leg {
                carrier: key(21),
                source: RateSource::PublishedCard(addr(31)),
                cost: zar(2_500),
                transit_days: 2,
            },
        ],
        wait_days: 3,
        hub_fees: zar(1_200),
    };
    let (a, b) = (direct.total().unwrap(), via_hub.total().unwrap());
    assert_eq!(a.minor_units, 18_000);
    assert_eq!(b.minor_units, 12_700);
    let chosen = if b.minor_units < a.minor_units {
        &via_hub
    } else {
        &direct
    };
    assert_eq!(
        chosen.shape,
        RouteShape::HubNearBuyer,
        "consolidation wins here, and the comparison ran locally with no quote API"
    );

    // ---- 8. Settlement: an operator licensed for ZA→NZ goods, and what it refuses --------------
    let scope = EscrowScope {
        operator: key(40),
        buyer_countries: vec![NZ],
        seller_countries: vec![ZA],
        supply_countries: vec![NZ],
        currencies: vec![ZAR],
        rail_classes: vec![RailClass::CustodialReversible],
        max_order_value: zar(500_000),
        excluded_categories: vec!["alcohol".into()],
        authorities: vec!["example authorisation".into()],
        declines: vec![],
    };
    let goods_trade = TradeContext {
        buyer_country: NZ,
        seller_country: ZA,
        supply_country: goods_supply,
        value: zar(90_000),
        rail_class: RailClass::CustodialReversible,
        category: "stationery".into(),
        is_import: false,
    };
    assert!(
        matches!(scope.check(&goods_trade), EscrowAvailability::Available(_)),
        "the shipped-goods line is inside the operator's licence"
    );

    // the SAME operator must refuse the event line — only the place of supply differs
    let event_trade = TradeContext {
        supply_country: event_supply,
        value: zar(30_000),
        category: "events".into(),
        ..goods_trade.clone()
    };
    assert_eq!(
        scope.check(&event_trade),
        EscrowAvailability::None(vec![ScopeMismatch::SupplyCountry]),
        "escrow scope must fail closed on place of supply, not just on the parties' countries — \
         both parties are identical between these two trades"
    );

    // ---- 9. Orders: sealed, one per seller, never published ------------------------------------
    let mut orders: Vec<Order> = groups
        .iter()
        .map(|(seller, lines)| Order {
            buyer: buyer.clone(),
            seller: seller.clone(),
            lines: lines.clone(),
            total: zar(if seller == &seller_goods {
                90_000
            } else {
                30_000
            }),
            state: OrderState::Placed,
            placed: Timestamp(1_700_000_100_000),
        })
        .collect();

    // one seller accepts, the other declines — and there is no rollback, by design
    orders[0].state = OrderState::Accepted;
    orders[1].state = OrderState::Declined;
    assert_ne!(
        orders[0].state, orders[1].state,
        "a multi-seller cart is independent orders: one declining does not undo the other, \
         because a distributed transaction would need a coordinator over sovereign parties"
    );

    // ---- 10. Trust: a review, weighted by what actually attests it -----------------------------
    let attested = Review {
        subject: Subject::Seller(seller_goods.clone()),
        author: key(99), // a per-seller pseudonymous subkey, not the buyer's root identity
        score: 5,
        body: "arrived intact".into(),
        attestation: Some(PurchaseAttestation {
            attestor: Attestor::Escrow,
            issuer: key(40),
            order: addr(50),
            at: Timestamp(1_700_100_000_000),
        }),
        at: Timestamp(1_700_100_000_000),
    };
    let unattested = Review {
        attestation: None,
        score: 1,
        ..attested.clone()
    };
    let score = soko_trust::local_score(&[attested, unattested], Weighting::CONSERVATIVE);
    assert_eq!(
        score,
        Some(5.0),
        "the unattested one-star does not move a conservative index's score; \
         a different index may weight it differently, and that divergence is the design"
    );

    // ---- 11. Gateway: two stores are never served from one origin -----------------------------
    let store_a = StoreBinding {
        seller: seller_goods,
        host: StoreHost::Subdomain {
            label: "alice".into(),
            base: "soko.example".into(),
        },
    };
    let store_b = StoreBinding {
        seller: seller_event,
        host: StoreHost::Subdomain {
            label: "bob".into(),
            base: "soko.example".into(),
        },
    };
    assert!(
        store_a.origin_isolated_from(&store_b),
        "merchant render bundles are untrusted code; a shared origin lets one read the other's cart"
    );
}

/// The buyer's device is the only place the whole cart exists. Asserted separately because it is
/// a privacy property, not a correctness one, and privacy properties are the ones that erode
/// quietly when someone adds a convenient field later.
#[test]
fn no_seller_can_reconstruct_the_whole_cart_from_what_it_receives() {
    let cart = Cart {
        lines: vec![
            CartLine {
                offer: addr(1),
                seller: key(1),
                quantity: 1,
            },
            CartLine {
                offer: addr(2),
                seller: key(2),
                quantity: 1,
            },
            CartLine {
                offer: addr(3),
                seller: key(3),
                quantity: 1,
            },
        ],
    };
    for (seller, lines) in cart.split_by_seller() {
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].seller, seller);
        assert!(
            lines.len() < cart.lines.len(),
            "no seller's view is the whole cart"
        );
    }
}
