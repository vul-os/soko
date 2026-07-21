//! Cross-checks Soko's own computations against the TRACT conformance vectors — the corpus that
//! `../tract/conformance/vectors/` derives by hand, directly from the specification text, without
//! ever consulting this workspace (see `conformance/README.md` and
//! `conformance/vectors/README.md` in the `tract` repo for why that separation is the whole
//! point: a vector exported from Soko could only ever prove Soko self-consistent, never that Soko
//! agrees with the spec).
//!
//! Before this file existed, Soko never read those vectors at all — the loop the discipline
//! describes was open on Soko's side: the vectors proved the spec had been read correctly, and
//! nothing proved Soko agreed with it. This file closes that loop for the slice of the corpus
//! Soko can actually check today: the four `"kind": "derived-value"` vectors, whose formulas map
//! onto a real Soko function.
//!
//! ## What this deliberately does not do
//!
//! - It does not check the seventeen `"kind": "cbor-encoding"` vectors. Those pin exact §16 wire
//!   bytes (canonical CBOR with integer CDDL keys), and nothing in this workspace encodes with
//!   integer keys yet — `soko-core`'s round-trip tests (see `soko_core::roundtrip`) establish that
//!   the *current* derive-based encoding is internally *stable*, which is a different, narrower
//!   claim than "matches §16's CDDL byte-for-byte". Checking the CBOR vectors is future work for
//!   whenever Soko grows a real wire encoder.
//! - It does not check the two `"kind": "structural-check"` vectors (`TRACT-CAT-03`,
//!   `TRACT-PUBSEAL-03`) for the same reason: they assert decode-time rejection of a *specific
//!   encoded byte shape*, which needs the real encoder above.
//! - Where a derived-value vector's formula has no matching Soko function at all, this file does
//!   not invent one to "cover" it — it records the vector as **explicitly uncovered**, with the
//!   specific reason, in the summary this test prints. Silently skipping it would be worse than
//!   useless: a coverage gap that prints nothing looks identical to a coverage gap that was
//!   checked and passed.
//!
//! ## Handling the vectors' absence
//!
//! A checkout of `soko` alone, with no sibling `tract` checkout, is legitimate — this repository
//! does not vendor the spec. [`vectors_dir`] returns `None` in that case and the test below prints
//! why it is skipping and returns, rather than failing. It never silently passes with no output.

use std::path::{Path, PathBuf};

use serde_json::Value;

use soko_catalog::{Attribute, IdentityRung, ProductGroup, ProductRecord};
use soko_core::{roundtrip, ContentAddress, Country, Currency, IdentityKey, Money, Timestamp};
use soko_delivery::{
    Consignment, CustodyHandoff, DistributorCapacity, Leg, Parcel, RateCard, RateSource,
    RouteOption, RouteShape, WeightBracket, Zone,
};
use soko_gateway::{StoreBinding, StoreHost};
use soko_jurisdiction::{place_of_supply, Anchors, InRegionRepresentative, ResponsibleParties};
use soko_offer::{
    Availability, Consideration, Fulfilment, Item, Offer, PlaceRef, PriceTier, StockSignal,
};
use soko_order::{BoundedCounter, Cart, CartLine, Order, OrderState};
use soko_settle::{
    EscrowRuling, EscrowScope, EscrowState, PaymentAttestation, RailClass, RailSubstitution,
};
use soko_trust::{Attestor, PurchaseAttestation, Review, Subject};

// ==================================================================================================
// Vector location and loading
// ==================================================================================================

/// Where the TRACT vectors live, relative to this crate — `tract` is a sibling checkout of `soko`,
/// never vendored into it (`conformance/README.md`'s "no implementation is part of the standard"
/// cuts both ways: this workspace does not embed the spec repo either).
///
/// Returns `None`, rather than panicking, when the sibling checkout is absent — see the module
/// docs above for why that is a legitimate state this test must handle gracefully.
fn vectors_dir() -> Option<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tract/conformance/vectors");
    dir.is_dir().then_some(dir)
}

/// Read and parse one vector file. Panics on failure — unlike a missing `tract` checkout entirely
/// (handled by [`vectors_dir`]), a missing or malformed *file* inside a `vectors_dir` that does
/// exist means something is actually wrong (a rename upstream, a corrupt checkout) and should be
/// loud rather than silently skipped.
fn load(path: &Path) -> Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read vector file {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| {
        panic!(
            "failed to parse vector file {} as JSON: {e}",
            path.display()
        )
    })
}

// ==================================================================================================
// JSON -> Soko value parsing, for the fields these four vectors actually use
// ==================================================================================================

fn country_code(s: &str) -> Country {
    let b = s.as_bytes();
    assert_eq!(
        b.len(),
        2,
        "country code must be 2 ASCII letters, got {s:?}"
    );
    Country([b[0], b[1]])
}

fn currency_code(s: &str) -> Currency {
    let b = s.as_bytes();
    assert_eq!(
        b.len(),
        3,
        "currency code must be 3 ASCII letters, got {s:?}"
    );
    Currency([b[0], b[1], b[2]])
}

fn place_ref(v: &Value) -> PlaceRef {
    let obj = v.as_object().expect("PlaceRef must be a JSON object");
    PlaceRef {
        country: country_code(
            obj["1"]
                .as_str()
                .expect("PlaceRef.1 (country) must be a string"),
        ),
        locality: obj["2"]
            .as_str()
            .expect("PlaceRef.2 (locality) must be a string")
            .to_string(),
    }
}

/// Parse the vector's `fulfilment` object into `soko_offer::Fulfilment`.
///
/// Each variant is a map keyed by its §16.5.2 CDDL field number (`"0"`..`"6"`); most variants
/// carry their payload directly at that key, but `return-required` (key `"6"`) is a two-field
/// variant and carries its second field (`term_days`) as a **sibling** key `"7"` in the same map,
/// not nested — matching `offer-fulfilment.json`'s encoding exactly (see that vector's `return-
/// required` case).
fn fulfilment(v: &Value) -> Fulfilment {
    let obj = v
        .as_object()
        .expect("fulfilment must be an object keyed by its §16.5.2 CDDL field number");
    if let Some(to) = obj.get("0") {
        let to = to
            .as_array()
            .expect("Fulfilment.0 (ship destinations) must be an array")
            .iter()
            .map(|c| country_code(c.as_str().expect("destination must be a country string")))
            .collect();
        return Fulfilment::Ship { to };
    }
    if let Some(at) = obj.get("1") {
        return Fulfilment::Collect { at: place_ref(at) };
    }
    if obj.contains_key("2") {
        return Fulfilment::DigitalGrant;
    }
    if let Some(at) = obj.get("3") {
        return Fulfilment::PerformAtPlace { at: place_ref(at) };
    }
    if obj.contains_key("4") {
        return Fulfilment::PerformRemote;
    }
    if obj.contains_key("5") {
        let at = &obj["5"];
        return Fulfilment::AccessGrant {
            at: if at.is_null() {
                None
            } else {
                Some(place_ref(at))
            },
        };
    }
    if let Some(at) = obj.get("6") {
        let term_days = obj
            .get("7")
            .and_then(Value::as_u64)
            .expect("return-required (key 6) needs a sibling key 7 (term_days)")
            as u16;
        return Fulfilment::ReturnRequired {
            at: place_ref(at),
            term_days,
        };
    }
    panic!(
        "unrecognised Fulfilment CDDL keys in vector input: {:?}",
        obj.keys().collect::<Vec<_>>()
    );
}

// ==================================================================================================
// A small report so one test run can check every case in every vector and print one honest
// summary, rather than panicking (and stopping) on the first mismatch. A disagreement between a
// hand-derived vector and Soko is the single most valuable thing this file could find — stopping
// early would mean finding at most one per run.
// ==================================================================================================

#[derive(Default)]
struct Report {
    checked: Vec<String>,
    disagreements: Vec<String>,
    uncovered: Vec<String>,
}

impl Report {
    fn check(&mut self, name: impl Into<String>, ok: bool, detail: impl Into<String>) {
        let name = name.into();
        if ok {
            self.checked.push(name);
        } else {
            self.disagreements
                .push(format!("{name} — {}", detail.into()));
        }
    }

    fn uncovered(&mut self, name: impl Into<String>, case_count: usize, reason: impl Into<String>) {
        let plural = if case_count == 1 { "" } else { "s" };
        self.uncovered.push(format!(
            "{} ({case_count} case{plural}): {}",
            name.into(),
            reason.into()
        ));
    }

    /// Print the honest summary and fail loudly if anything disagreed. Uncovered cases are
    /// reported, never treated as a failure — they are an accurate statement of what Soko does not
    /// yet implement, not a bug in this test.
    fn finish(self) {
        println!("\n=== TRACT conformance summary (derived-value vectors) ===");
        println!("verified against Soko, all agreed: {}", self.checked.len());
        for c in &self.checked {
            println!("  ok  {c}");
        }
        println!(
            "explicitly uncovered:               {}",
            self.uncovered.len()
        );
        for u in &self.uncovered {
            println!("  --  {u}");
        }
        println!(
            "disagreements:                       {}",
            self.disagreements.len()
        );
        for d in &self.disagreements {
            println!("  !!  {d}");
        }
        println!("===========================================================\n");

        assert!(
            self.disagreements.is_empty(),
            "Soko disagrees with {} hand-derived TRACT vector case(s). Per conformance/README.md: \
             investigate which one is wrong, with the specification text as the tiebreaker — do \
             NOT edit the vector to match Soko, that is the exact failure mode the discipline \
             exists to prevent. Disagreements:\n{}",
            self.disagreements.len(),
            self.disagreements.join("\n")
        );
    }
}

// ==================================================================================================
// TRACT-DELIV-01 — billable weight (checkable) and rate-card bracket/price lookup (not checkable)
// ==================================================================================================

fn check_billable_weight(dir: &Path, report: &mut Report) {
    let doc =
        load(&dir.join("billable-weight-and-local-rate-card-price-lookup-tract-deliv-01.json"));
    let cases = doc["cases"].as_array().expect("cases must be an array");
    for case in cases {
        let name = case["name"].as_str().unwrap_or("<unnamed case>");
        let input = &case["input"];
        let expected = &case["expected"];

        let actual_grams = input["actual_grams"].as_u64().unwrap() as u32;
        let l = input["l_cm"].as_u64().unwrap() as u32;
        let w = input["w_cm"].as_u64().unwrap() as u32;
        let h = input["h_cm"].as_u64().unwrap() as u32;
        let divisor = input["dim_divisor"].as_u64().unwrap() as u32;

        // `Parcel`'s fields are typed in millimetres and the vector's L/W/H are centimetres, but
        // `billable_grams`'s formula (`max(actual, L*W*H / divisor)`) is unit-agnostic
        // multiply-then-divide arithmetic with no unit conversion inside it — feeding it the
        // vector's cm-denominated numbers checks the identical arithmetic the vector checks. This
        // is not a claim that Soko treats these values as centimetres; it is reusing the one
        // function that performs this arithmetic to check the arithmetic itself.
        let parcel = Parcel {
            length_mm: l,
            width_mm: w,
            height_mm: h,
            weight_grams: actual_grams,
        };
        let billable = parcel.billable_grams(divisor);
        let expected_billable = expected["billable_grams"].as_u64().unwrap() as u32;

        report.check(
            format!("TRACT-DELIV-01 billable weight — {name}"),
            billable == expected_billable,
            format!(
                "Parcel::billable_grams({divisor}) = {billable}, vector expects \
                 billable_grams = {expected_billable}"
            ),
        );
    }

    // The vector's second half — selecting a `WeightBracket` by billable weight and reading its
    // `price` off a published `RateCard`/`Zone` — has no Soko-side counterpart. `soko_delivery`
    // defines `RateCard`, `Zone` and `WeightBracket` as plain published data (§16.5.3) but no
    // function anywhere in the crate selects a bracket or looks up a price from one (confirmed:
    // `grep -n "fn " crates/soko-delivery/src/lib.rs` lists only `is_usable`, `billable_grams` and
    // `RouteOption::total` — no `select_bracket` or equivalent). The vector's `formula` field names
    // `billable_weight, select_bracket` as one unit; only the first half is checkable today.
    report.uncovered(
        "TRACT-DELIV-01 rate-card bracket selection and price lookup (select_bracket)",
        cases.len(),
        "no Soko function computes this: `RateCard`/`Zone`/`WeightBracket` are published data \
         with no selection or lookup method anywhere in soko-delivery",
    );
}

// ==================================================================================================
// TRACT-SETTLE-01 — EscrowScope checkout intersection (not checkable: no matching Soko function)
// ==================================================================================================

fn check_escrow_scope_intersection(dir: &Path, report: &mut Report) {
    let doc = load(&dir.join("escrowscope-checkout-intersection.json"));
    let case_count = doc["cases"].as_array().map(Vec::len).unwrap_or(0);

    // This is the one instance in this file where "the obvious Soko function" (`EscrowScope::
    // check`) does NOT actually compute what the vector checks, and it is worth being precise
    // about why rather than papering over the mismatch:
    //
    // - the vector's `formula`, `escrow_scope_intersect`, intersects TWO published `EscrowScope`
    //   declarations (a buyer-declared scope and a gateway-offered scope, each carrying
    //   `Vec<Country>`/`Vec<Currency>`/`Vec<RailClass>` etc.) into a narrower published scope,
    //   refusing outright if any field's intersection is empty (§9.4);
    // - `EscrowScope::check` (soko-settle) checks ONE concrete `TradeContext` — singular
    //   `Country`/`Currency`/`RailClass`/`Money`/`String` values, not sets — against ONE declared
    //   `EscrowScope`, returning the specific fields that mismatched. That is a scope-vs-trade
    //   membership test, structurally different from a scope-vs-scope set intersection: there is
    //   no second `EscrowScope` argument to `check` for a gateway's offered scope to occupy, and
    //   no way to feed it two `Vec<Country>` values and get a narrowed `Vec<Country>` back.
    //
    // `grep -rn intersect crates/` (run while building this test) finds no function anywhere in
    // the workspace, so this is not a naming mismatch this file could route around — the
    // computation the vector's `formula` names simply is not implemented yet. Reporting it as
    // uncovered, rather than silently calling `EscrowScope::check` "close enough", is the honest
    // reading of it.
    report.uncovered(
        "TRACT-SETTLE-01 EscrowScope checkout intersection (escrow_scope_intersect)",
        case_count,
        "no Soko function computes this: EscrowScope::check tests one concrete TradeContext \
         against one declared EscrowScope (a scope-vs-trade membership check), not two \
         EscrowScope declarations intersected into a narrower one (a scope-vs-scope set \
         intersection) — the two are structurally different computations, and no function \
         performing the second exists anywhere in the workspace (checked: `grep -rn intersect \
         crates/` finds nothing)",
    );
}

// ==================================================================================================
// TRACT-FULF-01 / TRACT-FULF-02 — place-of-supply derivation across all 7 Fulfilment variants
// ==================================================================================================

fn check_place_of_supply(dir: &Path, report: &mut Report) {
    let doc = load(&dir.join(
        "place-of-supply-derivation-for-all-7-fulfilment-variants-tract-fulf-01-tract-fulf-02.json",
    ));

    // The vector's own description holds seller establishment (ZA) and buyer residence (DE)
    // constant across every case, specifically so a reader can see the derived anchor is never
    // either of those two values except where §4.3 says it should equal buyer residence. Those two
    // constants are NOT repeated in each case's `input` JSON (only in the vector's prose) — seller
    // establishment plays no role in `place_of_supply`'s signature at all (correctly: that is the
    // whole point of TRACT-FULF-01), so only buyer residence needs supplying here.
    let buyer_residence = country_code("DE");

    for case in doc["cases"].as_array().expect("cases must be an array") {
        let name = case["name"].as_str().unwrap_or("<unnamed case>");
        let input = &case["input"];
        let f = fulfilment(&input["fulfilment"]);
        let chosen_destination = input
            .get("chosen_destination")
            .and_then(Value::as_str)
            .map(country_code);
        let expected = country_code(
            case["expected"]
                .as_str()
                .expect("expected must be a country code string"),
        );

        let actual = place_of_supply(&f, buyer_residence, chosen_destination);
        report.check(
            format!("TRACT-FULF-01/02 place of supply — {name}"),
            actual == Ok(expected),
            format!("place_of_supply(..) = {actual:?}, vector expects Ok({expected:?})"),
        );
    }
}

// ==================================================================================================
// TRACT-DELIV-03 — route totals: same-currency sum, mixed-currency refusal
// ==================================================================================================

fn check_route_totals(dir: &Path, report: &mut Report) {
    let doc = load(&dir.join("route-totals.json"));

    for case in doc["cases"].as_array().expect("cases must be an array") {
        let name = case["name"].as_str().unwrap_or("<unnamed case>");
        let legs_json = case["input"]
            .as_array()
            .expect("input must be an array of legs");

        let legs: Vec<Leg> = legs_json
            .iter()
            .enumerate()
            .map(|(i, leg)| {
                let minor_units = leg["2"]
                    .as_i64()
                    .expect("leg.2 (minor_units) must be an int");
                let currency = currency_code(
                    leg["3"]
                        .as_str()
                        .expect("leg.3 (currency) must be a string"),
                );
                Leg {
                    carrier: IdentityKey(vec![i as u8]),
                    source: RateSource::LiveQuoteRequired,
                    cost: Money {
                        minor_units,
                        currency,
                    },
                    transit_days: 1,
                }
            })
            .collect();

        // Zero hub fees, denominated in the first leg's currency so hub_fees itself never
        // introduces a currency mismatch this vector is not testing for.
        let hub_currency = legs
            .first()
            .map(|l| l.cost.currency)
            .unwrap_or(Currency(*b"USD"));
        let route = RouteOption {
            shape: RouteShape::Direct,
            legs,
            wait_days: 0,
            hub_fees: Money {
                minor_units: 0,
                currency: hub_currency,
            },
        };

        let result = route.total();
        let expected_refused = case["expected"]["refused"]
            .as_bool()
            .expect("expected.refused must be a bool");

        match (&result, expected_refused) {
            (Ok(total), false) => {
                let exp = &case["expected"]["total"];
                let exp_minor = exp["1"].as_i64().expect("expected.total.1 must be an int");
                let exp_currency = currency_code(
                    exp["2"]
                        .as_str()
                        .expect("expected.total.2 must be a string"),
                );
                report.check(
                    format!("TRACT-DELIV-03 route total — {name}"),
                    total.minor_units == exp_minor && total.currency == exp_currency,
                    format!(
                        "RouteOption::total() = {total:?}, vector expects \
                         {{minor_units: {exp_minor}, currency: {exp_currency:?}}}"
                    ),
                );
            }
            (Err(_), true) => {
                report.check(format!("TRACT-DELIV-03 route total — {name}"), true, "");
            }
            _ => {
                report.check(
                    format!("TRACT-DELIV-03 route total — {name}"),
                    false,
                    format!(
                        "RouteOption::total() = {result:?}, vector expects refused = {expected_refused}"
                    ),
                );
            }
        }
    }
}

// ==================================================================================================
// The orchestrating test
// ==================================================================================================

#[test]
fn tract_hand_derived_vectors_agree_with_soko() {
    let Some(dir) = vectors_dir() else {
        println!(
            "SKIP tract_hand_derived_vectors_agree_with_soko: no sibling `tract` checkout found \
             at {} — a checkout of `soko` alone is legitimate and this test does not fail for it. \
             Clone the `tract` repo next to `soko` (so `../tract` from the soko workspace root \
             resolves) to run this check.",
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tract/conformance/vectors")
                .display()
        );
        return;
    };

    let mut report = Report::default();
    check_billable_weight(&dir, &mut report);
    check_escrow_scope_intersection(&dir, &mut report);
    check_place_of_supply(&dir, &mut report);
    check_route_totals(&dir, &mut report);
    report.finish();
}

// ==================================================================================================
// Round-trip serialisation coverage for every other crate's public serialisable types.
//
// `soko-core`'s own types are covered in `soko-core/src/lib.rs`'s tests, next to the
// `soko_core::roundtrip` helpers this file reuses (see that module's doc comment for why byte
// STABILITY, not mere round-trip equality, is the property worth checking, and why decoding needs
// its own explicit "all bytes consumed" check rather than trusting `ciborium::from_reader` alone).
// This section extends the same check across every other crate in the workspace, since soko-node
// already depends on all of them and is the one place a single test file can reach every type.
// ==================================================================================================

fn key(n: u8) -> IdentityKey {
    IdentityKey(vec![n])
}
fn addr(n: u8) -> ContentAddress {
    ContentAddress(vec![n])
}
fn zar(n: i64) -> Money {
    Money {
        minor_units: n,
        currency: Currency(*b"ZAR"),
    }
}
fn place(country: &str, locality: &str) -> PlaceRef {
    PlaceRef {
        country: country_code(country),
        locality: locality.to_string(),
    }
}

#[test]
fn catalog_types_roundtrip_stably() {
    let record = ProductRecord {
        name: "Field Notebook".into(),
        description: "pocket notebook".into(),
        attributes: vec![
            Attribute {
                key: "colour".into(),
                value: "black".into(),
            },
            Attribute {
                key: "size".into(),
                value: "A6".into(),
            },
        ],
        identity: vec![
            IdentityRung::ContentAddress(addr(1)),
            IdentityRung::ClaimedExternal {
                scheme: "gtin".into(),
                value: "0012345".into(),
            },
            IdentityRung::ManufacturerSigned(key(2)),
        ],
        group: Some(addr(9)),
        components: vec![addr(3), addr(4)],
    };
    roundtrip::assert_stable(&record);
    roundtrip::assert_truncation_is_rejected(&record);
    roundtrip::assert_trailing_garbage_is_rejected(&record);

    roundtrip::assert_stable(&Attribute {
        key: "colour".into(),
        value: "black".into(),
    });

    let group = ProductGroup {
        name: "Field Notebook".into(),
        varies_by: vec!["colour".into(), "size".into()],
        variants: vec![addr(1), addr(2)],
    };
    roundtrip::assert_stable(&group);
}

#[test]
fn offer_types_roundtrip_stably() {
    for item in [
        Item::Product(addr(1)),
        Item::VariantOfGroup {
            group: addr(2),
            variant: addr(3),
        },
        Item::Service(addr(4)),
        Item::Right(addr(5)),
        Item::Capacity(addr(6)),
    ] {
        roundtrip::assert_stable(&item);
    }

    for availability in [
        Availability::Count(StockSignal::Exact(10)),
        Availability::Count(StockSignal::InStock),
        Availability::Count(StockSignal::Low),
        Availability::Count(StockSignal::OutOfStock),
        Availability::TimeSlots {
            ical: "FREQ=WEEKLY".into(),
            slot_minutes: 30,
        },
        Availability::CapacityPerInterval {
            capacity: 40,
            ical: "FREQ=WEEKLY;BYDAY=SA".into(),
        },
        Availability::Unlimited,
        Availability::MadeToOrder { lead_days: 14 },
    ] {
        roundtrip::assert_stable(&availability);
    }

    let venue = place("FR", "Cannes");
    for fulfilment in [
        Fulfilment::Ship {
            to: vec![country_code("FR"), country_code("DE")],
        },
        Fulfilment::Collect {
            at: place("US", "Portland"),
        },
        Fulfilment::DigitalGrant,
        Fulfilment::PerformAtPlace { at: venue.clone() },
        Fulfilment::PerformRemote,
        Fulfilment::AccessGrant {
            at: Some(place("GB", "London")),
        },
        Fulfilment::AccessGrant { at: None },
        Fulfilment::ReturnRequired {
            at: place("JP", "Osaka"),
            term_days: 30,
        },
    ] {
        roundtrip::assert_stable(&fulfilment);
    }
    roundtrip::assert_stable(&venue);

    for consideration in [
        Consideration::Fixed(zar(45_000)),
        Consideration::Tiered(vec![
            PriceTier {
                min_qty: 1,
                unit_price: zar(100),
            },
            PriceTier {
                min_qty: 10,
                unit_price: zar(90),
            },
        ]),
        Consideration::Recurring {
            amount: zar(5_000),
            rrule: "FREQ=MONTHLY".into(),
        },
        Consideration::Metered {
            dimension: "gigabytes".into(),
            unit_price: zar(50),
        },
        Consideration::DepositBalance {
            deposit: zar(10_000),
            balance: zar(35_000),
        },
        Consideration::QuoteRequired,
    ] {
        roundtrip::assert_stable(&consideration);
    }

    let offer = Offer {
        item: Item::Product(addr(1)),
        availability: Availability::Count(StockSignal::Exact(10)),
        fulfilment: Fulfilment::Ship {
            to: vec![country_code("NZ")],
        },
        consideration: Consideration::Fixed(zar(45_000)),
        sell_to: vec![country_code("NZ"), country_code("ZA")],
        published: Timestamp(1_700_000_000_000),
    };
    roundtrip::assert_stable(&offer);
    roundtrip::assert_truncation_is_rejected(&offer);
    roundtrip::assert_trailing_garbage_is_rejected(&offer);
}

#[test]
fn delivery_types_roundtrip_stably() {
    let bracket = WeightBracket {
        max_grams: 1000,
        price: zar(500),
    };
    roundtrip::assert_stable(&bracket);

    let zone = Zone {
        id: 1,
        brackets: vec![
            bracket.clone(),
            WeightBracket {
                max_grams: 5000,
                price: zar(900),
            },
        ],
        transit_days: 3,
    };
    roundtrip::assert_stable(&zone);

    let card = RateCard {
        carrier: key(20),
        serves: vec![country_code("FR"), country_code("DE")],
        zones: vec![zone],
        dim_divisor: 5000,
        surcharge_pct: 3,
        excluded_categories: vec!["hazmat".into()],
        published: Timestamp(1_700_000_000_000),
    };
    roundtrip::assert_stable(&card);
    roundtrip::assert_truncation_is_rejected(&card);
    roundtrip::assert_trailing_garbage_is_rejected(&card);

    let parcel = Parcel {
        length_mm: 220,
        width_mm: 140,
        height_mm: 30,
        weight_grams: 300,
    };
    roundtrip::assert_stable(&parcel);

    for source in [
        RateSource::PublishedCard(addr(30)),
        RateSource::LiveQuoteRequired,
    ] {
        roundtrip::assert_stable(&source);
    }

    let leg = Leg {
        carrier: key(20),
        source: RateSource::PublishedCard(addr(30)),
        cost: zar(9_000),
        transit_days: 7,
    };
    roundtrip::assert_stable(&leg);

    let handoff = CustodyHandoff {
        from: key(1),
        to: key(2),
        at: Timestamp(1_700_000_100_000),
    };
    roundtrip::assert_stable(&handoff);

    let consignment = Consignment {
        order: addr(50),
        custodian: key(2),
        handoffs: vec![handoff],
    };
    roundtrip::assert_stable(&consignment);

    let capacity = DistributorCapacity {
        distributor: key(3),
        country: country_code("ZA"),
        locality: "Cape Town".into(),
        storage_per_day: zar(20),
        handling_fee: zar(150),
        slots: 50,
        excluded_categories: vec!["alcohol".into()],
    };
    roundtrip::assert_stable(&capacity);

    for shape in [
        RouteShape::Direct,
        RouteShape::HubNearBuyer,
        RouteShape::HubNearSellers,
    ] {
        roundtrip::assert_stable(&shape);
    }

    let route = RouteOption {
        shape: RouteShape::HubNearBuyer,
        legs: vec![leg],
        wait_days: 3,
        hub_fees: zar(1_200),
    };
    roundtrip::assert_stable(&route);
    roundtrip::assert_truncation_is_rejected(&route);
    roundtrip::assert_trailing_garbage_is_rejected(&route);
}

#[test]
fn order_types_roundtrip_stably() {
    let line = CartLine {
        offer: addr(1),
        seller: key(1),
        quantity: 2,
    };
    roundtrip::assert_stable(&line);

    let cart = Cart {
        lines: vec![
            line.clone(),
            CartLine {
                offer: addr(2),
                seller: key(2),
                quantity: 1,
            },
        ],
    };
    roundtrip::assert_stable(&cart);

    let counter = BoundedCounter::new(key(11), 6);
    roundtrip::assert_stable(&counter);

    for state in [
        OrderState::Draft,
        OrderState::Placed,
        OrderState::Accepted,
        OrderState::Declined,
        OrderState::Countered,
        OrderState::Fulfilling,
        OrderState::Delivered,
        OrderState::Closed,
        OrderState::Cancelled,
    ] {
        roundtrip::assert_stable(&state);
    }

    let order = Order {
        buyer: key(9),
        seller: key(1),
        lines: vec![line],
        total: zar(90_000),
        state: OrderState::Placed,
        placed: Timestamp(1_700_000_100_000),
    };
    roundtrip::assert_stable(&order);
    roundtrip::assert_truncation_is_rejected(&order);
    roundtrip::assert_trailing_garbage_is_rejected(&order);
}

#[test]
fn settle_types_roundtrip_stably() {
    for class in [RailClass::CustodialReversible, RailClass::NonCustodialFinal] {
        roundtrip::assert_stable(&class);
    }
    for sub in [RailSubstitution::Agreed, RailSubstitution::Refused] {
        roundtrip::assert_stable(&sub);
    }

    let attestation = PaymentAttestation {
        payer: key(1),
        payee: key(2),
        order: addr(50),
        amount: zar(90_000),
        rail_class: RailClass::CustodialReversible,
        external_ref: "ref-123".into(),
        at: Timestamp(1_700_100_000_000),
    };
    roundtrip::assert_stable(&attestation);

    let scope = EscrowScope {
        operator: key(40),
        buyer_countries: vec![country_code("NZ")],
        seller_countries: vec![country_code("ZA")],
        supply_countries: vec![country_code("NZ")],
        currencies: vec![Currency(*b"ZAR")],
        rail_classes: vec![RailClass::CustodialReversible],
        max_order_value: zar(500_000),
        excluded_categories: vec!["alcohol".into()],
        authorities: vec!["example authorisation".into()],
        declines: vec![],
    };
    roundtrip::assert_stable(&scope);
    roundtrip::assert_truncation_is_rejected(&scope);
    roundtrip::assert_trailing_garbage_is_rejected(&scope);

    for state in [
        EscrowState::Funded,
        EscrowState::Held,
        EscrowState::Released,
        EscrowState::Refunded,
        EscrowState::Split,
    ] {
        roundtrip::assert_stable(&state);
    }

    let ruling = EscrowRuling {
        operator: key(40),
        order: addr(50),
        state: EscrowState::Released,
        to_seller: zar(88_000),
        to_buyer: zar(0),
        reason: "delivered, no dispute".into(),
        at: Timestamp(1_700_200_000_000),
    };
    roundtrip::assert_stable(&ruling);
    roundtrip::assert_truncation_is_rejected(&ruling);
    roundtrip::assert_trailing_garbage_is_rejected(&ruling);
}

#[test]
fn trust_types_roundtrip_stably() {
    for attestor in [Attestor::Seller, Attestor::Escrow] {
        roundtrip::assert_stable(&attestor);
    }

    let purchase = PurchaseAttestation {
        attestor: Attestor::Escrow,
        issuer: key(40),
        order: addr(50),
        at: Timestamp(1_700_100_000_000),
    };
    roundtrip::assert_stable(&purchase);

    for subject in [
        Subject::Product(addr(1)),
        Subject::Seller(key(1)),
        Subject::Distributor(key(2)),
        Subject::Courier(key(3)),
    ] {
        roundtrip::assert_stable(&subject);
    }

    let review = Review {
        subject: Subject::Seller(key(1)),
        author: key(99),
        score: 5,
        body: "arrived intact".into(),
        attestation: Some(purchase),
        at: Timestamp(1_700_100_000_000),
    };
    roundtrip::assert_stable(&review);
    roundtrip::assert_truncation_is_rejected(&review);
    roundtrip::assert_trailing_garbage_is_rejected(&review);

    let unattested = Review {
        attestation: None,
        ..review
    };
    roundtrip::assert_stable(&unattested);
}

#[test]
fn jurisdiction_types_roundtrip_stably() {
    let anchors = Anchors {
        seller_establishment: country_code("ZA"),
        buyer_residence: country_code("NZ"),
        place_of_supply: country_code("NZ"),
        delivery_destination: Some(country_code("NZ")),
    };
    roundtrip::assert_stable(&anchors);
    roundtrip::assert_truncation_is_rejected(&anchors);
    roundtrip::assert_trailing_garbage_is_rejected(&anchors);

    let rep = InRegionRepresentative {
        region: country_code("DE"),
        who: key(7),
    };
    roundtrip::assert_stable(&rep);

    let parties = ResponsibleParties {
        seller_of_record: key(2),
        facilitator: Some(key(40)),
        importer_of_record: None,
        responsible_person: Some(rep),
    };
    roundtrip::assert_stable(&parties);
    roundtrip::assert_truncation_is_rejected(&parties);
    roundtrip::assert_trailing_garbage_is_rejected(&parties);
}

#[test]
fn gateway_types_roundtrip_stably() {
    for host in [
        StoreHost::Subdomain {
            label: "alice".into(),
            base: "soko.example".into(),
        },
        StoreHost::Custom("shop.example".into()),
    ] {
        roundtrip::assert_stable(&host);
    }

    let binding = StoreBinding {
        seller: key(1),
        host: StoreHost::Subdomain {
            label: "alice".into(),
            base: "soko.example".into(),
        },
    };
    roundtrip::assert_stable(&binding);
    roundtrip::assert_truncation_is_rejected(&binding);
    roundtrip::assert_trailing_garbage_is_rejected(&binding);
}
