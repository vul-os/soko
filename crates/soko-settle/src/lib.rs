//! # soko-settle — payment attestations, escrow scope and lifecycle (TRACT §9)
//!
//! **The protocol carries attestations, never funds.** This crate models what crosses the
//! settlement boundary and what must be verified before it is believed. It specifies no rail, no
//! currency, no token and no ledger — those sit behind [`soko_seam::settle`], which names no
//! provider anywhere.
//!
//! ## Rail class is part of the type
//!
//! The most important property of a payment method, for commerce, is **what recourse it leaves the
//! buyer**. A reversible rail has chargebacks and a pending state; a final rail has neither. So the
//! class is never flattened to a boolean, and substituting one for the other is a decision for the
//! parties rather than an implementation detail (see [`RailSubstitution`]).
//!
//! This also resolves the escrow problem for a large class of trades without building anything: for
//! a stranger selling physical goods on a reversible rail, the card network's existing chargeback
//! machinery *is* the dispute system, and it is not yours to operate.
//!
//! ## Escrow is the operator class
//!
//! It requires legal standing, a payment-provider relationship, a float and jurisdiction-specific
//! licensing — none derivable from a keypair. What bounds it: permissionless entry, competition,
//! per-order choice by both parties, no access to identity keys, and **every ruling published as a
//! signed object**, so an operator that rules unfairly accumulates a permanent verifiable record.
//!
//! ## The measured failure mode, carried in the types
//!
//! OpenBazaar's escrow was opt-in and **bad actors simply declined it**. TRACT keeps escrow
//! optional for a good reason — mandatory escrow would exclude regions no licensed operator serves
//! — but that means an unescrowed trade must be an explicit, disclosed outcome rather than a
//! silent default. [`EscrowAvailability::None`] exists so the absence is a value the interface has
//! to render, not a `None` it can forget.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use soko_core::{ContentAddress, Country, Currency, IdentityKey, Money, Timestamp};
/// The wire representation of a rail's settlement class.
///
/// Deliberately **not** `soko_seam::settle::RailClass`. That type is an in-process contract and
/// `soko-seam` carries zero dependencies on purpose — adding `serde` there would push a dependency
/// onto every implementor of the seam, which is exactly what a seam exists to avoid. So the wire
/// type lives here, in TRACT's layer, and converts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RailClass {
    /// Custodial, reversible: chargebacks exist, so the card network is already the adjudicator.
    CustodialReversible,
    /// Non-custodial, final: nobody custodies and nothing reverses. The absence of recourse must be
    /// disclosed to the buyer before confirm, never discovered after.
    NonCustodialFinal,
}

impl From<soko_seam::settle::RailClass> for RailClass {
    fn from(c: soko_seam::settle::RailClass) -> Self {
        match c {
            soko_seam::settle::RailClass::CustodialReversible => RailClass::CustodialReversible,
            soko_seam::settle::RailClass::NonCustodialFinal => RailClass::NonCustodialFinal,
        }
    }
}

impl From<RailClass> for soko_seam::settle::RailClass {
    fn from(c: RailClass) -> Self {
        match c {
            RailClass::CustodialReversible => soko_seam::settle::RailClass::CustodialReversible,
            RailClass::NonCustodialFinal => soko_seam::settle::RailClass::NonCustodialFinal,
        }
    }
}

/// Proof that a payment happened. Carries a reference to external settlement — never funds, and
/// never card data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentAttestation {
    /// Who paid.
    pub payer: IdentityKey,
    /// Who was paid.
    pub payee: IdentityKey,
    /// The order this settles.
    pub order: ContentAddress,
    /// Amount settled.
    pub amount: Money,
    /// The rail's class, which determines the buyer's recourse.
    pub rail_class: RailClass,
    /// Opaque reference into the external settlement system. Meaningful only to the parties and
    /// the provider.
    pub external_ref: String,
    /// When settlement was confirmed.
    pub at: Timestamp,
}

/// Whether a rail substitution is permitted.
///
/// Failing a `NonCustodialFinal` request over to a `CustodialReversible` rail (or the reverse)
/// changes what happens when the trade goes wrong. It is never automatic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RailSubstitution {
    /// Both parties agreed to the substitution.
    Agreed,
    /// Not agreed — the request must fail rather than silently downgrade.
    Refused,
}

/// What an escrow operator can lawfully serve.
///
/// Escrow means holding client funds, which is licensed activity in most jurisdictions, so an
/// operator's reach is bounded by the authorisations it actually holds — not by preference. This
/// declaration is a signed public object: a false one is durable evidence rather than a deniable
/// claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscrowScope {
    /// The operator.
    pub operator: IdentityKey,
    /// Where consumers may be.
    pub buyer_countries: Vec<Country>,
    /// Where sellers may be.
    pub seller_countries: Vec<Country>,
    /// Where the supply may happen.
    pub supply_countries: Vec<Country>,
    /// Currencies it can settle.
    pub currencies: Vec<Currency>,
    /// Rail classes it supports.
    pub rail_classes: Vec<RailClass>,
    /// Ceiling above which it will not hold — usually a KYC threshold.
    pub max_order_value: Money,
    /// Categories refused.
    pub excluded_categories: Vec<String>,
    /// The authorisations claimed. Prose, because regulators do not share a schema.
    pub authorities: Vec<String>,
}

impl soko_core::public::Publishable for EscrowScope {}

/// The concrete trade being checked against a scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeContext {
    /// Where the buyer resides.
    pub buyer_country: Country,
    /// Where the seller is established.
    pub seller_country: Country,
    /// Where the supply happens — derived from the fulfilment axis, not from either party.
    pub supply_country: Country,
    /// Order value.
    pub value: Money,
    /// Rail class proposed.
    pub rail_class: RailClass,
    /// Category of the goods or service.
    pub category: String,
}

/// Whether escrow is available for a trade, and if not, why.
///
/// The `None` variant carries a reason because "no escrow" must be shown to both parties before
/// they commit, never silently applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscrowAvailability {
    /// This operator can serve the trade.
    Available(IdentityKey),
    /// No operator matched. The trade may still proceed, with disclosed risk.
    None(ScopeMismatch),
}

/// Why a scope did not match. Specific rather than a bare boolean, because the buyer deserves to
/// know whether the blocker is geography, money, or the goods themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeMismatch {
    /// The buyer's country is outside the operator's licence.
    BuyerCountry,
    /// The seller's country is outside it.
    SellerCountry,
    /// The place of supply is outside it.
    SupplyCountry,
    /// The operator cannot settle this currency.
    Currency,
    /// The operator does not support this rail class.
    RailClass,
    /// Above the operator's ceiling.
    ValueCeiling,
    /// The category is refused.
    Category,
}

impl EscrowScope {
    /// Check a trade against this scope, failing closed.
    ///
    /// Every field must match. A near-miss is a miss: an operator licensed for one region is not an
    /// option for a transaction in another, and the protocol expresses that rather than discovering
    /// it in a regulator's letter.
    pub fn check(&self, t: &TradeContext) -> EscrowAvailability {
        use ScopeMismatch as M;
        let fail = |m| EscrowAvailability::None(m);
        if !self.buyer_countries.contains(&t.buyer_country) {
            return fail(M::BuyerCountry);
        }
        if !self.seller_countries.contains(&t.seller_country) {
            return fail(M::SellerCountry);
        }
        if !self.supply_countries.contains(&t.supply_country) {
            return fail(M::SupplyCountry);
        }
        if !self.currencies.contains(&t.value.currency) {
            return fail(M::Currency);
        }
        if !self.rail_classes.contains(&t.rail_class) {
            return fail(M::RailClass);
        }
        if self.max_order_value.currency != t.value.currency
            || t.value.minor_units > self.max_order_value.minor_units
        {
            return fail(M::ValueCeiling);
        }
        if self.excluded_categories.iter().any(|c| c == &t.category) {
            return fail(M::Category);
        }
        EscrowAvailability::Available(self.operator.clone())
    }
}

/// Where funds sit in an escrowed trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EscrowState {
    /// Buyer has paid; the operator holds.
    Funded,
    /// Goods dispatched; still held.
    Held,
    /// Released to the seller.
    Released,
    /// Returned to the buyer.
    Refunded,
    /// Divided between the parties.
    Split,
}

/// A published escrow decision.
///
/// This is the accountability mechanism: because every ruling is a signed public object on the
/// operator's own feed, an operator that rules unfairly builds a permanent, verifiable record of
/// having done so — without needing an adjudicator standing above it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscrowRuling {
    /// The operator ruling.
    pub operator: IdentityKey,
    /// The order concerned.
    pub order: ContentAddress,
    /// The outcome.
    pub state: EscrowState,
    /// Amount to the seller.
    pub to_seller: Money,
    /// Amount to the buyer.
    pub to_buyer: Money,
    /// Stated reason. Public, and permanent.
    pub reason: String,
    /// When it was decided.
    pub at: Timestamp,
}

impl soko_core::public::Publishable for EscrowRuling {}

#[cfg(test)]
mod tests {
    use super::*;

    const ZA: Country = Country(*b"ZA");
    const DE: Country = Country(*b"DE");
    const ZAR: Currency = Currency(*b"ZAR");

    fn scope() -> EscrowScope {
        EscrowScope {
            operator: IdentityKey(vec![9]),
            buyer_countries: vec![ZA],
            seller_countries: vec![ZA],
            supply_countries: vec![ZA],
            currencies: vec![ZAR],
            rail_classes: vec![RailClass::CustodialReversible],
            max_order_value: Money {
                minor_units: 500_000,
                currency: ZAR,
            },
            excluded_categories: vec!["alcohol".into()],
            authorities: vec!["TPPP registration".into()],
        }
    }

    fn trade() -> TradeContext {
        TradeContext {
            buyer_country: ZA,
            seller_country: ZA,
            supply_country: ZA,
            value: Money {
                minor_units: 120_000,
                currency: ZAR,
            },
            rail_class: RailClass::CustodialReversible,
            category: "books".into(),
        }
    }

    #[test]
    fn in_scope_trade_is_served() {
        assert!(matches!(
            scope().check(&trade()),
            EscrowAvailability::Available(_)
        ));
    }

    /// The worked example from the spec: an operator licensed for one region must not be offered
    /// for a consumer in another. This is the case that motivated scope declarations at all.
    #[test]
    fn out_of_region_buyer_is_refused_not_stretched() {
        let mut t = trade();
        t.buyer_country = DE;
        assert_eq!(
            scope().check(&t),
            EscrowAvailability::None(ScopeMismatch::BuyerCountry)
        );
    }

    /// An event held abroad moves the place of supply even when both parties are local — the scope
    /// must be checked against supply, not just the parties.
    #[test]
    fn out_of_region_place_of_supply_is_refused() {
        let mut t = trade();
        t.supply_country = DE;
        assert_eq!(
            scope().check(&t),
            EscrowAvailability::None(ScopeMismatch::SupplyCountry)
        );
    }

    #[test]
    fn above_ceiling_is_refused() {
        let mut t = trade();
        t.value.minor_units = 900_000;
        assert_eq!(
            scope().check(&t),
            EscrowAvailability::None(ScopeMismatch::ValueCeiling)
        );
    }

    /// A final rail where the operator only supports reversible must fail, not substitute — the
    /// buyer's recourse differs, so it is the parties' decision.
    #[test]
    fn unsupported_rail_class_fails_closed() {
        let mut t = trade();
        t.rail_class = RailClass::NonCustodialFinal;
        assert_eq!(
            scope().check(&t),
            EscrowAvailability::None(ScopeMismatch::RailClass)
        );
    }

    #[test]
    fn excluded_category_is_refused() {
        let mut t = trade();
        t.category = "alcohol".into();
        assert_eq!(
            scope().check(&t),
            EscrowAvailability::None(ScopeMismatch::Category)
        );
    }

    /// A currency the operator cannot settle is a mismatch even if every other field passes.
    #[test]
    fn wrong_currency_is_refused() {
        let mut t = trade();
        t.value.currency = Currency(*b"EUR");
        assert_eq!(
            scope().check(&t),
            EscrowAvailability::None(ScopeMismatch::Currency)
        );
    }
}
