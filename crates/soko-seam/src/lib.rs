//! # soko-seam — the traits Soko refuses to decide for you
//!
//! TRACT specifies a *seam* at every point where a real-world institution has to be chosen:
//! who settles money, who holds goods in dispute, who renders a store to a browser. This crate
//! is those seams and nothing else.
//!
//! **It names no provider anywhere.** Not Stripe, not Paystack, not Peach, not patala, not a
//! chain. That is deliberate and load-bearing: the moment a protocol crate names a provider,
//! every implementor inherits that provider's jurisdiction, licensing, and politics. An
//! operator wiring their own PSP writes a small impl and never depends on anything here beyond
//! the traits.
//!
//! This crate has, and will keep, **zero dependencies**.
//!
//! ## Why a seam and not an implementation
//!
//! TRACT §0.4.2 confines the protocol's only operator class to the gateway role, because
//! settlement and custody need scarce resources — legal standing, a payment-provider
//! relationship, a float, jurisdiction-specific licensing — that cannot be derived from a
//! keypair. A protocol crate cannot supply those, so it must not pretend to. What it can do is
//! specify exactly what crosses the boundary, and make a failure to verify fail closed.
//!
//! ## Status
//!
//! Scaffold. The trait shapes below are placeholders pending TRACT §9 and §12; they are here so
//! the dependency direction is fixed from the first commit — everything depends on the seam,
//! the seam depends on nothing.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Settlement seam (TRACT §9).
///
/// The classification is **in the type**, never flattened to a bool, because it changes the
/// buyer's recourse and therefore the UX contract: a reversible rail has chargebacks and a
/// pending state; a final rail has neither and the buyer bears the risk.
pub mod settle {
    /// How a rail settles, and what recourse it leaves the buyer.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RailClass {
        /// Custodial, reversible (chargebacks possible), usually KYC'd, delayed settlement.
        /// The card network is the adjudicator — which is precisely why TRACT does not need
        /// to build one for this class of trade (§9.6).
        CustodialReversible,
        /// Non-custodial, final, wallet-to-wallet, near-instant. Nobody custodies and nobody
        /// can reverse it; the absence of recourse MUST be disclosed to the buyer before
        /// confirm, never discovered after (§9.2).
        NonCustodialFinal,
    }
}

/// Escrow seam (TRACT §9.6).
///
/// Escrow is the one place TRACT permits an operator to hold funds. Every decision an escrow
/// operator makes is a **signed public object** published to its own feed, so an operator that
/// rules unfairly accumulates a permanent, verifiable record of having done so. That is the
/// accountability mechanism, and it works without an adjudicator standing above the operator.
pub mod escrow {}

/// Storefront seam (TRACT §12).
///
/// A browser cannot verify a signature, so a shopper without a keypair trusts the gateway to
/// have rendered honestly. This is a real trust downgrade and TRACT discloses it as one; the
/// mitigation is that any node can re-render the same store from the same signed objects and
/// be compared byte-for-byte, not that the trust is absent.
pub mod storefront {}
