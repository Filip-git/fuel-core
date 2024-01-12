use std::sync::Arc;

use super::{
    init_coin,
    init_contract,
    init_da_message,
    runner::{
        HandlesGenesisResource,
        ProcessState,
        ProcessStateGroup,
    },
    GenesisRunner,
};

use crate::database::{
    genesis_progress::GenesisResource,
    Database,
};
use fuel_core_chain_config::{
    CoinConfig,
    ContractBalanceConfig,
    ContractConfig,
    ContractStateConfig,
    GenesisCommitment,
    Group,
    MessageConfig,
    StateReader,
};
use fuel_core_executor::refs::ContractRef;
use fuel_core_types::fuel_types::{
    BlockHeight,
    ContractId,
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

pub struct StopSignals {
    signals: Vec<Arc<Notify>>,
}

impl StopSignals {
    pub fn new() -> Self {
        Self { signals: vec![] }
    }

    pub fn add(&mut self) -> Arc<Notify> {
        let signal = Arc::new(Notify::new());
        self.signals.push(signal.clone());
        signal
    }
}

impl Iterator for StopSignals {
    type Item = Arc<Notify>;

    fn next(&mut self) -> Option<Self::Item> {
        self.signals.pop()
    }
}

pub struct GenesisWorkers {
    db: Database,
    cancel_token: CancellationToken,
    block_height: BlockHeight,
    state_reader: StateReader,
}

impl GenesisWorkers {
    pub fn new(
        db: Database,
        cancel_token: CancellationToken,
        block_height: BlockHeight,
        state_reader: StateReader,
    ) -> Self {
        Self {
            db,
            cancel_token,
            block_height,
            state_reader,
        }
    }

    pub async fn spawn_coins_worker(
        &self,
        stop_signal: Arc<Notify>,
    ) -> anyhow::Result<()> {
        let coins = self.state_reader.coins().unwrap();
        self.spawn_worker(coins, stop_signal).await
    }

    pub async fn spawn_messages_worker(
        &self,
        stop_signal: Arc<Notify>,
    ) -> anyhow::Result<()> {
        let messages = self.state_reader.messages().unwrap();
        self.spawn_worker(messages, stop_signal).await
    }

    pub async fn spawn_contracts_worker(
        &self,
        stop_signal: Arc<Notify>,
    ) -> anyhow::Result<()> {
        let contracts = self.state_reader.contracts().unwrap();
        self.spawn_worker(contracts, stop_signal).await
    }

    pub async fn spawn_contract_state_worker(
        &self,
        stop_signal: Arc<Notify>,
    ) -> anyhow::Result<()> {
        let contract_state = self.state_reader.contract_state().unwrap();
        self.spawn_worker(contract_state, stop_signal).await
    }

    pub async fn spawn_contract_balance_worker(
        &self,
        stop_signal: Arc<Notify>,
    ) -> anyhow::Result<()> {
        let contract_balance = self.state_reader.contract_balance().unwrap();
        self.spawn_worker(contract_balance, stop_signal).await
    }

    pub async fn spawn_contracts_root_worker(self) -> anyhow::Result<()> {
        tokio_rayon::spawn(move || {
            let chunks = self.db.genesis_contract_ids_iter();

            let contract_ids = chunks.into_iter().enumerate().map(
                |(index, chunk)| -> anyhow::Result<_> {
                    let data = vec![chunk?];
                    Ok(Group { index, data })
                },
            );

            self.create_runner(contract_ids, None).run()
        })
        .await
    }

    fn spawn_worker<T, I>(
        &self,
        data: I,
        stop_signal: Arc<Notify>,
    ) -> tokio_rayon::AsyncRayonHandle<Result<(), anyhow::Error>>
    where
        Handler: ProcessStateGroup<T>,
        T: HandlesGenesisResource,
        I: IntoIterator<Item = anyhow::Result<Group<T>>> + Send + 'static,
    {
        let runner = self.create_runner(data, Some(stop_signal));
        tokio_rayon::spawn(move || runner.run())
    }

    fn create_runner<T, I>(
        &self,
        data: I,
        stop_signal: Option<Arc<Notify>>,
    ) -> GenesisRunner<Handler, I, Database>
    where
        Handler: ProcessStateGroup<T>,
        T: HandlesGenesisResource,
        I: IntoIterator<Item = anyhow::Result<Group<T>>>,
    {
        let handler = Handler::new(self.block_height);
        let database = self.db.clone();
        GenesisRunner::new(
            stop_signal,
            self.cancel_token.clone(),
            handler,
            data,
            database,
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct Handler {
    output_index: u64,
    block_height: BlockHeight,
}

impl Handler {
    fn new(block_height: BlockHeight) -> Self {
        Self {
            output_index: 0,
            block_height,
        }
    }
}

impl HandlesGenesisResource for CoinConfig {
    fn genesis_resource() -> GenesisResource {
        GenesisResource::Coins
    }
}

impl HandlesGenesisResource for MessageConfig {
    fn genesis_resource() -> GenesisResource {
        GenesisResource::Messages
    }
}

impl HandlesGenesisResource for ContractConfig {
    fn genesis_resource() -> GenesisResource {
        GenesisResource::Contracts
    }
}

impl HandlesGenesisResource for ContractStateConfig {
    fn genesis_resource() -> GenesisResource {
        GenesisResource::ContractStates
    }
}

impl HandlesGenesisResource for ContractBalanceConfig {
    fn genesis_resource() -> GenesisResource {
        GenesisResource::ContractBalances
    }
}

impl HandlesGenesisResource for ContractId {
    fn genesis_resource() -> GenesisResource {
        GenesisResource::ContractsRoot
    }
}

impl ProcessState<CoinConfig> for Handler {
    fn process(&mut self, coin: CoinConfig, tx: &mut Database) -> anyhow::Result<()> {
        let root = init_coin(tx, &coin, self.output_index, self.block_height)?;
        tx.add_coin_root(root)?;

        self.output_index = self.output_index
                .checked_add(1)
                .expect("The maximum number of UTXOs supported in the genesis configuration has been exceeded.");

        Ok(())
    }
}

impl ProcessState<MessageConfig> for Handler {
    fn process(
        &mut self,
        message: MessageConfig,
        tx: &mut Database,
    ) -> anyhow::Result<()> {
        let root = init_da_message(tx, &message)?;
        tx.add_message_root(root)?;
        Ok(())
    }
}

impl ProcessState<ContractConfig> for Handler {
    fn process(
        &mut self,
        contract: ContractConfig,
        tx: &mut Database,
    ) -> anyhow::Result<()> {
        init_contract(tx, &contract, self.output_index, self.block_height)?;
        tx.add_contract_id(contract.contract_id)?;

        self.output_index = self.output_index
                .checked_add(1)
                .expect("The maximum number of UTXOs supported in the genesis configuration has been exceeded.");

        Ok::<(), anyhow::Error>(())
    }
}

impl ProcessStateGroup<ContractStateConfig> for Handler {
    fn process_group(
        &mut self,
        group: Vec<ContractStateConfig>,
        tx: &mut Database,
    ) -> anyhow::Result<()> {
        tx.update_contract_states(group)?;
        Ok(())
    }
}

impl ProcessStateGroup<ContractBalanceConfig> for Handler {
    fn process_group(
        &mut self,
        group: Vec<ContractBalanceConfig>,
        tx: &mut Database,
    ) -> anyhow::Result<()> {
        tx.update_contract_balances(group)?;
        Ok(())
    }
}

impl ProcessState<ContractId> for Handler {
    fn process(&mut self, item: ContractId, tx: &mut Database) -> anyhow::Result<()> {
        let mut contract_ref = ContractRef::new(tx, item);
        let root = contract_ref.root()?;
        let db = contract_ref.database_mut();
        db.add_contract_root(root)?;
        Ok(())
    }
}
