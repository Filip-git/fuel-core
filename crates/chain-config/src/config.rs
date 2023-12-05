mod chain;
mod coin;
mod consensus;
mod contract;
mod contract_balance;
mod contract_state;
mod message;
mod state;

#[cfg(all(test, feature = "random"))]
pub(crate) fn random_bytes_32(rng: &mut impl rand::Rng) -> [u8; 32] {
    rng.gen()
}

pub use chain::*;
pub use coin::*;
pub use consensus::*;
pub use contract::*;
pub use contract_balance::*;
pub use contract_state::*;
pub use message::*;
pub use state::*;
