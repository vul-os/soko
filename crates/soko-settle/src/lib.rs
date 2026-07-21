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
    /// Transaction shapes this operator declines regardless of everything above (D1).
    pub declines: Vec<DeclinedClass>,
}

impl soko_core::public::Publishable for EscrowScope {}

/// A class of transaction an operator declines to serve.
///
/// Distinct from `excluded_categories`, which is about *what* is sold. These are about the shape of
/// the transaction, and they exist because that is the shape tax law actually keys on.
///
/// The motivating case is recorded as decision D1: EU VAT Art 14a treats an electronic interface as
/// the deemed supplier for imported consignments below a value threshold, and for supplies within a
/// region by sellers not established in it. An operator that declines both is outside the rule on
/// its face — but until it can *declare* that, "we don't serve those" is an intention rather than
/// something a buyer's node can check before routing a trade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeclinedClass {
    /// Imported consignments into `region` whose intrinsic value is at or below `at_or_below`.
    LowValueImport {
        /// The destination region the rule attaches to.
        region: Country,
        /// The threshold, inclusive.
        at_or_below: Money,
    },
    /// Supplies into `region` by a seller not established in it.
    NonResidentSellerInto {
        /// The destination region.
        region: Country,
    },
}

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
    /// Whether the goods cross a border into the buyer's country.
    pub is_import: bool,
}

/// Whether escrow is available for a trade, and if not, why.
///
/// The `None` variant carries reasons — plural — because "no escrow" must be shown to both
/// parties before they commit, never silently applied, and a buyer told only the first of several
/// blockers has not actually been told why. A buyer informed "your country" when the currency was
/// also wrong still has to guess at the second reason, or worse, fix the first and be surprised by
/// the next one on retry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscrowAvailability {
    /// This operator can serve the trade.
    Available(IdentityKey),
    /// No operator matched. The trade may still proceed, with disclosed risk. Carries every
    /// blocking reason found, not just the first — see [`EscrowScope::check`].
    None(Vec<ScopeMismatch>),
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
    /// The transaction's shape is one this operator declines (D1).
    DeclinedClass,
}

impl EscrowScope {
    /// Check a trade against this scope, failing closed.
    ///
    /// Every field must match. A near-miss is a miss: an operator licensed for one region is not an
    /// option for a transaction in another, and the protocol expresses that rather than discovering
    /// it in a regulator's letter.
    ///
    /// Every declared field is consulted regardless of whether an earlier one already failed —
    /// this does **not** short-circuit on the first mismatch. §9.4's posture is that a scope
    /// mismatch is disclosed, and disclosing only the first of several simultaneous blockers is
    /// still a silent partial downgrade: a buyer told "buyer country" who fixes that and retries
    /// only then discovers "currency" has been true the whole time has been misled about how close
    /// the trade was to being served.
    pub fn check(&self, t: &TradeContext) -> EscrowAvailability {
        use ScopeMismatch as M;
        let mut mismatches = Vec::new();
        if !self.buyer_countries.contains(&t.buyer_country) {
            mismatches.push(M::BuyerCountry);
        }
        if !self.seller_countries.contains(&t.seller_country) {
            mismatches.push(M::SellerCountry);
        }
        if !self.supply_countries.contains(&t.supply_country) {
            mismatches.push(M::SupplyCountry);
        }
        if !self.currencies.contains(&t.value.currency) {
            mismatches.push(M::Currency);
        }
        if !self.rail_classes.contains(&t.rail_class) {
            mismatches.push(M::RailClass);
        }
        if self.max_order_value.currency != t.value.currency
            || t.value.minor_units > self.max_order_value.minor_units
        {
            mismatches.push(M::ValueCeiling);
        }
        if self.excluded_categories.iter().any(|c| c == &t.category) {
            mismatches.push(M::Category);
        }
        if self.declines.iter().any(|d| d.matches(t)) {
            mismatches.push(M::DeclinedClass);
        }
        if mismatches.is_empty() {
            EscrowAvailability::Available(self.operator.clone())
        } else {
            EscrowAvailability::None(mismatches)
        }
    }

    /// Validate the scope declaration itself, independent of any trade.
    ///
    /// `max_order_value` is a **price** in the sense [`soko_core::Money::price`] means: a ceiling
    /// this operator publishes, meant to be read by a buyer's interface as "escrow tops out here".
    /// A negative ceiling is not a valid ceiling — it is malformed input that happens to still
    /// "work" today, because [`EscrowScope::check`]'s `ValueCeiling` comparison
    /// (`t.value.minor_units > self.max_order_value.minor_units`) is true for essentially every
    /// real trade against a negative bound, so the scope fails closed by accident. That accidental
    /// safety is the wrong reason for it to be safe: every trade this operator is asked about would
    /// report a `ValueCeiling` mismatch as if it were merely too large, hiding that the scope
    /// itself was never usable and should have been rejected before publication.
    pub fn validate(&self) -> Result<(), soko_core::Error> {
        if self.max_order_value.is_negative() {
            return Err(soko_core::Error::NegativeAmount);
        }
        Ok(())
    }
}

impl DeclinedClass {
    /// Whether this declined class covers the trade.
    ///
    /// Deliberately conservative on the value threshold: `at_or_below` is inclusive, matching how
    /// the rules that motivate this are written. A trade sitting exactly on the threshold is
    /// declined, because being one cent inside a rule is being inside it.
    pub fn matches(&self, t: &TradeContext) -> bool {
        match self {
            DeclinedClass::LowValueImport {
                region,
                at_or_below,
            } => {
                t.is_import
                    && t.buyer_country == *region
                    && t.value.currency == at_or_below.currency
                    && t.value.minor_units <= at_or_below.minor_units
            }
            DeclinedClass::NonResidentSellerInto { region } => {
                t.buyer_country == *region && t.seller_country != *region
            }
        }
    }
}

impl EscrowScope {
    /// Narrow this scope by another — what the two can actually do together.
    ///
    /// §9.4 describes a buyer's node intersecting a gateway's declared scope with the operators
    /// and terms it trusts. That is a different operation from [`EscrowScope::check`], which tests
    /// one concrete trade for membership in one scope, and it had no implementation until the
    /// conformance vectors asked for it.
    ///
    /// `None` means the intersection is empty in at least one dimension — there is no trade the
    /// two would both accept, which §9.4 requires be disclosed rather than silently downgraded.
    ///
    /// **Every dimension narrows, and `excluded_categories` narrows by UNION rather than
    /// intersection.** That asymmetry is the whole subtlety: exclusions are prohibitions, so
    /// intersecting them would keep only the categories *both* parties refuse and silently permit
    /// everything one of them refuses alone. The value ceiling takes the lower of the two for the
    /// same reason — the more restrictive party governs.
    pub fn intersect(&self, other: &EscrowScope) -> Option<EscrowScope> {
        fn common<T: Copy + PartialEq>(a: &[T], b: &[T]) -> Vec<T> {
            a.iter().filter(|x| b.contains(x)).copied().collect()
        }

        let buyer_countries = common(&self.buyer_countries, &other.buyer_countries);
        let seller_countries = common(&self.seller_countries, &other.seller_countries);
        let supply_countries = common(&self.supply_countries, &other.supply_countries);
        let currencies = common(&self.currencies, &other.currencies);
        let rail_classes = common(&self.rail_classes, &other.rail_classes);

        if buyer_countries.is_empty()
            || seller_countries.is_empty()
            || supply_countries.is_empty()
            || currencies.is_empty()
            || rail_classes.is_empty()
        {
            return None;
        }

        // A ceiling in a currency neither side can settle is meaningless, so the lower ceiling is
        // only comparable when the two agree on its currency. Where they disagree, the surviving
        // currency set decides which ceiling still applies.
        let max_order_value = if self.max_order_value.currency == other.max_order_value.currency {
            Money {
                minor_units: self
                    .max_order_value
                    .minor_units
                    .min(other.max_order_value.minor_units),
                currency: self.max_order_value.currency,
            }
        } else if currencies.contains(&self.max_order_value.currency) {
            self.max_order_value
        } else {
            other.max_order_value
        };

        // UNION, not intersection — see the doc comment. Getting this backwards permits a category
        // one party explicitly refuses.
        let mut excluded_categories = self.excluded_categories.clone();
        for c in &other.excluded_categories {
            if !excluded_categories.contains(c) {
                excluded_categories.push(c.clone());
            }
        }

        let mut authorities = self.authorities.clone();
        authorities.extend(other.authorities.iter().cloned());

        // UNION, for the same reason exclusions union: a decline is a refusal, and intersecting
        // refusals would keep only what both parties decline and silently accept what one of them
        // will not touch.
        let mut declines = self.declines.clone();
        for d in &other.declines {
            if !declines.contains(d) {
                declines.push(d.clone());
            }
        }

        Some(EscrowScope {
            // The narrowed scope is still served by THIS operator; the other side is a policy, not
            // a second custodian.
            operator: self.operator.clone(),
            buyer_countries,
            seller_countries,
            supply_countries,
            currencies,
            rail_classes,
            max_order_value,
            excluded_categories,
            authorities,
            declines,
        })
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
            declines: vec![],
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
            is_import: false,
        }
    }

    /// Decision D1 made executable: an operator that declines low-value imports is outside EU VAT
    /// Art 14a on its face, and a buyer's node can now check that before routing a trade instead
    /// of taking the operator's word for it.
    #[test]
    fn a_declined_transaction_class_refuses_an_otherwise_valid_trade() {
        let eur = Currency(*b"EUR");
        let mut s = scope();
        s.currencies.push(eur);
        s.buyer_countries.push(DE);
        s.supply_countries.push(DE);
        s.max_order_value = Money {
            minor_units: 5_000_000,
            currency: ZAR,
        };
        s.declines = vec![DeclinedClass::LowValueImport {
            region: DE,
            at_or_below: Money {
                minor_units: 15_000,
                currency: eur,
            }, // EUR 150.00
        }];

        let low_value_import = TradeContext {
            buyer_country: DE,
            seller_country: ZA,
            supply_country: DE,
            value: Money {
                minor_units: 9_900,
                currency: eur,
            },
            rail_class: RailClass::CustodialReversible,
            category: "books".into(),
            is_import: true,
        };
        match s.check(&low_value_import) {
            EscrowAvailability::None(reasons) => {
                assert!(reasons.contains(&ScopeMismatch::DeclinedClass))
            }
            other => panic!("a declined class must refuse: {other:?}"),
        }
    }

    /// The same goods above the threshold are served — the decline is about the transaction's
    /// shape, not about the goods or the parties.
    #[test]
    fn the_same_trade_above_the_threshold_is_served() {
        let eur = Currency(*b"EUR");
        let mut s = scope();
        s.currencies.push(eur);
        s.buyer_countries.push(DE);
        s.supply_countries.push(DE);
        s.max_order_value = Money {
            minor_units: 5_000_000,
            currency: eur,
        };
        s.declines = vec![DeclinedClass::LowValueImport {
            region: DE,
            at_or_below: Money {
                minor_units: 15_000,
                currency: eur,
            },
        }];
        let above = TradeContext {
            buyer_country: DE,
            seller_country: ZA,
            supply_country: DE,
            value: Money {
                minor_units: 20_000,
                currency: eur,
            },
            rail_class: RailClass::CustodialReversible,
            category: "books".into(),
            is_import: true,
        };
        assert!(matches!(s.check(&above), EscrowAvailability::Available(_)));
    }

    /// Exactly on the threshold is inside it. Being one cent inside a rule is being inside it, and
    /// an exclusive comparison here would put an operator marginally in scope while believing
    /// itself out of it.
    #[test]
    fn the_value_threshold_is_inclusive() {
        let eur = Currency(*b"EUR");
        let d = DeclinedClass::LowValueImport {
            region: DE,
            at_or_below: Money {
                minor_units: 15_000,
                currency: eur,
            },
        };
        let at = TradeContext {
            buyer_country: DE,
            seller_country: ZA,
            supply_country: DE,
            value: Money {
                minor_units: 15_000,
                currency: eur,
            },
            rail_class: RailClass::CustodialReversible,
            category: "books".into(),
            is_import: true,
        };
        assert!(d.matches(&at));
    }

    /// A domestic sale is not an import, so the low-value decline does not touch it — which is the
    /// whole point of D1: the wedge market is outside the rule already.
    #[test]
    fn a_domestic_trade_is_untouched_by_an_import_decline() {
        let d = DeclinedClass::LowValueImport {
            region: ZA,
            at_or_below: Money {
                minor_units: 15_000,
                currency: ZAR,
            },
        };
        assert!(!d.matches(&trade()), "domestic ZA->ZA is not an import");
    }

    /// The operation §9.4 describes and the code did not have: a gateway's declared scope narrowed
    /// by what the buyer will accept.
    #[test]
    fn intersection_narrows_every_dimension() {
        let gateway = scope();
        let buyer_policy = EscrowScope {
            operator: IdentityKey(vec![0]),
            buyer_countries: vec![ZA, DE],
            seller_countries: vec![ZA],
            supply_countries: vec![ZA, DE],
            currencies: vec![ZAR, Currency(*b"EUR")],
            rail_classes: vec![RailClass::CustodialReversible],
            max_order_value: Money {
                minor_units: 100_000,
                currency: ZAR,
            },
            excluded_categories: vec!["weapons".into()],
            authorities: vec![],
            declines: vec![],
        };
        let n = gateway.intersect(&buyer_policy).expect("these overlap");
        assert_eq!(
            n.buyer_countries,
            vec![ZA],
            "only the common country survives"
        );
        assert_eq!(n.currencies, vec![ZAR]);
        assert_eq!(
            n.max_order_value.minor_units, 100_000,
            "the LOWER ceiling governs — the more restrictive party wins"
        );
        assert_eq!(
            n.operator, gateway.operator,
            "the custodian is still the gateway"
        );
    }

    /// The subtlety worth a test of its own. Exclusions are prohibitions, so they UNION.
    /// Intersecting them would keep only what both parties refuse and silently permit alcohol —
    /// which the gateway refuses — merely because the buyer had no opinion about it.
    #[test]
    fn exclusions_union_rather_than_intersect() {
        let gateway = scope(); // excludes "alcohol"
        let buyer_policy = EscrowScope {
            excluded_categories: vec!["weapons".into()],
            ..scope()
        };
        let n = gateway.intersect(&buyer_policy).unwrap();
        assert!(n.excluded_categories.contains(&"alcohol".to_string()));
        assert!(n.excluded_categories.contains(&"weapons".to_string()));
        assert_eq!(n.excluded_categories.len(), 2, "both prohibitions survive");
    }

    /// No overlap in even one dimension means no trade the two would both accept — which §9.4
    /// requires be disclosed, not silently downgraded to something weaker.
    #[test]
    fn empty_intersection_is_none_rather_than_a_permissive_scope() {
        let gateway = scope();
        let incompatible = EscrowScope {
            buyer_countries: vec![DE],
            ..scope()
        };
        assert!(gateway.intersect(&incompatible).is_none());
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
            EscrowAvailability::None(vec![ScopeMismatch::BuyerCountry])
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
            EscrowAvailability::None(vec![ScopeMismatch::SupplyCountry])
        );
    }

    #[test]
    fn above_ceiling_is_refused() {
        let mut t = trade();
        t.value.minor_units = 900_000;
        assert_eq!(
            scope().check(&t),
            EscrowAvailability::None(vec![ScopeMismatch::ValueCeiling])
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
            EscrowAvailability::None(vec![ScopeMismatch::RailClass])
        );
    }

    #[test]
    fn excluded_category_is_refused() {
        let mut t = trade();
        t.category = "alcohol".into();
        assert_eq!(
            scope().check(&t),
            EscrowAvailability::None(vec![ScopeMismatch::Category])
        );
    }

    /// A currency the operator cannot settle is a mismatch even if every other field passes.
    ///
    /// It also, correctly, makes the value ceiling impossible to evaluate: `max_order_value` is
    /// denominated in the operator's currency, and a trade in a currency the operator does not
    /// even list can't be compared against it. Both are true at once and both must be reported —
    /// this is the concrete case that motivated collecting every mismatch instead of stopping at
    /// the first.
    #[test]
    fn wrong_currency_is_refused() {
        let mut t = trade();
        t.value.currency = Currency(*b"EUR");
        assert_eq!(
            scope().check(&t),
            EscrowAvailability::None(vec![ScopeMismatch::Currency, ScopeMismatch::ValueCeiling])
        );
    }

    /// The finding this section exists to fix: `check` used to return only the first blocking
    /// reason it found, so a buyer told "buyer country" never learned the rail class was also
    /// wrong. Against the OLD short-circuiting implementation this would have observed just
    /// `[BuyerCountry]` and failed. Every simultaneous mismatch must now be reported together, in
    /// field-declaration order, so a buyer sees the whole picture on the first try rather than
    /// discovering blockers one retry at a time.
    #[test]
    fn multiple_simultaneous_mismatches_are_all_reported() {
        let mut t = trade();
        t.buyer_country = DE;
        t.rail_class = RailClass::NonCustodialFinal;
        t.category = "alcohol".into();
        assert_eq!(
            scope().check(&t),
            EscrowAvailability::None(vec![
                ScopeMismatch::BuyerCountry,
                ScopeMismatch::RailClass,
                ScopeMismatch::Category,
            ])
        );
    }

    /// A scope whose declared ceiling is negative is malformed input, not a stricter scope — see
    /// [`EscrowScope::validate`] for why leaving it to `check`'s ordinary `ValueCeiling` path would
    /// hide the real problem behind a misleading "too large" reading on every trade.
    #[test]
    fn validate_rejects_a_negative_ceiling() {
        let mut s = scope();
        s.max_order_value.minor_units = -1;
        assert!(matches!(
            s.validate(),
            Err(soko_core::Error::NegativeAmount)
        ));
    }

    #[test]
    fn validate_accepts_a_well_formed_scope() {
        assert!(scope().validate().is_ok());
    }
}
