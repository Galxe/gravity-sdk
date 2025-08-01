use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use gaptos::aptos_consensus_notifications::{
    ConsensusCommitNotification, ConsensusNotification, ConsensusNotificationListener,
};
use gaptos::aptos_event_notifications::{EventNotificationSender, EventSubscriptionService};
use gaptos::aptos_mempool_notifications::MempoolNotificationSender;
use gaptos::aptos_types::transaction::Transaction;
use tokio::sync::Mutex;

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
            todo!()
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

    /// Handles a commit notification sent by consensus
    async fn handle_consensus_commit_notification(
        &mut self,
        consensus_commit_notification: ConsensusCommitNotification,
    ) -> anyhow::Result<()> {
        // Handle the commit notification
        let committed_transactions = consensus_commit_notification.get_transactions().clone();

        // TODO(gravity_byteyue): the block timestamp usecs should be modified
        self.mempool_notification_handler
            .notify_mempool_of_committed_transactions(
                committed_transactions,
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            )
            .await?;
        let block_number = consensus_commit_notification.get_block_number();
        let events = consensus_commit_notification.get_subscribable_events().clone();
        let mut event_subscription_service = self.event_subscription_service.lock().await;
        event_subscription_service.notify_events(block_number, events).unwrap();
        self.consensus_notification_listener
            .respond_to_commit_notification(consensus_commit_notification, Ok(()))
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn handle_consensus_notification(&mut self, notification: ConsensusNotification) {
        // Handle the notification
        let result = match notification {
            ConsensusNotification::NotifyCommit(commit_notification) => {
                self.handle_consensus_commit_notification(commit_notification).await
            }
            ConsensusNotification::SyncToTarget(sync_notification) => {
                self
                    .event_subscription_service
                    .lock()
                    .await
                    .notify_initial_configs(sync_notification.get_target().ledger_info().block_number())
                    .unwrap();
                self.consensus_notification_listener
                    .respond_to_sync_target_notification(sync_notification, Ok(()))
                    .map_err(|e| anyhow::anyhow!(e));
                Ok(())
            },
            ConsensusNotification::SyncForDuration(consensus_sync_duration_notification) => todo!(),
        };

        // Log any errors from notification handling
        if let Err(error) = result {
            // warn!(LogSchema::new(LogEntry::ConsensusNotification)
            //     .error(&error)
            //     .message("Error encountered when handling the consensus notification!"));
        }
    }

    pub async fn start(&mut self) {
        loop {
            ::futures::select! {
                notification = self.consensus_notification_listener.select_next_some() => {
                    self.handle_consensus_notification(notification).await;
                },
                // _ = progress_check_interval.select_next_some() => {
                //     self.drive_progress().await;
                // }
            }
        }
    }
}
