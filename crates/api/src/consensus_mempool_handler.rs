use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use futures::StreamExt;
use gaptos::{
    aptos_consensus_notifications::{
        ConsensusCommitNotification, ConsensusNotification, ConsensusNotificationListener,
    },
    aptos_event_notifications::{EventNotificationSender, EventSubscriptionService},
    aptos_logger::warn,
    aptos_mempool_notifications::MempoolNotificationSender,
    aptos_types::transaction::Transaction,
};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// A simple handler for sending notifications to mempool
#[derive(Clone)]
pub struct MempoolNotificationHandler<M: MempoolNotificationSender> {
    mempool_notification_sender: M,
}

impl<M: MempoolNotificationSender> MempoolNotificationHandler<M> {
    pub fn new(mempool_notification_sender: M) -> Self {
        Self { mempool_notification_sender }
    }

    /// Notifies mempool that transactions have been committed.
    pub async fn notify_mempool_of_committed_transactions(
        &mut self,
        committed_transactions: Vec<Transaction>,
        block_timestamp_usecs: u64,
    ) -> anyhow::Result<()> {
        let result = self
            .mempool_notification_sender
            .notify_new_commit(committed_transactions, block_timestamp_usecs)
            .await;

        if let Err(error) = result {
            Err(anyhow::anyhow!("failed to notify mempool of committed transactions: {error}"))
        } else {
            Ok(())
        }
    }
}

pub struct ConsensusToMempoolHandler<M: MempoolNotificationSender> {
    mempool_notification_handler: MempoolNotificationHandler<M>,
    consensus_notification_listener: ConsensusNotificationListener,
    event_subscription_service: Arc<Mutex<EventSubscriptionService>>,
}

impl<M: MempoolNotificationSender> ConsensusToMempoolHandler<M> {
    pub fn new(
        mempool_notification_handler: MempoolNotificationHandler<M>,
        consensus_notification_listener: ConsensusNotificationListener,
        event_subscription_service: Arc<Mutex<EventSubscriptionService>>,
    ) -> Self {
        Self {
            mempool_notification_handler,
            consensus_notification_listener,
            event_subscription_service,
        }
    }

    /// Handles a commit notification sent by consensus.
    ///
    /// Fails only when mempool can no longer receive notifications.
    async fn handle_consensus_commit_notification(
        &mut self,
        consensus_commit_notification: ConsensusCommitNotification,
    ) -> anyhow::Result<()> {
        // Handle the commit notification
        let committed_transactions = consensus_commit_notification.get_transactions().clone();

        // TODO(gravity_byteyue): ideally the block timestamp should come from
        // ConsensusCommitNotification rather than the local wall clock. For now, use
        // SystemTime as a compatible workaround.
        self.mempool_notification_handler
            .notify_mempool_of_committed_transactions(
                committed_transactions,
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64,
            )
            .await?;
        let block_number = consensus_commit_notification.get_block_number();
        let events = consensus_commit_notification.get_subscribable_events().clone();
        let mut event_subscription_service = self.event_subscription_service.lock().await;
        if let Err(error) = event_subscription_service.notify_events(block_number, events) {
            warn!("Failed to notify events for block {}: {:?}", block_number, error);
        }
        if let Err(error) = self
            .consensus_notification_listener
            .respond_to_commit_notification(consensus_commit_notification, Ok(()))
        {
            warn!(
                "Failed to respond to commit notification for block {}: {:?}",
                block_number, error
            );
        }
        Ok(())
    }

    async fn handle_consensus_notification(
        &mut self,
        notification: ConsensusNotification,
    ) -> anyhow::Result<()> {
        match notification {
            ConsensusNotification::NotifyCommit(commit_notification) => {
                self.handle_consensus_commit_notification(commit_notification).await
            }
            ConsensusNotification::SyncToTarget(sync_notification) => {
                let ledger_info = sync_notification.get_target().ledger_info();
                let block_number = match ledger_info.commit_info().epoch_block_info() {
                    Some(info) => info.block_number,
                    None => ledger_info.block_number(),
                };
                match self
                    .event_subscription_service
                    .lock()
                    .await
                    .notify_initial_configs(block_number)
                {
                    Ok(_) => {}
                    Err(e) => {
                        warn!(
                            "Failed to notify initial configs for block {}: {:?}. \
                             This is expected after a cross-epoch unwind; \
                             the node will re-sync missing blocks via block sync.",
                            block_number, e
                        );
                    }
                }
                if let Err(e) = self
                    .consensus_notification_listener
                    .respond_to_sync_target_notification(sync_notification, Ok(()))
                {
                    warn!("Failed to respond to sync target notification: {:?}", e);
                }
                Ok(())
            }
            ConsensusNotification::SyncForDuration(_consensus_sync_duration_notification) => {
                todo!()
            }
        }
    }

    /// Forwards consensus notifications until `shutdown` is cancelled.
    ///
    /// Returns `Ok(())` for a requested shutdown. Any other exit means consensus
    /// or mempool went away while the node was still running, and is returned as
    /// an error so the engine can report it.
    pub async fn start(&mut self, shutdown: CancellationToken) -> anyhow::Result<()> {
        loop {
            let notification = tokio::select! {
                // Checked first so queued notifications are dropped, not drained,
                // once shutdown has been requested.
                biased;
                _ = shutdown.cancelled() => return Ok(()),
                notification = self.consensus_notification_listener.next() => notification,
            };

            // Consensus and mempool close their channels when their runtimes shut
            // down, which is only expected after shutdown has been requested.
            let result = match notification {
                Some(notification) => self.handle_consensus_notification(notification).await,
                None => Err(anyhow::anyhow!("consensus notification stream closed")),
            };
            if let Err(error) = result {
                if shutdown.is_cancelled() {
                    return Ok(());
                }
                return Err(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures::channel::mpsc;
    use gaptos::{
        aptos_consensus_notifications::{
            new_consensus_notifier_listener_pair, ConsensusNotificationSender, ConsensusNotifier,
        },
        aptos_crypto::HashValue,
        aptos_infallible::RwLock,
        aptos_mempool_notifications::{MempoolCommitNotification, MempoolNotifier},
        aptos_storage_interface::{DbReader, DbReaderWriter, DbWriter},
    };
    use tokio::task::JoinHandle;

    use super::*;

    /// The handler never reads storage in these tests.
    struct UnusedDb;
    impl DbReader for UnusedDb {}
    impl DbWriter for UnusedDb {}

    struct RunningHandler {
        consensus_notifier: ConsensusNotifier,
        mempool_receiver: mpsc::Receiver<MempoolCommitNotification>,
        shutdown: CancellationToken,
        task: JoinHandle<anyhow::Result<()>>,
    }

    fn spawn_handler() -> RunningHandler {
        let (consensus_notifier, consensus_listener) = new_consensus_notifier_listener_pair(5_000);
        let (mempool_sender, mempool_receiver) = mpsc::channel(16);
        let event_subscription_service = Arc::new(Mutex::new(EventSubscriptionService::new(
            Arc::new(RwLock::new(DbReaderWriter::new(UnusedDb))),
        )));
        let mut handler = ConsensusToMempoolHandler::new(
            MempoolNotificationHandler::new(MempoolNotifier::new(mempool_sender)),
            consensus_listener,
            event_subscription_service,
        );
        let shutdown = CancellationToken::new();
        let handler_shutdown = shutdown.clone();
        let task = tokio::spawn(async move { handler.start(handler_shutdown).await });
        RunningHandler { consensus_notifier, mempool_receiver, shutdown, task }
    }

    /// Sends a commit from a task that owns its own notifier clone, so the
    /// consensus stream stays open until that commit has been answered. The
    /// task resolves to whether consensus got a successful response.
    fn send_commit(notifier: &ConsensusNotifier, block_number: u64) -> JoinHandle<bool> {
        let notifier = notifier.clone();
        tokio::spawn(async move {
            let transactions = vec![Transaction::StateCheckpoint(HashValue::zero())];
            notifier.notify_new_commit(transactions, vec![], block_number).await.is_ok()
        })
    }

    async fn handler_exit(task: JoinHandle<anyhow::Result<()>>) -> anyhow::Result<()> {
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("handler did not stop")
            .expect("handler panicked")
    }

    #[tokio::test]
    async fn closed_consensus_stream_while_running_is_reported() {
        let handler = spawn_handler();

        drop(handler.consensus_notifier);

        assert!(handler_exit(handler.task).await.is_err());
    }

    #[tokio::test]
    async fn closed_consensus_stream_after_shutdown_is_clean() {
        let handler = spawn_handler();

        handler.shutdown.cancel();
        drop(handler.consensus_notifier);

        handler_exit(handler.task).await.unwrap();
    }

    #[tokio::test]
    async fn idle_stream_keeps_handler_running_until_shutdown() {
        let handler = spawn_handler();

        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(!handler.task.is_finished());

        handler.shutdown.cancel();
        handler_exit(handler.task).await.unwrap();
    }

    #[tokio::test]
    async fn pending_commits_are_answered_before_closure_is_reported() {
        let handler = spawn_handler();
        let commits: Vec<_> =
            (1..=3).map(|block| send_commit(&handler.consensus_notifier, block)).collect();

        drop(handler.consensus_notifier);

        assert!(handler_exit(handler.task).await.is_err());
        for commit in commits {
            assert!(commit.await.unwrap(), "consensus did not get a commit response");
        }
        assert_eq!(handler.mempool_receiver.collect::<Vec<_>>().await.len(), 3);
    }

    #[tokio::test]
    async fn queued_commits_are_dropped_after_shutdown() {
        let handler = spawn_handler();
        // A single poll enqueues the commit without yielding to the handler task.
        let transactions = vec![Transaction::StateCheckpoint(HashValue::zero())];
        let mut commit =
            Box::pin(handler.consensus_notifier.notify_new_commit(transactions, vec![], 1));
        assert!(futures::poll!(&mut commit).is_pending());

        handler.shutdown.cancel();

        handler_exit(handler.task).await.unwrap();
        assert!(handler.mempool_receiver.collect::<Vec<_>>().await.is_empty());
    }

    #[tokio::test]
    async fn closed_mempool_while_running_is_reported() {
        let handler = spawn_handler();

        drop(handler.mempool_receiver);
        let _commit = send_commit(&handler.consensus_notifier, 1);

        assert!(handler_exit(handler.task).await.is_err());
    }
}
