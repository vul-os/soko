# Offers: the four axes

Commerce specifications tend to model retail well and everything else badly, then accrete
category-specific extensions — a bookings module, a subscriptions module, a rentals module. TRACT
instead expresses **every** trade as one object with four orthogonal axes.

> An **offer** commits to a **Fulfilment** against **Consideration**, bounded by **Availability**,
> for an **Item**.

Each axis is a small closed set. Combined, they cover shapes that centralized platforms usually
need plugins for.

## Item

| Variant | Example |
|---|---|
| product | a tin of beans |
| variant-of-group | that shirt, in medium, in blue |
| service | a haircut, a consulting hour |
| right / licence | a font licence, a ticket, a membership |
| capacity | a seat, a room-night, a table |

## Availability

| Variant | Standard profiled | Example |
|---|---|---|
| count | — | 14 in stock |
| time slots | RFC 5545 `VAVAILABILITY` / `VFREEBUSY` | Tuesday 09:00–17:00, 30-minute slots |
| capacity per interval | RFC 5545 | 40 seats per sitting |
| unlimited | — | a digital download |
| made-to-order | — | lead time rather than stock |

Booking availability profiles iCalendar rather than inventing a calendar format, so a seller can
publish availability from the calendar software they already run.

## Fulfilment

| Variant | Example | Determines place of supply as |
|---|---|---|
| ship | a parcel | destination |
| collect | click-and-collect | the collection point |
| digital grant | a download or licence key | buyer's residence |
| perform-at-place | a haircut, an event | **the venue** |
| perform-remote | consulting over video | buyer's residence |
| access grant | a gym membership | the facility |
| return-required | a rental, a hire | collection point |

That last column is not decoration — see [Jurisdiction & tax](./jurisdiction.md). An event held in
Berlin is taxed where the event happens, regardless of where either party lives, and only the
Fulfilment axis knows that.

## Consideration

| Variant | Example |
|---|---|
| fixed | R120 |
| tiered / volume | cheaper per unit above 50 |
| recurring | R99/month |
| metered | per API call, per kWh |
| deposit + balance | 20% now, balance on delivery |
| quote-required (RFQ) | B2B contract pricing |

## Why this is not too generic to be useful

The test is whether the axes collapse real cases without special-casing. They do:

- **A rental**: item = product, availability = time slots, fulfilment = ship + return-required,
  consideration = fixed per period + deposit.
- **A made-to-measure suit**: item = product, availability = made-to-order, fulfilment = ship,
  consideration = deposit + balance.
- **A metered API**: item = right, availability = unlimited, fulfilment = access grant,
  consideration = metered.
- **A restaurant booking**: item = capacity, availability = capacity per interval, fulfilment =
  perform-at-place, consideration = fixed or none.

None of these needs a category-specific code path, and each one is a shape that mainstream
platforms handle with a paid extension or not at all.

## Geo-availability is part of the offer

An offer declares where it may sell to, which tax treatment attaches, and who the responsible
person is per region. A seller with no responsible person in a region that requires one simply
cannot construct a valid offer for it. The protocol expresses the constraint rather than leaving it
to become a legal problem later.
