//! # soko-gateway — the storefront role (TRACT §12)
//!
//! A gateway renders signed catalogue objects into ordinary HTML over ordinary HTTPS, so a shopper
//! with no keypair can browse and buy. It is the **only operator class** in TRACT, together with
//! the settlement role it usually bundles (§9).
//!
//! ## Two rules that are not style advice
//!
//! **Origin isolation.** Merchant render bundles are untrusted code. If stores shared an origin,
//! one malicious bundle could read another store's cart and session. Every store gets its own
//! origin — see [`StoreBinding::origin_isolated_from`].
//!
//! **Process isolation.** A gateway terminates untrusted connections and renders untrusted bundles.
//! It runs as a separate process with no access to identity keys or the object store. "One binary,
//! several roles" never means one address space.
//!
//! ## The limit that does not go away
//!
//! DMTAP's mail gateway self-extinguishes as adoption grows. This one does not, because browsers
//! are permanent and a shopper without a keypair cannot verify a signature. The mitigation is
//! **detectability, not prevention**: any node can re-render the same store from the same signed
//! objects and be compared. A native client needs no gateway at all.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use soko_core::IdentityKey;

/// How a store is addressed on the web.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoreHost {
    /// A single-label subdomain of the gateway's own domain, from one wildcard certificate.
    /// Works instantly from a bare key, with no domain of the seller's own.
    Subdomain {
        /// The label, e.g. `alice` in `alice.example.com`.
        label: String,
        /// The gateway's base domain.
        base: String,
    },
    /// The seller's own domain, pointed at the gateway. Portable: identity is the keypair and the
    /// store is the feed, so repointing later changes neither.
    Custom(String),
}

impl StoreHost {
    /// The web origin this store is served from.
    pub fn origin(&self) -> String {
        match self {
            StoreHost::Subdomain { label, base } => format!("https://{label}.{base}"),
            StoreHost::Custom(d) => format!("https://{d}"),
        }
    }
}

/// A store as served by a gateway.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreBinding {
    /// Whose store it is. The gateway never holds this key's secret half.
    pub seller: IdentityKey,
    /// Where it is served.
    pub host: StoreHost,
}

impl StoreBinding {
    /// Whether this store is origin-isolated from another.
    ///
    /// A gateway serving two stores from one origin is **non-conformant**, not merely
    /// ill-advised: merchant bundles are untrusted code, and a shared origin lets one read the
    /// other's cart and session.
    pub fn origin_isolated_from(&self, other: &StoreBinding) -> bool {
        self.host.origin() != other.host.origin()
    }
}

/// What a gateway must be able to say about what it served.
///
/// Because any node can re-render the same store from the same signed objects, a gateway that
/// publishes the object addresses it rendered makes dishonest rendering *detectable* by anyone who
/// checks. That is the whole mitigation for the trust downgrade, so it is part of the contract
/// rather than an optional nicety.
pub trait RenderLog {
    /// The content addresses this gateway rendered for a given store.
    fn rendered(&self, seller: &IdentityKey) -> Vec<soko_core::ContentAddress>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(label: &str) -> StoreBinding {
        StoreBinding {
            seller: IdentityKey(label.as_bytes().to_vec()),
            host: StoreHost::Subdomain {
                label: label.into(),
                base: "soko.example".into(),
            },
        }
    }

    #[test]
    fn distinct_subdomains_are_origin_isolated() {
        assert!(binding("alice").origin_isolated_from(&binding("bob")));
    }

    /// Two stores on one origin is the failure this rule exists to prevent — one merchant's
    /// untrusted bundle reading another's cart.
    #[test]
    fn same_origin_two_stores_is_not_isolated() {
        let a = StoreBinding {
            seller: IdentityKey(vec![1]),
            host: StoreHost::Custom("shop.example".into()),
        };
        let b = StoreBinding {
            seller: IdentityKey(vec![2]),
            host: StoreHost::Custom("shop.example".into()),
        };
        assert!(!a.origin_isolated_from(&b));
    }

    /// A path is not an origin. Serving stores as /alice and /bob on one host shares a browser
    /// origin and therefore shares storage — the mistake this method exists to catch.
    #[test]
    fn subdomain_and_custom_domain_origins_differ() {
        let sub = binding("alice");
        let custom = StoreBinding {
            seller: IdentityKey(vec![9]),
            host: StoreHost::Custom("alice.example".into()),
        };
        assert!(sub.origin_isolated_from(&custom));
        assert_eq!(sub.host.origin(), "https://alice.soko.example");
    }
}
