use super::*;
use std::sync::Arc;
use futures::channel::mpsc::Receiver;
use tokio::sync::RwLock;

pub struct Blockchain {
    pub state: Arc<RwLock<State>>,
    pub storage: Arc<dyn Storage>,
}

impl Blockchain {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { state: Arc::new(RwLock::new(State::new())), storage }
    }

    pub fn state(&self) -> Arc<RwLock<State>> {
        self.state.clone()
    }

    pub async fn process_blocks_pipeline(&mut self, ordered_block_receicer: Receiver<ExecutableBlock>) -> Result<(), String> {
        let start_block = self.state.read().await.get_current_block_number() + 1;
        let state = self.state.clone();
        let storage = self.storage.clone();
        tokio::spawn(async move {
            let executor = PipelineExecutor::new(state, start_block, ordered_block_receicer);
            executor.start(storage).await;
        });
        Ok(())
    }

    pub async fn get_account_state(
        &self,
        account_id: &AccountId,
    ) -> Result<Option<AccountState>, String> {
        let state = self.state.read().await;
        if let Some(account) = state.get_account(&account_id.0) {
            Ok(Some(AccountState {
                nonce: account.nonce,
                balance: account.balance,
                kv_store: account.kv_store.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn run(&self) {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}
