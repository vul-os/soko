//! # soko-node — the Soko node
//!
//! One binary, several roles: publish a catalogue, receive sealed orders, compute delivery
//! routing, serve public objects. Which roles are active is a run-time choice, not a build-time
//! one — the same shape the DMTAP substrate uses.
//!
//! The gateway role is deliberately **not** here. It terminates untrusted connections and renders
//! untrusted merchant bundles, so it runs as a separate process with no access to identity keys
//! (TRACT §12.4). "One binary, several roles" never means one address space.
//!
//! ## What this currently does
//!
//! Nothing networked. With no arguments it prints what it is; with `demo` it walks one complete
//! two-seller trade through every crate and prints the decisions, so the object model can be
//! inspected without reading test source. The same trade is asserted in `tests/end_to_end.rs` —
//! this is the readable view of it, not a second implementation of it.

use soko_core::{ContentAddress, Country, Currency, IdentityKey, Money, Timestamp};
use soko_delivery::{Leg, Parcel, RateSource, RouteOption, RouteShape};
use soko_jurisdiction::place_of_supply;
use soko_offer::{Availability, Consideration, Fulfilment, Item, Offer, PlaceRef, StockSignal};
use soko_order::{BoundedCounter, Cart, CartLine};
use soko_settle::{EscrowAvailability, EscrowScope, RailClass, TradeContext};

const ZA: Country = Country(*b"ZA");
const DE: Country = Country(*b"DE");
const NZ: Country = Country(*b"NZ");
const ZAR: Currency = Currency(*b"ZAR");

fn cc(c: Country) -> String {
    String::from_utf8_lossy(&c.0).to_string()
}
fn rands(m: Money) -> String {
    format!(
        "R{}.{:02}",
        m.minor_units / 100,
        (m.minor_units % 100).abs()
    )
}
fn zar(n: i64) -> Money {
    Money {
        minor_units: n,
        currency: ZAR,
    }
}

fn main() {
    if std::env::args().nth(1).as_deref() != Some("demo") {
        println!(
            "soko-node {} — TRACT reference implementation\n\
             \n\
             Pre-alpha: the protocol is being written first, and nothing is networked yet.\n\
             Spec:   https://github.com/vul-os/kotva/tree/main/profiles/tract\n\
             Roles:  seller · buyer · courier · distributor · index (planned)\n\
             Note:   the gateway role runs as a separate process, never in this one.\n\
             \n\
             Try:    cargo run -p soko-node -- demo",
            env!("CARGO_PKG_VERSION")
        );
        return;
    }

    println!("One cart, two sovereign sellers, three countries.\n");

    // Two sellers who have never heard of each other. One ships a physical good; the other sells
    // admission to an event held somewhere neither party lives.
    let goods = Offer {
        item: Item::Product(ContentAddress(vec![1])),
        availability: Availability::Count(StockSignal::Exact(10)),
        fulfilment: Fulfilment::Ship { to: vec![NZ] },
        consideration: Consideration::Fixed(zar(45_000)),
        sell_to: vec![NZ, ZA],
        published: Timestamp(0),
    };
    let event = Offer {
        item: Item::Capacity(ContentAddress(vec![2])),
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
        published: Timestamp(0),
    };

    let cart = Cart {
        lines: vec![
            CartLine {
                offer: ContentAddress(vec![1]),
                seller: IdentityKey(vec![1]),
                quantity: 2,
            },
            CartLine {
                offer: ContentAddress(vec![2]),
                seller: IdentityKey(vec![2]),
                quantity: 1,
            },
        ],
    };

    println!("CART — held on the buyer's device, in NZ");
    for (seller, lines) in cart.split_by_seller() {
        println!(
            "  seller {:?}: {} line(s)  ->  one sealed order, containing only these",
            seller.0,
            lines.len()
        );
    }
    println!("  no party receives the whole cart\n");

    // Place of supply comes from the fulfilment axis, not from either party's country.
    println!("PLACE OF SUPPLY — derived per line, from fulfilment");
    let g = place_of_supply(&goods.fulfilment, NZ, Some(NZ)).unwrap();
    let e = place_of_supply(&event.fulfilment, NZ, None).unwrap();
    println!("  shipped good   -> {}  (the destination)", cc(g));
    println!(
        "  event ticket   -> {}  (the venue — neither party is there)",
        cc(e)
    );
    println!("  two lines, one cart, different tax anchors\n");

    // Two replicas of the goods seller, selling at the same moment with no coordination.
    println!("INVENTORY — two replicas, no coordinator");
    let mut shop = BoundedCounter::new(IdentityKey(vec![11]), 6);
    let mut warehouse = BoundedCounter::new(IdentityKey(vec![12]), 4);
    shop.reserve(2).ok();
    while warehouse.reserve(1).is_ok() {}
    println!(
        "  shop      sold {}, {} left in its own quota",
        shop.sold(),
        shop.quota()
    );
    println!(
        "  warehouse sold {}, {} left in its own quota",
        warehouse.sold(),
        warehouse.quota()
    );
    println!(
        "  total sold {} of 10 — no oversell, and neither replica asked the other\n",
        shop.sold() + warehouse.sold()
    );

    // Routing compared locally from published rate cards.
    println!("DELIVERY — compared on this machine, from published rate cards");
    let parcel = Parcel {
        length_mm: 220,
        width_mm: 140,
        height_mm: 30,
        weight_grams: 300,
    };
    println!(
        "  parcel billable weight: {}g (actual 300g, volumetric divisor 5000)",
        parcel.billable_grams(5000)
    );
    let leg = |cost, days, id| Leg {
        carrier: IdentityKey(vec![id]),
        source: RateSource::PublishedCard(ContentAddress(vec![30])),
        cost: zar(cost),
        transit_days: days,
    };
    let direct = RouteOption {
        shape: RouteShape::Direct,
        legs: vec![leg(18_000, 9, 20)],
        wait_days: 0,
        hub_fees: zar(0),
    };
    let hub = RouteOption {
        shape: RouteShape::HubNearBuyer,
        legs: vec![leg(9_000, 7, 20), leg(2_500, 2, 21)],
        wait_days: 3,
        hub_fees: zar(1_200),
    };
    for r in [&direct, &hub] {
        println!(
            "  {:?}: {} (wait {} day(s))",
            r.shape,
            rands(r.total().unwrap()),
            r.wait_days
        );
    }
    println!("  chosen: consolidation, and no quote API was called\n");

    // The same escrow operator serves one line and must refuse the other.
    println!("ESCROW — one operator, licensed for ZA->NZ goods");
    let scope = EscrowScope {
        operator: IdentityKey(vec![40]),
        buyer_countries: vec![NZ],
        seller_countries: vec![ZA],
        supply_countries: vec![NZ],
        currencies: vec![ZAR],
        rail_classes: vec![RailClass::CustodialReversible],
        max_order_value: zar(500_000),
        excluded_categories: vec![],
        authorities: vec!["example authorisation".into()],
        declines: vec![],
    };
    let base = TradeContext {
        buyer_country: NZ,
        seller_country: ZA,
        supply_country: g,
        value: zar(90_000),
        rail_class: RailClass::CustodialReversible,
        category: "stationery".into(),
        is_import: false,
    };
    let evt = TradeContext {
        supply_country: e,
        category: "events".into(),
        ..base.clone()
    };
    for (label, t) in [("shipped good", &base), ("event ticket", &evt)] {
        match scope.check(t) {
            EscrowAvailability::Available(_) => println!("  {label}: escrow available"),
            EscrowAvailability::None(why) => println!(
                "  {label}: NO escrow — {why:?}. The trade may still proceed, \
                 with the risk disclosed to both parties."
            ),
        }
    }
    println!(
        "\n  Both parties are identical between those two trades.\n  \
         Only the place of supply differs, and that is enough."
    );
}
