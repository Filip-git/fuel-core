use fuel_core_storage::Result as StorageResult;
use fuel_core_types::{
    fuel_tx::UtxoId,
    fuel_types::{
        Address,
        BlockHeight,
    },
    fuel_vm::SecretKey,
};

#[cfg(feature = "std")]
use bech32::{
    ToBase32,
    Variant::Bech32m,
};
#[cfg(feature = "std")]
use core::str::FromStr;
#[cfg(feature = "std")]
use fuel_core_types::fuel_types::Bytes32;
#[cfg(feature = "std")]
use itertools::Itertools;
use serde::{
    Deserialize,
    Serialize,
};
use serde_with::{
    serde_as,
    skip_serializing_none,
};
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::path::Path;

use super::{
    coin::CoinConfig,
    contract::ContractConfig,
    message::MessageConfig,
};

// Fuel Network human-readable part for bech32 encoding
pub const FUEL_BECH32_HRP: &str = "fuel";
pub const TESTNET_INITIAL_BALANCE: u64 = 10_000_000;

pub const TESTNET_WALLET_SECRETS: [&str; 5] = [
    "0xde97d8624a438121b86a1956544bd72ed68cd69f2c99555b08b1e8c51ffd511c",
    "0x37fa81c84ccd547c30c176b118d5cb892bdb113e8e80141f266519422ef9eefd",
    "0x862512a2363db2b3a375c0d4bbbd27172180d89f23f2e259bac850ab02619301",
    "0x976e5c3fa620092c718d852ca703b6da9e3075b9f2ecb8ed42d9f746bf26aafb",
    "0x7f8a325504e7315eda997db7861c9447f5c3eff26333b20180475d94443a10c6",
];

pub const STATE_CONFIG_FILENAME: &str = "state_config.json";

// TODO: do streaming deserialization to handle large state configs
#[serde_as]
#[skip_serializing_none]
#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
pub struct StateConfig {
    /// Spendable coins
    pub coins: Option<Vec<CoinConfig>>,
    /// Contract state
    pub contracts: Option<Vec<ContractConfig>>,
    /// Messages from Layer 1
    pub messages: Option<Vec<MessageConfig>>,
}

impl StateConfig {
    pub fn generate_state_config<T>(db: T) -> StorageResult<Self>
    where
        T: ChainConfigDb,
    {
        Ok(StateConfig {
            coins: db.get_coin_config()?,
            contracts: db.get_contract_config()?,
            messages: db.get_message_config()?,
        })
    }

    #[cfg(feature = "std")]
    pub fn load_from_directory(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().join(STATE_CONFIG_FILENAME);

        let contents = std::fs::read(&path)?;
        serde_json::from_slice(&contents).map_err(|e| {
            anyhow::Error::new(e).context(format!(
                "an error occurred while loading the chain state file: {:?}",
                path.to_str()
            ))
        })
    }

    #[cfg(feature = "std")]
    pub fn create_config_file(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        use anyhow::Context;

        let state_writer = File::create(path.as_ref().join(STATE_CONFIG_FILENAME))?;

        serde_json::to_writer_pretty(state_writer, self)
            .context("failed to dump chain parameters snapshot to JSON")?;

        Ok(())
    }

    #[cfg(feature = "std")]
    pub fn local_testnet() -> Self {
        // endow some preset accounts with an initial balance
        tracing::info!("Initial Accounts");
        let initial_coins = TESTNET_WALLET_SECRETS
            .into_iter()
            .map(|secret| {
                let secret = SecretKey::from_str(secret).expect("Expected valid secret");
                let address = Address::from(*secret.public_key().hash());
                let bech32_data = Bytes32::new(*address).to_base32();
                let bech32_encoding =
                    bech32::encode(FUEL_BECH32_HRP, bech32_data, Bech32m).unwrap();
                tracing::info!(
                    "PrivateKey({:#x}), Address({:#x} [bech32: {}]), Balance({})",
                    secret,
                    address,
                    bech32_encoding,
                    TESTNET_INITIAL_BALANCE
                );
                Self::initial_coin(secret, TESTNET_INITIAL_BALANCE, None)
            })
            .collect_vec();

        Self {
            coins: Some(initial_coins),
            ..StateConfig::default()
        }
    }

    #[cfg(feature = "random")]
    pub fn random_testnet() -> Self {
        tracing::info!("Initial Accounts");
        let mut rng = rand::thread_rng();
        let initial_coins = (0..5)
            .map(|_| {
                let secret = SecretKey::random(&mut rng);
                let address = Address::from(*secret.public_key().hash());
                let bech32_data = Bytes32::new(*address).to_base32();
                let bech32_encoding =
                    bech32::encode(FUEL_BECH32_HRP, bech32_data, Bech32m).unwrap();
                tracing::info!(
                    "PrivateKey({:#x}), Address({:#x} [bech32: {}]), Balance({})",
                    secret,
                    address,
                    bech32_encoding,
                    TESTNET_INITIAL_BALANCE
                );
                Self::initial_coin(secret, TESTNET_INITIAL_BALANCE, None)
            })
            .collect_vec();

        Self {
            coins: Some(initial_coins),
            ..StateConfig::default()
        }
    }

    pub fn initial_coin(
        secret: SecretKey,
        amount: u64,
        utxo_id: Option<UtxoId>,
    ) -> CoinConfig {
        let address = Address::from(*secret.public_key().hash());

        CoinConfig {
            tx_id: utxo_id.as_ref().map(|u| *u.tx_id()),
            output_index: utxo_id.as_ref().map(|u| u.output_index()),
            tx_pointer_block_height: None,
            tx_pointer_tx_idx: None,
            maturity: None,
            owner: address,
            amount,
            asset_id: Default::default(),
        }
    }
}

pub trait ChainConfigDb {
    /// Returns *all* unspent coin configs available in the database.
    fn get_coin_config(&self) -> StorageResult<Option<Vec<CoinConfig>>>;
    /// Returns *alive* contract configs available in the database.
    fn get_contract_config(&self) -> StorageResult<Option<Vec<ContractConfig>>>;
    /// Returns *all* unspent message configs available in the database.
    fn get_message_config(&self) -> StorageResult<Option<Vec<MessageConfig>>>;
    /// Returns the last available block height.
    fn get_block_height(&self) -> StorageResult<BlockHeight>;
}

#[cfg(test)]
mod tests {
    use fuel_core_types::{
        blockchain::primitives::DaBlockHeight,
        fuel_asm::op,
        fuel_types::{
            AssetId,
            Bytes32,
        },
        fuel_vm::Contract,
    };
    use rand::{
        rngs::StdRng,
        Rng,
        RngCore,
        SeedableRng,
    };

    use crate::{
        CoinConfig,
        ContractConfig,
        MessageConfig,
    };

    #[cfg(feature = "std")]
    use std::env::temp_dir;

    use super::StateConfig;

    #[cfg(feature = "std")]
    #[test]
    fn can_roundtrip_write_read() {
        let tmp_file = temp_dir();
        let disk_config = StateConfig::local_testnet();

        disk_config.create_config_file(&tmp_file).unwrap();

        let load_config = StateConfig::load_from_directory(&tmp_file).unwrap();

        assert_eq!(disk_config, load_config);
    }

    #[test]
    fn snapshot_simple_contract() {
        let config = config_contract();
        let json = serde_json::to_string_pretty(&config).unwrap();
        insta::assert_snapshot!(json);
    }

    #[test]
    fn can_roundtrip_simple_contract() {
        let config = config_contract();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized_config: StateConfig =
            serde_json::from_str(json.as_str()).unwrap();
        assert_eq!(config, deserialized_config);
    }

    #[test]
    fn snapshot_contract_with_state() {
        let config = config_contract_with_state();
        let json = serde_json::to_string_pretty(&config).unwrap();
        insta::assert_snapshot!(json);
    }

    #[test]
    fn can_roundtrip_contract_with_state() {
        let config = config_contract_with_state();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized_config: StateConfig =
            serde_json::from_str(json.as_str()).unwrap();
        assert_eq!(config, deserialized_config);
    }

    #[test]
    fn snapshot_contract_with_balances() {
        let config = config_contract_with_balance();
        let json = serde_json::to_string_pretty(&config).unwrap();
        insta::assert_snapshot!(json);
    }

    #[test]
    fn can_roundtrip_contract_with_balances() {
        let config = config_contract_with_balance();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized_config: StateConfig =
            serde_json::from_str(json.as_str()).unwrap();
        assert_eq!(config, deserialized_config);
    }

    #[test]
    fn snapshot_contract_with_utxo_id() {
        let config = config_contract_with_utxoid();
        let json = serde_json::to_string_pretty(&config).unwrap();
        insta::assert_snapshot!(json);
    }

    #[test]
    fn can_roundtrip_contract_with_utxoid() {
        let config = config_contract_with_utxoid();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized_config: StateConfig =
            serde_json::from_str(json.as_str()).unwrap();
        assert_eq!(config, deserialized_config);
    }

    #[test]
    fn snapshot_contract_with_tx_pointer() {
        let config = config_contract_with_tx_pointer();
        let json = serde_json::to_string_pretty(&config).unwrap();
        insta::assert_snapshot!(json);
    }

    #[test]
    fn can_roundtrip_contract_with_tx_pointer() {
        let config = config_contract_with_tx_pointer();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized_config: StateConfig =
            serde_json::from_str(json.as_str()).unwrap();
        assert_eq!(config, deserialized_config);
    }

    #[test]
    fn snapshot_simple_coin_state() {
        let config = test_config_coin_state();
        let json = serde_json::to_string_pretty(&config).unwrap();
        insta::assert_snapshot!(json);
    }

    #[test]
    fn can_roundtrip_simple_coin_state() {
        let config = test_config_coin_state();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized_config: StateConfig =
            serde_json::from_str(json.as_str()).unwrap();
        assert_eq!(config, deserialized_config);
    }

    #[test]
    fn snapshot_simple_message_state() {
        let config = test_message_config();
        let json = serde_json::to_string_pretty(&config).unwrap();
        insta::assert_snapshot!(json);
    }

    #[test]
    fn can_roundtrip_simple_message_state() {
        let config = test_message_config();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized_config: StateConfig =
            serde_json::from_str(json.as_str()).unwrap();
        assert_eq!(config, deserialized_config);
    }

    fn config_contract_with_state() -> StateConfig {
        let mut rng = StdRng::seed_from_u64(1);
        let test_key: Bytes32 = rng.gen();
        let test_value: Bytes32 = rng.gen();
        let state = Some(vec![(test_key, test_value)]);

        StateConfig {
            contracts: Some(vec![ContractConfig {
                state,
                ..base_contract_config()
            }]),
            ..Default::default()
        }
    }

    fn config_contract_with_tx_pointer() -> StateConfig {
        let mut rng = StdRng::seed_from_u64(1);

        StateConfig {
            contracts: Some(vec![ContractConfig {
                tx_pointer_block_height: rng.gen(),
                tx_pointer_tx_idx: rng.gen(),
                ..base_contract_config()
            }]),
            ..Default::default()
        }
    }

    fn config_contract_with_utxoid() -> StateConfig {
        let mut rng = StdRng::seed_from_u64(1);

        StateConfig {
            contracts: Some(vec![ContractConfig {
                tx_id: rng.gen(),
                output_index: rng.gen(),
                ..base_contract_config()
            }]),
            ..Default::default()
        }
    }

    fn config_contract_with_balance() -> StateConfig {
        let mut rng = StdRng::seed_from_u64(1);
        let test_asset_id: AssetId = rng.gen();
        let test_balance: u64 = rng.gen();
        let balances = Some(vec![(test_asset_id, test_balance)]);

        StateConfig {
            contracts: Some(vec![ContractConfig {
                balances,
                ..base_contract_config()
            }]),
            ..Default::default()
        }
    }

    fn config_contract() -> StateConfig {
        StateConfig {
            contracts: Some(vec![ContractConfig {
                ..base_contract_config()
            }]),
            ..Default::default()
        }
    }

    fn base_contract_config() -> ContractConfig {
        ContractConfig {
            code: Contract::from(op::ret(0x10).to_bytes().to_vec()).into(),
            ..Default::default()
        }
    }

    fn test_config_coin_state() -> StateConfig {
        let mut rng = StdRng::seed_from_u64(1);
        let tx_id: Option<Bytes32> = Some(rng.gen());
        let output_index: Option<u8> = Some(rng.gen());
        let block_created = Some(rng.next_u32().into());
        let block_created_tx_idx = Some(rng.gen());
        let maturity = Some(rng.next_u32().into());
        let owner = rng.gen();
        let amount = rng.gen();
        let asset_id = rng.gen();

        StateConfig {
            coins: Some(vec![CoinConfig {
                tx_id,
                output_index,
                tx_pointer_block_height: block_created,
                tx_pointer_tx_idx: block_created_tx_idx,
                maturity,
                owner,
                amount,
                asset_id,
            }]),
            ..Default::default()
        }
    }

    fn test_message_config() -> StateConfig {
        let mut rng = StdRng::seed_from_u64(1);

        StateConfig {
            messages: Some(vec![MessageConfig {
                sender: rng.gen(),
                recipient: rng.gen(),
                nonce: rng.gen(),
                amount: rng.gen(),
                data: vec![rng.gen()],
                da_height: DaBlockHeight(rng.gen()),
            }]),
            ..Default::default()
        }
    }
}
