//! # soko-node — the Soko node
//!
//! One binary, several roles: publish a catalogue, receive sealed orders, compute delivery
//! routing, serve public objects. Which roles are active is a run-time choice, not a build-time
//! one — the same shape the DMTAP substrate uses.
//!
//! The gateway role is deliberately **not** here. It terminates untrusted connections and renders
//! untrusted merchant bundles, so it runs as a separate process with no access to identity keys
//! (TRACT §12.4).

fn main() {
    println!(
        "soko-node {} — TRACT reference implementation\n\
         \n\
         Pre-alpha: the protocol is being written first. Nothing is wired yet.\n\
         Spec:  https://github.com/vul-os/tract\n\
         Roles: seller · buyer · courier · distributor · index (planned)\n\
         Note:  the gateway role runs as a separate process, never in this one.",
        env!("CARGO_PKG_VERSION")
    );
}
