//! `soko-storefront` — renders a seller's catalogue into ordinary HTML.
//!
//! This is the storefront half of the gateway role (TRACT §12): the surface that exists because a
//! browser cannot verify a signature. A TRACT-native client fetches the same signed objects and
//! checks them itself; a shopper with no keypair gets this instead, and trusts that it rendered
//! honestly.
//!
//! ## What it demonstrates, and what it does not
//!
//! It renders real [`soko_offer::Offer`], [`soko_catalog::ProductRecord`], [`soko_delivery`] and
//! [`soko_trust`] values through the same types the rest of the workspace uses — the four axes
//! decide what each listing says, the fulfilment axis decides the tax line, and the reviews are
//! weighted by [`soko_trust::Weighting`]. Nothing is hardcoded HTML pretending to be data.
//!
//! It does **not** yet fetch from a feed, verify a signature, or take an order. Those are the parts
//! that make it a gateway rather than a renderer, and they are not built. Said plainly here because
//! a screenshot of this page would otherwise imply a working store.
//!
//! ## Usage
//!
//! ```text
//! cargo run -p soko-gateway --bin soko-storefront -- > store.html
//! cargo run -p soko-gateway --bin soko-storefront -- --serve 8080
//! ```

use soko_catalog::{canonicalise, Attribute, IdentityRung, ProductRecord};
use soko_core::{ContentAddress, Country, Currency, IdentityKey, Money, Timestamp};
use soko_delivery::{Leg, Parcel, RateSource, RouteOption, RouteShape};
use soko_gateway::{StoreBinding, StoreHost};
use soko_offer::{Availability, Consideration, Fulfilment, Item, Offer, PlaceRef, StockSignal};
use soko_trust::{Attestor, PurchaseAttestation, Review, Subject, Weighting};
use std::io::{Read, Write};
use std::net::TcpListener;

const ZA: Country = Country(*b"ZA");
const DE: Country = Country(*b"DE");
const NZ: Country = Country(*b"NZ");
const ZAR: Currency = Currency(*b"ZAR");

fn zar(n: i64) -> Money {
    Money {
        minor_units: n,
        currency: ZAR,
    }
}

fn price(m: Money) -> String {
    format!(
        "R{}<span class=\"cents\">.{:02}</span>",
        m.minor_units / 100,
        (m.minor_units % 100).abs()
    )
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn cc(c: Country) -> String {
    String::from_utf8_lossy(&c.0).to_string()
}

/// One listing: the product record, the offer over it, and whatever reviews reference the seller.
struct Listing {
    record: ProductRecord,
    offer: Offer,
    seller: IdentityKey,
    reviews: Vec<Review>,
}

/// What the four axes say, in the words a shopper needs.
///
/// This is the whole argument for the axis model in one function: availability, fulfilment and
/// consideration each render independently, so a haircut and a tin of beans go through the same
/// code path and neither needs a category-specific branch.
impl Listing {
    fn availability_line(&self) -> String {
        match &self.offer.availability {
            Availability::Count(StockSignal::Exact(n)) => format!("{n} in stock"),
            Availability::Count(StockSignal::InStock) => "In stock".into(),
            Availability::Count(StockSignal::Low) => "Low stock".into(),
            Availability::Count(StockSignal::OutOfStock) => "Out of stock".into(),
            Availability::TimeSlots { slot_minutes, .. } => {
                format!("Bookable · {slot_minutes}-minute slots")
            }
            Availability::CapacityPerInterval { capacity, .. } => {
                format!("{capacity} places per session")
            }
            Availability::Unlimited => "Available now".into(),
            Availability::MadeToOrder { lead_days } => {
                format!("Made to order · ships in {lead_days} working days")
            }
        }
    }

    fn fulfilment_line(&self) -> String {
        match &self.offer.fulfilment {
            Fulfilment::Ship { to } => format!(
                "Ships to {}",
                to.iter().map(|c| cc(*c)).collect::<Vec<_>>().join(", ")
            ),
            Fulfilment::Collect { at } => format!("Collect in {}", esc(&at.locality)),
            Fulfilment::DigitalGrant => "Instant download".into(),
            Fulfilment::PerformAtPlace { at } => {
                format!("Held in {}, {}", esc(&at.locality), cc(at.country))
            }
            Fulfilment::PerformRemote => "Delivered remotely".into(),
            Fulfilment::AccessGrant { at: Some(at) } => format!("Access at {}", esc(&at.locality)),
            Fulfilment::AccessGrant { at: None } => "Access granted online".into(),
            Fulfilment::ReturnRequired { at, term_days } => {
                format!("Hire · {term_days} days · return to {}", esc(&at.locality))
            }
        }
    }

    fn price_line(&self) -> String {
        match &self.offer.consideration {
            Consideration::Fixed(m) => price(*m),
            Consideration::Tiered(tiers) => tiers
                .first()
                .map(|t| {
                    format!(
                        "{} <span class=\"from\">from {}+</span>",
                        price(t.unit_price),
                        t.min_qty
                    )
                })
                .unwrap_or_default(),
            Consideration::Recurring { amount, .. } => {
                format!("{}<span class=\"per\">/month</span>", price(*amount))
            }
            Consideration::Metered {
                dimension,
                unit_price,
            } => {
                format!(
                    "{}<span class=\"per\">/{}</span>",
                    price(*unit_price),
                    esc(dimension)
                )
            }
            Consideration::DepositBalance { deposit, balance } => format!(
                "{}<span class=\"per\">deposit</span><span class=\"then\">then {} on delivery</span>",
                price(*deposit),
                price(*balance)
            ),
            Consideration::QuoteRequired => "<span class=\"rfq\">Request a quote</span>".into(),
        }
    }

    /// Which jurisdiction taxes this line, derived from the fulfilment axis rather than from
    /// either party's country (§11.2). Shown on the page because for the event it is genuinely
    /// surprising, and a storefront that hides it is hiding the thing that decides the tax.
    fn supply_line(&self) -> String {
        let anchor = soko_jurisdiction_place(&self.offer.fulfilment);
        format!("Place of supply: {anchor}")
    }

    fn score(&self) -> Option<f32> {
        soko_trust::local_score(&self.reviews, Weighting::CONSERVATIVE)
    }
}

/// Local restatement of the §11.2 derivation, kept out of this binary's runtime dependency graph:
/// a demo renderer should not need the tax crate to print one line of prose.
///
/// The restatement is *not* excused from agreeing with the real thing. `soko-jurisdiction` is
/// pulled in as a **dev-dependency** below and every branch here is checked against
/// `soko_jurisdiction::place_of_supply` in the "soko_jurisdiction_place agreement" tests near the
/// bottom of this file's `tests` module — this is what "agrees with
/// `soko_jurisdiction::place_of_supply` by construction — same match,
/// same order" is actually enforced by, rather than merely claimed by. (An earlier version of this
/// comment said a dependency would create a cycle; checked while adding those tests, and it
/// wouldn't have — `soko-jurisdiction` depends only on `soko-core` and `soko-offer`, neither of
/// which depends back on `soko-gateway`. It stays out anyway, because "the storefront binary links
/// the tax crate" is not a runtime dependency this renderer should carry regardless.)
fn soko_jurisdiction_place(f: &Fulfilment) -> String {
    match f {
        Fulfilment::Ship { to } => to.first().map(|c| cc(*c)).unwrap_or_else(|| "—".into()),
        Fulfilment::Collect { at }
        | Fulfilment::PerformAtPlace { at }
        | Fulfilment::ReturnRequired { at, .. } => cc(at.country),
        Fulfilment::AccessGrant { at: Some(at) } => cc(at.country),
        Fulfilment::AccessGrant { at: None }
        | Fulfilment::DigitalGrant
        | Fulfilment::PerformRemote => "buyer's country".into(),
    }
}

/// The inputs to one demo listing.
///
/// A struct rather than nine positional arguments: the four axes are the whole point of this
/// page, and `listing(x, y, z, a, b, c, 1, true, vec![])` hides which value is which axis.
struct Seed<'a> {
    name: &'a str,
    desc: &'a str,
    attrs: &'a [(&'a str, &'a str)],
    availability: Availability,
    fulfilment: Fulfilment,
    consideration: Consideration,
    seller: u8,
    manufacturer_signed: bool,
    reviews: Vec<Review>,
}

fn listing(s: Seed<'_>) -> Listing {
    let Seed {
        name,
        desc,
        attrs,
        availability,
        fulfilment,
        consideration,
        seller,
        manufacturer_signed,
        reviews,
    } = s;
    let mut identity = vec![IdentityRung::ContentAddress(ContentAddress(vec![
        seller, 1,
    ]))];
    if manufacturer_signed {
        identity.push(IdentityRung::ManufacturerSigned(IdentityKey(vec![
            200 + seller,
        ])));
    }
    Listing {
        record: canonicalise(ProductRecord {
            name: name.into(),
            description: desc.into(),
            attributes: attrs
                .iter()
                .map(|(k, v)| Attribute {
                    key: (*k).into(),
                    value: (*v).into(),
                })
                .collect(),
            identity,
            group: None,
            components: vec![],
        }),
        offer: Offer {
            item: Item::Product(ContentAddress(vec![seller, 1])),
            availability,
            fulfilment,
            consideration,
            sell_to: vec![NZ, ZA, DE],
            published: Timestamp(1_784_000_000_000),
        },
        seller: IdentityKey(vec![seller]),
        reviews,
    }
}

fn review(score: u8, body: &str, attestor: Option<Attestor>, seller: u8) -> Review {
    Review {
        subject: Subject::Seller(IdentityKey(vec![seller])),
        author: IdentityKey(vec![90 + seller]),
        score,
        body: body.into(),
        attestation: attestor.map(|a| PurchaseAttestation {
            attestor: a,
            issuer: IdentityKey(vec![40]),
            order: ContentAddress(vec![50]),
            at: Timestamp(0),
        }),
        at: Timestamp(0),
    }
}

fn catalogue() -> Vec<Listing> {
    vec![
        listing(Seed {
            name: "Field Notebook",
            desc: "Pocket notebook, 90gsm, sewn binding.",
            attrs: &[("Colour", "black"), ("size", "A6")],
            availability: Availability::Count(StockSignal::Exact(14)),
            fulfilment: Fulfilment::Ship { to: vec![NZ, ZA] },
            consideration: Consideration::Fixed(zar(24_500)),
            seller: 1,
            manufacturer_signed: true,
            reviews: vec![
                review(
                    5,
                    "Arrived intact, exactly as described.",
                    Some(Attestor::Escrow),
                    1,
                ),
                review(4, "Good paper.", Some(Attestor::Seller), 1),
                review(1, "unrelated complaint", None, 1),
            ],
        }),
        listing(Seed {
            name: "Letterpress workshop",
            desc: "Two hours, small group, all materials included.",
            attrs: &[("duration", "2 hours")],
            availability: Availability::CapacityPerInterval {
                capacity: 8,
                ical: "FREQ=WEEKLY;BYDAY=SA".into(),
            },
            fulfilment: Fulfilment::PerformAtPlace {
                at: PlaceRef {
                    country: DE,
                    locality: "Berlin".into(),
                },
            },
            consideration: Consideration::Fixed(zar(78_000)),
            seller: 2,
            manufacturer_signed: false,
            reviews: vec![review(5, "Worth the trip.", Some(Attestor::Escrow), 2)],
        }),
        listing(Seed {
            name: "Scaffold tower hire",
            desc: "6m aluminium tower. Collection and return at the yard.",
            attrs: &[("height", "6 m")],
            availability: Availability::Count(StockSignal::Low),
            fulfilment: Fulfilment::ReturnRequired {
                at: PlaceRef {
                    country: ZA,
                    locality: "Durban".into(),
                },
                term_days: 7,
            },
            consideration: Consideration::DepositBalance {
                deposit: zar(150_000),
                balance: zar(45_000),
            },
            seller: 1,
            manufacturer_signed: false,
            reviews: vec![],
        }),
        listing(Seed {
            name: "Typeface licence",
            desc: "Single-seat desktop licence, perpetual.",
            attrs: &[("format", "OTF")],
            availability: Availability::Unlimited,
            fulfilment: Fulfilment::DigitalGrant,
            consideration: Consideration::Fixed(zar(112_000)),
            seller: 2,
            manufacturer_signed: true,
            reviews: vec![review(4, "Clean hinting.", Some(Attestor::Seller), 2)],
        }),
        listing(Seed {
            name: "Made-to-measure apron",
            desc: "Waxed canvas. Cut to your measurements.",
            attrs: &[("material", "waxed canvas")],
            availability: Availability::MadeToOrder { lead_days: 12 },
            fulfilment: Fulfilment::Ship {
                to: vec![NZ, ZA, DE],
            },
            consideration: Consideration::DepositBalance {
                deposit: zar(40_000),
                balance: zar(96_000),
            },
            seller: 1,
            manufacturer_signed: false,
            reviews: vec![],
        }),
        listing(Seed {
            name: "Bulk kraft boxes",
            desc: "Flat-packed, 200x140x60mm.",
            attrs: &[("pack", "100")],
            availability: Availability::Count(StockSignal::InStock),
            fulfilment: Fulfilment::Ship { to: vec![ZA] },
            consideration: Consideration::QuoteRequired,
            seller: 2,
            manufacturer_signed: false,
            reviews: vec![],
        }),
    ]
}

/// Render one listing to its `<article class="card">` fragment.
///
/// Pulled out of [`render`]'s map closure so tests can feed it a single [`Listing`] — including
/// one with attacker-shaped strings in the merchant-controlled fields — without going through the
/// fixed demo [`catalogue`]. Extracting this changes nothing about what [`render`] produces: same
/// body, called with the same arguments, in the same order.
fn render_card(l: &Listing) -> String {
    let attrs = l
        .record
        .attributes
        .iter()
        .map(|a| {
            format!(
                "<span class=\"attr\">{}: {}</span>",
                esc(&a.key),
                esc(&a.value)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let signed = l
        .record
        .identity
        .iter()
        .any(|r| matches!(r, IdentityRung::ManufacturerSigned(_)));
    let badge = if signed {
        "<span class=\"badge signed\" title=\"The manufacturer signed this product record, so a reseller cannot misdescribe it\">maker-signed</span>"
    } else {
        ""
    };
    let stars = match l.score() {
        Some(s) => format!(
            "<span class=\"score\">{s:.1}</span><span class=\"scorenote\">{} attested</span>",
            l.reviews.iter().filter(|r| r.attestation.is_some()).count()
        ),
        None => "<span class=\"scorenote\">no attested reviews</span>".into(),
    };
    format!(
        r#"<article class="card">
  <div class="cardtop">
    <h3>{name}</h3>{badge}
  </div>
  <p class="desc">{desc}</p>
  <div class="attrs">{attrs}</div>
  <div class="meta">
    <span class="avail">{avail}</span>
    <span class="fulfil">{fulfil}</span>
  </div>
  <div class="supply">{supply}</div>
  <div class="cardfoot">
    <div class="price">{price}</div>
    <div class="rating">{stars}</div>
  </div>
  <div class="seller">seller {seller:?}</div>
</article>"#,
        name = esc(&l.record.name),
        badge = badge,
        desc = esc(&l.record.description),
        attrs = attrs,
        avail = esc(&l.availability_line()),
        fulfil = l.fulfilment_line(),
        supply = esc(&l.supply_line()),
        price = l.price_line(),
        stars = stars,
        seller = l.seller.0,
    )
}

fn render() -> String {
    let items = catalogue();

    // Delivery is computed here, from published rate cards, exactly as a buyer's node would.
    let parcel = Parcel {
        length_mm: 220,
        width_mm: 140,
        height_mm: 30,
        weight_grams: 300,
    };
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

    let store = StoreBinding {
        seller: IdentityKey(vec![1]),
        host: StoreHost::Subdomain {
            label: "hollow-press".into(),
            base: "soko.example".into(),
        },
    };

    let cards = items.iter().map(render_card).collect::<Vec<_>>().join("\n");

    let routes = [&direct, &hub]
        .iter()
        .map(|r| {
            format!(
                "<tr><td>{:?}</td><td class=\"num\">{}</td><td class=\"num\">{} day(s)</td></tr>",
                r.shape,
                price(r.total().unwrap()),
                r.wait_days
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Hollow Press — a Soko storefront</title>
<style>
*,*::before,*::after{{box-sizing:border-box}}
:root{{--bg:#09090B;--panel:#131316;--panel2:#18181B;--border:rgba(255,255,255,.08);
--border2:rgba(255,255,255,.14);--text:#FAFAFA;--muted:#A1A1AA;--faint:#71717A;
--accent:#16D97F;--accent2:#0FA95F;
--sans:-apple-system,BlinkMacSystemFont,"Segoe UI",Inter,Roboto,Helvetica,Arial,sans-serif;
--mono:ui-monospace,SFMono-Regular,"SF Mono",Menlo,Consolas,monospace}}
body{{margin:0;background:var(--bg);color:var(--text);font-family:var(--sans);line-height:1.55;
-webkit-font-smoothing:antialiased}}
.wrap{{max-width:1100px;margin:0 auto;padding:0 24px}}
header.site{{border-bottom:1px solid var(--border);background:color-mix(in srgb,var(--bg) 80%,transparent)}}
.nav{{display:flex;align-items:center;gap:14px;height:66px}}
.mark{{width:30px;height:30px;border-radius:8px;background:linear-gradient(135deg,var(--accent),var(--accent2))}}
.storename{{font-weight:700;letter-spacing:-.02em;font-size:18px}}
.host{{font-family:var(--mono);font-size:12px;color:var(--faint);margin-left:2px}}
.spacer{{flex:1}}
.gwnote{{font-size:12px;color:var(--muted);border:1px solid var(--border2);padding:5px 11px;border-radius:999px}}
.hero{{padding:40px 0 10px}}
.hero h1{{margin:0;font-size:clamp(26px,4vw,38px);letter-spacing:-.03em}}
.hero p{{color:var(--muted);margin:10px 0 0;max-width:60ch}}
.grid{{display:grid;grid-template-columns:repeat(3,1fr);gap:16px;padding:28px 0 8px}}
@media(max-width:900px){{.grid{{grid-template-columns:1fr 1fr}}}}
@media(max-width:620px){{.grid{{grid-template-columns:1fr}}}}
.card{{background:linear-gradient(180deg,var(--panel),var(--bg));border:1px solid var(--border);
border-radius:14px;padding:18px;display:flex;flex-direction:column;gap:9px}}
.cardtop{{display:flex;align-items:flex-start;gap:8px;justify-content:space-between}}
.card h3{{margin:0;font-size:16.5px;letter-spacing:-.01em}}
.badge{{font-size:10.5px;font-weight:700;letter-spacing:.04em;text-transform:uppercase;
padding:3px 7px;border-radius:5px;white-space:nowrap}}
.badge.signed{{color:var(--accent);background:color-mix(in srgb,var(--accent) 13%,transparent);
border:1px solid color-mix(in srgb,var(--accent) 34%,transparent)}}
.desc{{margin:0;color:var(--muted);font-size:13.5px}}
.attrs{{display:flex;flex-wrap:wrap;gap:6px}}
.attr{{font-family:var(--mono);font-size:11px;color:var(--muted);background:rgba(255,255,255,.05);
border-radius:4px;padding:2px 6px}}
.meta{{display:flex;flex-direction:column;gap:3px;font-size:13px;margin-top:2px}}
.avail{{color:var(--text)}}
.fulfil{{color:var(--muted)}}
.supply{{font-family:var(--mono);font-size:11px;color:var(--faint);
border-top:1px dashed var(--border);padding-top:8px}}
.cardfoot{{display:flex;align-items:flex-end;justify-content:space-between;gap:14px;margin-top:auto;padding-top:6px}}
.price{{font-size:20px;font-weight:700;letter-spacing:-.02em}}
.price .cents{{font-size:13px;font-weight:600;color:var(--muted)}}
.price .per,.price .from{{font-size:11.5px;font-weight:500;color:var(--faint);margin-left:3px}}
.price .then{{display:block;font-size:11.5px;font-weight:500;color:var(--faint);margin-top:1px}}
.rfq{{font-size:15px;font-weight:600;color:var(--accent)}}
.rating{{text-align:right;line-height:1.25;flex:none;min-width:88px}}
.score{{display:block;font-weight:700;color:var(--accent);font-size:15px}}
.scorenote{{font-size:10.5px;color:var(--faint);display:block}}
.seller{{font-family:var(--mono);font-size:10.5px;color:var(--faint)}}
section.band{{margin:34px 0;background:linear-gradient(120deg,var(--panel),var(--bg));
border:1px solid var(--border);border-radius:16px;padding:24px}}
section.band h2{{margin:0 0 4px;font-size:19px;letter-spacing:-.02em}}
section.band p{{margin:0 0 14px;color:var(--muted);font-size:14px;max-width:70ch}}
table{{width:100%;border-collapse:collapse;font-size:13.5px}}
th,td{{text-align:left;padding:8px 10px;border-bottom:1px solid var(--border)}}
th{{color:var(--faint);font-weight:600;font-size:11px;letter-spacing:.06em;text-transform:uppercase}}
td.num{{font-family:var(--mono)}}
tr:last-child td{{border-bottom:0}}
.chosen td{{color:var(--accent)}}
footer.site{{border-top:1px solid var(--border);margin-top:30px;padding:22px 0 40px;
color:var(--faint);font-size:12.5px}}
footer.site code{{font-family:var(--mono);color:var(--muted)}}
</style></head><body>
<header class="site"><div class="wrap nav">
  <span class="mark"></span>
  <span class="storename">Hollow Press</span>
  <span class="host">{origin}</span>
  <span class="spacer"></span>
  <span class="gwnote">rendered by a gateway · not verified in your browser</span>
</div></header>
<main class="wrap">
  <div class="hero">
    <h1>Six things, one shape</h1>
    <p>A notebook, a workshop, a scaffold hire, a font licence, a made-to-measure apron and a bulk
    quote. Every one of them is the same offer object with different values on four axes — there is
    no product type, no booking module and no rentals plugin behind this page.</p>
  </div>

  <div class="grid">
{cards}
  </div>

  <section class="band">
    <h2>Delivery, computed here</h2>
    <p>Both options below were priced on this machine from published rate cards. No quote API was
    called, so no third party learned what is in the basket. The parcel bills at
    <strong>{billable}g</strong> — actual weight {actual}g, checked against volumetric.</p>
    <table>
      <tr><th>Route</th><th>Total</th><th>Buyer waits</th></tr>
      {routes}
    </table>
  </section>

  <section class="band">
    <h2>Why the workshop shows a different tax country</h2>
    <p>The letterpress workshop is held in Berlin. Neither the seller nor the buyer is German, and
    the place of supply is Germany anyway — because it is derived from the fulfilment axis, not
    from either party's country. A storefront that hid that line would be hiding the thing that
    decides the tax.</p>
  </section>
</main>
<footer class="site"><div class="wrap">
  Rendered by <code>soko-storefront</code> from real catalogue objects — the four axes, the routing
  comparison and the review weighting are all live calls into the workspace crates.
  <strong>Not yet a gateway:</strong> it does not fetch from a feed, verify a signature, or take an
  order. Pre-alpha.
</div></footer>
</body></html>"#,
        origin = esc(&store.host.origin()),
        cards = cards,
        billable = parcel.billable_grams(5000),
        actual = parcel.weight_grams,
        routes = routes,
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let html = render();

    if let Some(pos) = args.iter().position(|a| a == "--serve") {
        let port: u16 = args
            .get(pos + 1)
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind");
        eprintln!("soko-storefront on http://127.0.0.1:{port}");
        for stream in listener.incoming().flatten() {
            let mut s = stream;
            let mut buf = [0u8; 1024];
            let _ = s.read(&mut buf);
            let res = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                html.len(),
                html
            );
            let _ = s.write_all(res.as_bytes());
        }
    } else {
        print!("{html}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soko_offer::PriceTier;
    use std::collections::HashSet;

    // ---- shared builders -------------------------------------------------------------------
    //
    // A `Listing` needs all four struct fields even when a test only exercises one axis, so these
    // fill the other three with values that are inert for whichever line is under test.

    fn place_ref(country: Country, locality: &str) -> PlaceRef {
        PlaceRef {
            country,
            locality: locality.into(),
        }
    }

    fn record_with(name: &str, desc: &str, attrs: &[(&str, &str)]) -> ProductRecord {
        canonicalise(ProductRecord {
            name: name.into(),
            description: desc.into(),
            attributes: attrs
                .iter()
                .map(|(k, v)| Attribute {
                    key: (*k).into(),
                    value: (*v).into(),
                })
                .collect(),
            identity: vec![IdentityRung::ContentAddress(ContentAddress(vec![9]))],
            group: None,
            components: vec![],
        })
    }

    fn listing_for(
        availability: Availability,
        fulfilment: Fulfilment,
        consideration: Consideration,
    ) -> Listing {
        Listing {
            record: record_with("Test item", "A test description.", &[]),
            offer: Offer {
                item: Item::Product(ContentAddress(vec![9])),
                availability,
                fulfilment,
                consideration,
                sell_to: vec![ZA, DE, NZ],
                published: Timestamp(0),
            },
            seller: IdentityKey(vec![9]),
            reviews: vec![],
        }
    }

    fn simple_listing(availability: Availability, fulfilment: Fulfilment) -> Listing {
        listing_for(availability, fulfilment, Consideration::Fixed(zar(100)))
    }

    // ---- price() / esc() --------------------------------------------------------------------

    /// The one worked example the task's own screenshot depends on: 24_500 minor units of ZAR is
    /// two hundred and forty-five rand, not two hundred and forty-five point zero zero cents or
    /// any other off-by-a-hundred mistake an integer-division slip would produce silently.
    #[test]
    fn price_renders_minor_units_as_rand_and_cents() {
        assert_eq!(price(zar(24_500)), "R245<span class=\"cents\">.00</span>");
        assert_eq!(price(zar(1)), "R0<span class=\"cents\">.01</span>");
        assert_eq!(price(zar(100)), "R1<span class=\"cents\">.00</span>");
        assert_eq!(price(zar(0)), "R0<span class=\"cents\">.00</span>");
    }

    #[test]
    fn esc_escapes_the_three_html_metacharacters() {
        assert_eq!(esc("a & b"), "a &amp; b");
        assert_eq!(esc("<tag>"), "&lt;tag&gt;");
        assert_eq!(esc("plain text"), "plain text");
        assert_eq!(
            esc("<script>alert('x')</script>"),
            "&lt;script&gt;alert('x')&lt;/script&gt;"
        );
    }

    // ---- availability_line: every Availability variant, every StockSignal sub-case ----------

    /// Pinned wording per branch, not just "it returned something": a match arm that swaps two
    /// branches' text (e.g. `Low` and `OutOfStock` trading places) still produces non-empty,
    /// mutually distinct strings, so a distinctness-only check would not catch it. Exact text is
    /// the only thing that catches a wrong-but-plausible line.
    #[test]
    fn availability_line_covers_every_variant_with_the_expected_wording() {
        let line = |a| simple_listing(a, Fulfilment::DigitalGrant).availability_line();
        assert_eq!(
            line(Availability::Count(StockSignal::Exact(14))),
            "14 in stock"
        );
        assert_eq!(line(Availability::Count(StockSignal::InStock)), "In stock");
        assert_eq!(line(Availability::Count(StockSignal::Low)), "Low stock");
        assert_eq!(
            line(Availability::Count(StockSignal::OutOfStock)),
            "Out of stock"
        );
        assert_eq!(
            line(Availability::TimeSlots {
                ical: "FREQ=DAILY".into(),
                slot_minutes: 30,
            }),
            "Bookable · 30-minute slots"
        );
        assert_eq!(
            line(Availability::CapacityPerInterval {
                capacity: 12,
                ical: "FREQ=WEEKLY".into(),
            }),
            "12 places per session"
        );
        assert_eq!(line(Availability::Unlimited), "Available now");
        assert_eq!(
            line(Availability::MadeToOrder { lead_days: 5 }),
            "Made to order · ships in 5 working days"
        );
    }

    /// Belt-and-braces on top of the exact-wording test above: every one of the eight shapes
    /// (4 `StockSignal` sub-cases + 4 other `Availability` variants) must be non-empty and
    /// mutually distinct. An empty line is invisible on the card — indistinguishable from a field
    /// nobody wired up.
    #[test]
    fn every_availability_shape_is_nonempty_and_mutually_distinct() {
        let cases = [
            Availability::Count(StockSignal::Exact(3)),
            Availability::Count(StockSignal::InStock),
            Availability::Count(StockSignal::Low),
            Availability::Count(StockSignal::OutOfStock),
            Availability::TimeSlots {
                ical: "FREQ=DAILY".into(),
                slot_minutes: 15,
            },
            Availability::CapacityPerInterval {
                capacity: 4,
                ical: "FREQ=WEEKLY".into(),
            },
            Availability::Unlimited,
            Availability::MadeToOrder { lead_days: 2 },
        ];
        let mut seen = HashSet::new();
        for a in cases {
            let line = simple_listing(a, Fulfilment::DigitalGrant).availability_line();
            assert!(
                !line.is_empty(),
                "an empty availability line is invisible on the card"
            );
            assert!(
                seen.insert(line.clone()),
                "two different Availability shapes rendered identical text ({line:?}) — a \
                 shopper cannot tell them apart"
            );
        }
        assert_eq!(seen.len(), 8);
    }

    // ---- fulfilment_line: every Fulfilment variant, both AccessGrant sub-cases --------------

    #[test]
    fn fulfilment_line_covers_every_variant_with_the_expected_wording() {
        let line = |f| simple_listing(Availability::Unlimited, f).fulfilment_line();
        assert_eq!(
            line(Fulfilment::Ship { to: vec![NZ, ZA] }),
            "Ships to NZ, ZA"
        );
        assert_eq!(
            line(Fulfilment::Collect {
                at: place_ref(ZA, "Durban")
            }),
            "Collect in Durban"
        );
        assert_eq!(line(Fulfilment::DigitalGrant), "Instant download");
        assert_eq!(
            line(Fulfilment::PerformAtPlace {
                at: place_ref(DE, "Berlin")
            }),
            "Held in Berlin, DE"
        );
        assert_eq!(line(Fulfilment::PerformRemote), "Delivered remotely");
        assert_eq!(
            line(Fulfilment::AccessGrant {
                at: Some(place_ref(ZA, "Cape Town"))
            }),
            "Access at Cape Town"
        );
        assert_eq!(
            line(Fulfilment::AccessGrant { at: None }),
            "Access granted online"
        );
        assert_eq!(
            line(Fulfilment::ReturnRequired {
                at: place_ref(ZA, "Durban"),
                term_days: 7,
            }),
            "Hire · 7 days · return to Durban"
        );
    }

    /// The eight shapes a `Fulfilment` line can take (7 variants, `AccessGrant` split by
    /// `Option`) must all be non-empty and mutually distinguishable, for the same reason as
    /// `Availability` above.
    #[test]
    fn every_fulfilment_shape_is_nonempty_and_mutually_distinct() {
        let cases = [
            Fulfilment::Ship { to: vec![ZA] },
            Fulfilment::Collect {
                at: place_ref(ZA, "Durban"),
            },
            Fulfilment::DigitalGrant,
            Fulfilment::PerformAtPlace {
                at: place_ref(DE, "Berlin"),
            },
            Fulfilment::PerformRemote,
            Fulfilment::AccessGrant {
                at: Some(place_ref(ZA, "Cape Town")),
            },
            Fulfilment::AccessGrant { at: None },
            Fulfilment::ReturnRequired {
                at: place_ref(ZA, "Durban"),
                term_days: 3,
            },
        ];
        let mut seen = HashSet::new();
        for f in cases {
            let line = simple_listing(Availability::Unlimited, f).fulfilment_line();
            assert!(!line.is_empty());
            assert!(
                seen.insert(line.clone()),
                "two different Fulfilment shapes rendered identical text ({line:?})"
            );
        }
        assert_eq!(seen.len(), 8);
    }

    /// A locality is free text a merchant supplies (`PlaceRef::locality`), not a closed
    /// vocabulary like `Country`. Every branch that renders one must escape it, or a locality of
    /// `<img src=x onerror=alert(1)>` reaches the page as a live element instead of inert text.
    #[test]
    fn fulfilment_line_escapes_the_locality_in_every_branch_that_carries_one() {
        let hostile = "<img src=x onerror=alert(1)>";
        let branches = [
            Fulfilment::Collect {
                at: place_ref(ZA, hostile),
            },
            Fulfilment::PerformAtPlace {
                at: place_ref(DE, hostile),
            },
            Fulfilment::AccessGrant {
                at: Some(place_ref(ZA, hostile)),
            },
            Fulfilment::ReturnRequired {
                at: place_ref(ZA, hostile),
                term_days: 3,
            },
        ];
        for f in branches {
            let line = simple_listing(Availability::Unlimited, f).fulfilment_line();
            assert!(
                !line.contains(hostile),
                "unescaped locality reached fulfilment_line: {line}"
            );
            assert!(
                line.contains("&lt;img"),
                "expected the escaped form in: {line}"
            );
        }
    }

    // ---- price_line: every Consideration variant --------------------------------------------

    #[test]
    fn price_line_covers_every_variant_with_the_expected_wording() {
        let line =
            |c| listing_for(Availability::Unlimited, Fulfilment::DigitalGrant, c).price_line();
        assert_eq!(
            line(Consideration::Fixed(zar(24_500))),
            "R245<span class=\"cents\">.00</span>"
        );
        assert_eq!(
            line(Consideration::Tiered(vec![PriceTier {
                min_qty: 5,
                unit_price: zar(1_000),
            }])),
            "R10<span class=\"cents\">.00</span> <span class=\"from\">from 5+</span>"
        );
        assert_eq!(
            line(Consideration::Recurring {
                amount: zar(2_500),
                rrule: "FREQ=MONTHLY".into(),
            }),
            "R25<span class=\"cents\">.00</span><span class=\"per\">/month</span>"
        );
        assert_eq!(
            line(Consideration::Metered {
                dimension: "gigabytes".into(),
                unit_price: zar(150),
            }),
            "R1<span class=\"cents\">.50</span><span class=\"per\">/gigabytes</span>"
        );
        assert_eq!(
            line(Consideration::DepositBalance {
                deposit: zar(15_000),
                balance: zar(4_500),
            }),
            "R150<span class=\"cents\">.00</span><span class=\"per\">deposit</span>\
             <span class=\"then\">then R45<span class=\"cents\">.00</span> on delivery</span>"
        );
        assert_eq!(
            line(Consideration::QuoteRequired),
            "<span class=\"rfq\">Request a quote</span>"
        );
    }

    /// Six variants, all non-empty, all mutually distinct — same reasoning as the other two axes.
    #[test]
    fn every_consideration_variant_is_nonempty_and_mutually_distinct() {
        let cases = [
            Consideration::Fixed(zar(1_000)),
            Consideration::Tiered(vec![PriceTier {
                min_qty: 2,
                unit_price: zar(900),
            }]),
            Consideration::Recurring {
                amount: zar(500),
                rrule: "FREQ=MONTHLY".into(),
            },
            Consideration::Metered {
                dimension: "calls".into(),
                unit_price: zar(10),
            },
            Consideration::DepositBalance {
                deposit: zar(200),
                balance: zar(300),
            },
            Consideration::QuoteRequired,
        ];
        let mut seen = HashSet::new();
        for c in cases {
            let line =
                listing_for(Availability::Unlimited, Fulfilment::DigitalGrant, c).price_line();
            assert!(!line.is_empty());
            assert!(seen.insert(line));
        }
        assert_eq!(seen.len(), 6);
    }

    /// `dimension` in `Consideration::Metered` is merchant-supplied free text (what is being
    /// counted — "calls", "kWh", or whatever a seller types), so it must be escaped exactly like
    /// a product name or a locality.
    #[test]
    fn metered_dimension_is_escaped_in_price_line() {
        let l = listing_for(
            Availability::Unlimited,
            Fulfilment::DigitalGrant,
            Consideration::Metered {
                dimension: "</span><script>alert(1)</script>".into(),
                unit_price: zar(100),
            },
        );
        let line = l.price_line();
        assert!(!line.contains("<script>"), "unescaped dimension: {line}");
        assert!(line.contains("&lt;script&gt;"));
    }

    /// Honest documentation of a real gap, not a fix: `Consideration::Tiered` with no tiers
    /// renders an *empty* price line via `unwrap_or_default()`. That is the same failure shape as
    /// an unescaped string — a shopper sees nothing where a price should be, with no error
    /// anywhere in the pipeline — but no listing in `catalogue()` ever constructs an empty
    /// `Tiered`, and deciding what an empty tier list *should* say is a product call this test
    /// suite should not make unilaterally. Pinned so the behaviour cannot silently change either
    /// way without a test failing.
    #[test]
    fn tiered_consideration_with_no_tiers_renders_an_empty_price_line() {
        let l = listing_for(
            Availability::Unlimited,
            Fulfilment::DigitalGrant,
            Consideration::Tiered(vec![]),
        );
        assert_eq!(l.price_line(), "");
    }

    // ---- HTML escaping at the render_card / render level ------------------------------------

    /// The concrete injection the task calls out: a product name containing a `<script>` tag must
    /// come out inert. Merchant render bundles are untrusted input (§12.3), and a product name is
    /// exactly the shape of string an attacker-controlled merchant record can carry.
    #[test]
    fn render_card_neutralises_a_script_tag_in_the_product_name() {
        let mut l = simple_listing(Availability::Unlimited, Fulfilment::DigitalGrant);
        l.record.name = "<script>alert(document.cookie)</script>".into();
        let html = render_card(&l);
        assert!(
            !html.contains("<script>"),
            "a literal <script> tag reached the page: {html}"
        );
        assert!(html.contains("&lt;script&gt;alert(document.cookie)&lt;/script&gt;"));
    }

    /// Description and attribute key/value are the other two merchant-controlled `ProductRecord`
    /// fields the task names explicitly. All three must be inert in the same way the name is.
    #[test]
    fn render_card_neutralises_markup_in_description_and_attributes() {
        let mut l = simple_listing(Availability::Unlimited, Fulfilment::DigitalGrant);
        l.record.description = "<img src=x onerror=alert(1)>".into();
        l.record.attributes = vec![Attribute {
            key: "<b>bold-key</b>".into(),
            value: "\"><script>alert(2)</script>".into(),
        }];
        let html = render_card(&l);
        assert!(
            !html.contains("<img src=x"),
            "unescaped description: {html}"
        );
        assert!(
            !html.contains("<b>bold-key</b>"),
            "unescaped attribute key: {html}"
        );
        assert!(
            !html.contains("<script>alert(2)</script>"),
            "unescaped attribute value: {html}"
        );
        assert!(html.contains("&lt;img"));
        assert!(html.contains("&lt;b&gt;bold-key&lt;/b&gt;"));
        assert!(html.contains("&lt;script&gt;alert(2)&lt;/script&gt;"));
    }

    /// A finding from auditing every merchant-controlled string against `esc()` (per the task):
    /// none of `ProductRecord::name`, `::description`, or `Attribute::{key,value}` reach
    /// `render_card`'s output unescaped — the two tests above are the regression guard for that
    /// finding, not a fix, because nothing needed fixing.

    // ---- render(): shape of the full page ----------------------------------------------------

    #[test]
    fn render_produces_one_card_per_catalogue_listing() {
        let html = render();
        let n = catalogue().len();
        assert_eq!(html.matches("class=\"card\"").count(), n);
    }

    #[test]
    fn render_includes_a_place_of_supply_line_for_every_listing() {
        let html = render();
        let n = catalogue().len();
        assert_eq!(html.matches("Place of supply:").count(), n);
    }

    /// The worked example from the task: the Field Notebook is priced at 24_500 minor units of
    /// ZAR in `catalogue()`, which must reach the page as R245.00, not R2450.00 or R24.50.
    #[test]
    fn render_formats_the_notebook_price_as_two_hundred_and_forty_five_rand() {
        let html = render();
        assert!(html.contains("R245<span class=\"cents\">.00</span>"));
    }

    // ---- soko_jurisdiction_place agreement with the real §11.2 derivation -------------------
    //
    // `soko-jurisdiction` is a dev-dependency (tests only — see the doc comment on
    // `soko_jurisdiction_place`), so these compare the local restatement against the actual
    // `soko_jurisdiction::place_of_supply` function it claims to agree with, rather than merely
    // asserting the claim in prose.

    /// The three `Fulfilment` shapes where the real derivation returns exactly the place named in
    /// the fulfilment object (`Collect`, `PerformAtPlace`, `ReturnRequired`, and `AccessGrant`
    /// with a place) must match the local restatement byte-for-byte — there is no missing
    /// argument on either side that could excuse a difference here.
    #[test]
    fn stated_place_variants_match_the_real_derivation_exactly() {
        let at = place_ref(DE, "Berlin");
        let cases = [
            Fulfilment::Collect { at: at.clone() },
            Fulfilment::PerformAtPlace { at: at.clone() },
            Fulfilment::ReturnRequired {
                at: at.clone(),
                term_days: 5,
            },
            Fulfilment::AccessGrant {
                at: Some(at.clone()),
            },
        ];
        for f in cases {
            let shown = soko_jurisdiction_place(&f);
            let real = soko_jurisdiction::place_of_supply(&f, ZA, None)
                .expect("a stated place always resolves");
            assert_eq!(
                shown,
                cc(real),
                "{f:?} disagreed with the real §11.2 derivation"
            );
        }
    }

    /// `Ship` follows the buyer's *chosen* destination in the real derivation, which this
    /// storefront does not have (there is no live buyer at render time) — it shows the first
    /// ship-to territory as a stand-in. Feeding that same territory back into the real function
    /// as the chosen destination must agree exactly, or the stand-in is showing the wrong thing.
    #[test]
    fn ship_stand_in_matches_the_real_derivation_for_the_destination_it_shows() {
        let to = vec![NZ, ZA, DE];
        let f = Fulfilment::Ship { to: to.clone() };
        let shown = soko_jurisdiction_place(&f);
        let real = soko_jurisdiction::place_of_supply(&f, NZ, Some(to[0]))
            .expect("the first offered territory is always an accepted destination");
        assert_eq!(shown, cc(real));
    }

    /// An offer with no ship-to territories at all is the one input neither function can resolve
    /// — the local restatement says so with a dash, the real function refuses outright. Different
    /// shapes of "no answer", but both agree there is no answer.
    #[test]
    fn empty_ship_to_list_is_unresolvable_in_both_functions() {
        let f = Fulfilment::Ship { to: vec![] };
        assert_eq!(soko_jurisdiction_place(&f), "—");
        assert!(soko_jurisdiction::place_of_supply(&f, ZA, None).is_err());
    }

    /// `AccessGrant { at: None }`, `DigitalGrant` and `PerformRemote` are governed by buyer
    /// residence in the real derivation — nothing else. Proven here by showing the real function's
    /// answer tracks *whatever* residence is passed in, for all three, which is exactly the claim
    /// the local restatement's "buyer's country" placeholder makes.
    #[test]
    fn buyer_governed_variants_genuinely_track_buyer_residence_in_the_real_derivation() {
        for f in [
            Fulfilment::AccessGrant { at: None },
            Fulfilment::DigitalGrant,
            Fulfilment::PerformRemote,
        ] {
            assert_eq!(soko_jurisdiction::place_of_supply(&f, ZA, None), Ok(ZA));
            assert_eq!(soko_jurisdiction::place_of_supply(&f, DE, None), Ok(DE));
            assert_eq!(soko_jurisdiction_place(&f), "buyer's country");
        }
    }
}
