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

/// Local restatement of the §11.2 derivation, to avoid a dependency cycle from the gateway crate.
/// It agrees with `soko_jurisdiction::place_of_supply` by construction — same match, same order.
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

    let cards = items
        .iter()
        .map(|l| {
            let attrs = l
                .record
                .attributes
                .iter()
                .map(|a| format!("<span class=\"attr\">{}: {}</span>", esc(&a.key), esc(&a.value)))
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
        })
        .collect::<Vec<_>>()
        .join("\n");

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
