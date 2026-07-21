//! # soko-order — cart, inventory, sealed orders (TRACT §6–§7)
//!
//! ## The cart is the buyer's
//!
//! A [`Cart`] is state on the buyer's own devices, synced between them by the substrate's Sync
//! capability. It is not a session on anyone's server. Consequences, all falling out of that one
//! decision: it survives any store closing; it spans sellers who have never heard of each other;
//! and **no party sees the whole cart** — each seller learns only its own lines, at order time.
//!
//! ## Oversell without a coordinator
//!
//! A seller running one node has no problem. A seller running several replicas — a shop counter, a
//! warehouse, a cloud node — needs stock decrements to converge without overselling. Last-write-wins
//! oversells; a strongly-consistent counter needs a coordinator, which is the thing being removed.
//!
//! [`BoundedCounter`] partitions stock into per-replica quotas. A replica sells freely within its
//! own quota with no coordination, and replicas transfer quota between themselves on demand. The
//! invariant — total sales never exceed total stock — holds without a lock.
//!
//! **The cost, stated rather than hidden:** a replica that is partitioned while holding unused
//! quota *strands* that stock until it rejoins. The counter trades availability for safety, which
//! is the correct trade for inventory but is not free.
//!
//! ## What the bounded counter does NOT protect against
//!
//! This is the boundary that matters most, because it is easy to overclaim. The escrow/demarcation
//! literature this follows (O'Neil 1986 → Barbará-Millá & Garcia-Molina 1994 → Balegas et al.,
//! SRDS 2015, shipped as AntidoteDB's `antidote_crdt_counter_b`) assumes a **crash-recovery failure
//! model with durable state — not a Byzantine one**.
//!
//! So: a lying or state-losing replica simply oversells. The guarantee covers **a seller's own
//! replicas, which trust each other**. It says nothing about stock held across mutually distrusting
//! parties, and it does **not** protect a buyer from a dishonest seller. Bounded counters protect a
//! seller from their own concurrency. Conflating those two would be a safety claim no result here
//! supports.
//!
//! Three further consequences, none of them removable by better code:
//!
//! - **A replica needs an internal serialization point** — a single writer, a compare-and-swap, or
//!   consensus. A replica cannot itself be a set of mutually uncoordinated nodes; the coordination
//!   is pushed down one level and made local, not eliminated.
//! - **Reclaiming a dead replica's quota is a fallible decision.** If the failure detector is
//!   wrong, quota is issued twice and the invariant breaks. [`BoundedCounter`] deliberately exposes
//!   no reclaim operation, because a safe one needs a policy this crate does not have.
//! - **Transfer names a recipient.** At three or more replicas it is not anonymous, which
//!   reintroduces an allocation policy — see [`BoundedCounter::transfer_to`], whose signature makes
//!   that explicit by requiring the recipient rather than releasing quota into a pool.
//!
//! ## No cross-seller atomicity
//!
//! A multi-seller cart is a set of independent orders. One seller declining does not roll back
//! another's acceptance, and there is no distributed transaction — that would need a coordinator
//! with authority over sovereign parties. Checkout is compensating actions, and an interface must
//! render per-seller status rather than a single "order placed" that is not true.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use soko_core::{ContentAddress, IdentityKey, Money, Timestamp};

/// A line in a buyer's cart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartLine {
    /// The offer being bought.
    pub offer: ContentAddress,
    /// Which seller published it — the key that splits the cart at checkout.
    pub seller: IdentityKey,
    /// How many.
    pub quantity: u32,
}

/// The buyer's cart. Held by the buyer, never by a seller or a gateway.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cart {
    /// Lines, across any number of independent sellers.
    pub lines: Vec<CartLine>,
}

impl Cart {
    /// Split the cart into one group per seller.
    ///
    /// This is the privacy property made mechanical: what leaves the buyer's device is one sealed
    /// order per seller containing only that seller's lines. The cross-seller view exists nowhere
    /// but here.
    pub fn split_by_seller(&self) -> Vec<(IdentityKey, Vec<CartLine>)> {
        let mut out: Vec<(IdentityKey, Vec<CartLine>)> = Vec::new();
        for line in &self.lines {
            match out.iter_mut().find(|(s, _)| s == &line.seller) {
                Some((_, v)) => v.push(line.clone()),
                None => out.push((line.seller.clone(), vec![line.clone()])),
            }
        }
        out
    }
}

/// Per-replica stock quota, so replicas sell without coordinating.
///
/// **The fields are private on purpose.** The invariant this type exists for — total sales never
/// exceed the stock that was partitioned — is only true if `quota + sold` is conserved, and public
/// fields would make every method optional: `counter.quota = u32::MAX` would conjure stock from
/// nothing without a single call. An invariant a caller has to remember to respect is not an
/// invariant, and this project's claim is that its invariants are structural.
///
/// Construct with [`BoundedCounter::new`], which is the only way rights enter the system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundedCounter {
    replica: IdentityKey,
    quota: u32,
    sold: u32,
}

/// Why a reservation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ReserveError {
    /// This replica's quota is exhausted. Stock may still exist elsewhere — the caller should try
    /// to acquire quota from another replica before telling a buyer it is out of stock.
    #[error("local quota exhausted; other replicas may still hold stock")]
    QuotaExhausted,
    /// A transfer named the same replica as source and destination, which would duplicate rights
    /// rather than move them.
    #[error("cannot transfer rights to the same replica")]
    SameReplica,
}

impl BoundedCounter {
    /// Partition `quota` units of rights to `replica`.
    ///
    /// This is the only way rights are created. Everything else moves or spends them, so the sum
    /// across every replica is whatever was issued here and never more.
    pub fn new(replica: IdentityKey, quota: u32) -> Self {
        Self {
            replica,
            quota,
            sold: 0,
        }
    }

    /// Which replica holds these rights.
    pub fn replica(&self) -> &IdentityKey {
        &self.replica
    }

    /// Units this replica may still sell on its own authority.
    pub fn quota(&self) -> u32 {
        self.quota
    }

    /// Units this replica has sold.
    pub fn sold(&self) -> u32 {
        self.sold
    }

    /// Rights held by this replica, spent or not. Conserved across every operation on this type,
    /// which is the property the private fields exist to protect.
    pub fn rights(&self) -> u32 {
        self.quota + self.sold
    }

    /// Take `n` units from this replica's quota, or fail.
    ///
    /// Never coordinates and never blocks. Failure means *this replica* cannot serve the sale, not
    /// that the seller is out of stock — conflating those is how a partitioned replica turns
    /// stranded quota into a lost sale.
    pub fn reserve(&mut self, n: u32) -> Result<(), ReserveError> {
        if n > self.quota {
            return Err(ReserveError::QuotaExhausted);
        }
        self.quota -= n;
        self.sold += n;
        Ok(())
    }

    /// Move quota to another replica. Conserves total rights by construction.
    ///
    /// The signature takes the recipient deliberately. At three or more replicas a transfer is not
    /// anonymous — somebody has to decide who receives the quota — and taking `&mut other` makes
    /// that allocation decision visible at the call site instead of hiding it behind a pool. There
    /// is no `release_into_pool` counterpart, because a pool would need an allocator, and an
    /// allocator is the coordinator this design is trying not to require.
    ///
    /// Transferring to a replica with the same identity is refused: it would appear to conserve
    /// rights while actually duplicating them across two counters that both believe they hold the
    /// quota.
    pub fn transfer_to(&mut self, other: &mut BoundedCounter, n: u32) -> Result<(), ReserveError> {
        if other.replica == self.replica {
            return Err(ReserveError::SameReplica);
        }
        if n > self.quota {
            return Err(ReserveError::QuotaExhausted);
        }
        self.quota -= n;
        other.quota += n;
        Ok(())
    }

    /// Release an unsold reservation back to this replica's quota.
    pub fn release(&mut self, n: u32) {
        let n = n.min(self.sold);
        self.sold -= n;
        self.quota += n;
    }
}

/// Where an order is in its lifecycle (§18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderState {
    /// Composed but not sent.
    Draft,
    /// Sent to the seller.
    Placed,
    /// Seller accepted.
    Accepted,
    /// Seller declined.
    Declined,
    /// Seller proposed different terms.
    Countered,
    /// Being fulfilled.
    Fulfilling,
    /// Delivered or performed.
    Delivered,
    /// Complete.
    Closed,
    /// Cancelled by either party.
    Cancelled,
}

/// A sealed order: one seller, that seller's lines only.
///
/// Never published. It carries what is needed to fulfil — which includes personal data — and so
/// exists only at the two endpoints, where it can actually be deleted (§0.5.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    /// Who is buying.
    pub buyer: IdentityKey,
    /// Who is selling.
    pub seller: IdentityKey,
    /// Lines for this seller.
    pub lines: Vec<CartLine>,
    /// Order total.
    pub total: Money,
    /// Current state.
    pub state: OrderState,
    /// When placed.
    pub placed: Timestamp,
}

impl soko_core::sealed::Sealed for Order {}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u8) -> IdentityKey {
        IdentityKey(vec![n])
    }
    fn line(seller: u8, qty: u32) -> CartLine {
        CartLine {
            offer: ContentAddress(vec![seller]),
            seller: key(seller),
            quantity: qty,
        }
    }

    /// The cart splits so each seller sees only its own lines — the cross-seller view never leaves
    /// the buyer's device.
    #[test]
    fn cart_splits_per_seller_and_nobody_sees_the_whole_thing() {
        let cart = Cart {
            lines: vec![line(1, 2), line(2, 1), line(1, 3)],
        };
        let groups = cart.split_by_seller();
        assert_eq!(groups.len(), 2);
        let seller1 = groups.iter().find(|(s, _)| s == &key(1)).unwrap();
        assert_eq!(seller1.1.len(), 2);
        let seller2 = groups.iter().find(|(s, _)| s == &key(2)).unwrap();
        assert_eq!(seller2.1.len(), 1);
    }

    /// The core safety property: two replicas selling concurrently, with no coordination, cannot
    /// between them sell more than the stock that was partitioned to them.
    #[test]
    fn independent_replicas_cannot_oversell() {
        let mut a = BoundedCounter::new(key(1), 6);
        let mut b = BoundedCounter::new(key(2), 4);
        // both sell hard, neither coordinates
        while a.reserve(1).is_ok() {}
        while b.reserve(1).is_ok() {}
        assert_eq!(
            a.sold() + b.sold(),
            10,
            "total sold must equal total stock, never exceed it"
        );
        assert!(a.reserve(1).is_err() && b.reserve(1).is_err());
    }

    /// Rights are conserved by construction, not by the caller remembering to use the methods.
    /// The fields are private precisely so this holds without cooperation — before that, a plain
    /// `counter.quota = u32::MAX` conjured stock from nothing with no call at all.
    #[test]
    fn rights_are_conserved_across_every_operation() {
        let mut a = BoundedCounter::new(key(1), 6);
        let mut b = BoundedCounter::new(key(2), 4);
        let issued = a.rights() + b.rights();
        a.reserve(3).unwrap();
        a.transfer_to(&mut b, 2).unwrap();
        b.reserve(5).unwrap();
        b.release(2);
        assert_eq!(
            a.rights() + b.rights(),
            issued,
            "no operation creates or destroys rights"
        );
        assert_eq!(issued, 10);
    }

    /// Transferring to yourself would look like conservation while actually duplicating rights
    /// across two counters that each believe they hold the quota.
    #[test]
    fn transfer_to_the_same_replica_is_refused() {
        let mut a = BoundedCounter::new(key(1), 5);
        let mut impostor = BoundedCounter::new(key(1), 0);
        assert_eq!(
            a.transfer_to(&mut impostor, 5),
            Err(ReserveError::SameReplica)
        );
        assert_eq!(a.quota(), 5, "the refused transfer moved nothing");
    }

    /// Quota transfer conserves the total — the mechanism that lets a busy replica keep selling
    /// without a coordinator.
    #[test]
    fn quota_transfer_conserves_total() {
        let mut a = BoundedCounter::new(key(1), 6);
        let mut b = BoundedCounter::new(key(2), 4);
        a.transfer_to(&mut b, 5).unwrap();
        assert_eq!(a.quota() + b.quota(), 10);
        assert_eq!((a.quota(), b.quota()), (1, 9));
    }

    /// The honest cost, asserted: a replica can report exhausted while stock still exists
    /// elsewhere. If this replica is partitioned, that stock is stranded until it rejoins.
    #[test]
    fn a_replica_can_be_exhausted_while_stock_remains_elsewhere() {
        let mut a = BoundedCounter::new(key(1), 6);
        let b = BoundedCounter::new(key(2), 4);
        while a.reserve(1).is_ok() {}
        assert_eq!(a.reserve(1), Err(ReserveError::QuotaExhausted));
        assert!(
            b.quota() > 0,
            "stock exists, but replica A cannot sell it without a transfer"
        );
    }

    /// Releasing an unsold reservation returns it to quota rather than losing it.
    #[test]
    fn release_returns_quota() {
        let mut a = BoundedCounter::new(key(1), 5);
        a.reserve(3).unwrap();
        a.release(2);
        assert_eq!((a.quota(), a.sold()), (4, 1));
    }
}
