use fuel_core_types::fuel_types::{
    AssetId,
    Bytes32,
};
use serde::{
    Deserialize,
    Serialize,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct ContractBalanceConfig {
    pub contract_id: Bytes32,
    pub asset_id: AssetId,
    pub amount: u64,
}

#[cfg(all(test, feature = "random"))]
impl crate::Randomize for ContractBalanceConfig {
    fn randomize(mut rng: impl rand::Rng) -> Self {
        Self {
            contract_id: super::random_bytes_32(&mut rng).into(),
            asset_id: super::random_bytes_32(&mut rng).into(),
            amount: rng.gen(),
        }
    }
}
