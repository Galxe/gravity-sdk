// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Mempool is used to track transactions which have been submitted but not yet
//! agreed upon.
use crate::{
    core_mempool::{
        index::TxnPointer,
        transaction::{InsertionInfo, MempoolTransaction, TimelineState},
        transaction_store::TransactionStore,
    },
    logging::{LogEntry, LogSchema, TxnsLog},
    network::BroadcastPeerPriority,
    shared_mempool::types::{
        MempoolSenderBucket, MultiBucketTimelineIndexIds, TimelineIndexIdentifier,
    },
};
use gaptos::api_types::VerifiedTxnWithAccountSeqNum;
use gaptos::aptos_config::config::NodeConfig;
use aptos_consensus_types::common::{TransactionInProgress, TransactionSummary};
use gaptos::aptos_crypto::HashValue;
use gaptos::aptos_logger::prelude::*;
use gaptos::aptos_types::{
    account_address::AccountAddress,
    mempool_status::{MempoolStatus, MempoolStatusCode},
    transaction::{use_case::UseCaseKey, SignedTransaction},
    vm_status::DiscardedVMStatus,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{atomic::Ordering},
    time::{Duration, Instant, SystemTime},
};

use super::transaction::VerifiedTxn;
use gaptos::aptos_mempool::counters as counters;

pub struct Mempool {
    // Stores the metadata of all transactions in mempool (of all states).
    transactions: TransactionStore,
}

impl Mempool {
    pub fn new(config: &NodeConfig) -> Self {
        Mempool {
            transactions: TransactionStore::new(&config.mempool),
        }
    }

    /// This function will be called once the transaction has been stored.
    pub(crate) fn commit_transaction(&mut self, sender: &AccountAddress, sequence_number: u64) {
        debug!(
            "commit txn {} {}",
            sender,
            sequence_number
        );
        counters::MEMPOOL_TXN_COMMIT_COUNT.inc();
        self.transactions
            .commit_transaction(sender, sequence_number);
    }

    pub(crate) fn log_commit_transaction(
        &self,
        sender: &AccountAddress,
        sequence_number: u64,
        tracked_use_case: Option<(UseCaseKey, &String)>,
        block_timestamp: Duration,
    ) {
        trace!(
            LogSchema::new(LogEntry::RemoveTxn).txns(TxnsLog::new_txn(*sender, sequence_number)),
            is_rejected = false
        );
        self.log_commit_latency(*sender, sequence_number, tracked_use_case, block_timestamp);
        if let Some(ranking_score) = self.transactions.get_ranking_score(sender, sequence_number) {
            counters::core_mempool_txn_ranking_score(
                counters::REMOVE_LABEL,
                counters::COMMIT_ACCEPTED_LABEL,
                self.transactions.get_bucket(ranking_score, sender).as_str(),
                ranking_score,
            );
        }
    }

    fn log_reject_transaction(
        &self,
        sender: &AccountAddress,
        sequence_number: u64,
        reason_label: &'static str,
    ) {
        trace!(
            LogSchema::new(LogEntry::RemoveTxn).txns(TxnsLog::new_txn(*sender, sequence_number)),
            is_rejected = true,
            label = reason_label,
        );
        self.log_commit_rejected_latency(*sender, sequence_number, reason_label);
        if let Some(ranking_score) = self.transactions.get_ranking_score(sender, sequence_number) {
            counters::core_mempool_txn_ranking_score(
                counters::REMOVE_LABEL,
                reason_label,
                self.transactions.get_bucket(ranking_score, sender).as_str(),
                ranking_score,
            );
        }
    }

    pub(crate) fn reject_transaction(
        &mut self,
        sender: &AccountAddress,
        sequence_number: u64,
        hash: &HashValue,
        reason: &DiscardedVMStatus,
    ) {
        if *reason == DiscardedVMStatus::SEQUENCE_NUMBER_TOO_NEW {
            self.log_reject_transaction(sender, sequence_number, counters::COMMIT_IGNORED_LABEL);
            // Do not remove the transaction from mempool
            return;
        }

        let label = if *reason == DiscardedVMStatus::SEQUENCE_NUMBER_TOO_OLD {
            counters::COMMIT_REJECTED_DUPLICATE_LABEL
        } else {
            counters::COMMIT_REJECTED_LABEL
        };
        self.log_reject_transaction(sender, sequence_number, label);
        self.transactions
            .reject_transaction(sender, sequence_number, hash);
    }

    pub(crate) fn log_txn_latency(
        insertion_info: &InsertionInfo,
        bucket: &str,
        stage: &'static str,
        priority: &str,
    ) {
        if let Ok(time_delta) = SystemTime::now().duration_since(insertion_info.insertion_time) {
            counters::core_mempool_txn_commit_latency(
                stage,
                insertion_info.submitted_by_label(),
                bucket,
                time_delta,
                priority,
            );
        }
    }

    fn log_consensus_pulled_latency(&self, account: AccountAddress, sequence_number: u64) {
        if let Some((insertion_info, bucket, priority)) = self
            .transactions
            .get_insertion_info_and_bucket(&account, sequence_number)
        {
            let prev_count = insertion_info
                .consensus_pulled_counter
                .fetch_add(1, Ordering::Relaxed);
            Self::log_txn_latency(
                insertion_info,
                bucket.as_str(),
                counters::CONSENSUS_PULLED_LABEL,
                priority.as_str(),
            );
            counters::CORE_MEMPOOL_TXN_CONSENSUS_PULLED_BY_BUCKET
                .with_label_values(&[bucket.as_str()])
                .observe((prev_count + 1) as f64);
        }
    }

    fn log_commit_rejected_latency(
        &self,
        account: AccountAddress,
        sequence_number: u64,
        stage: &'static str,
    ) {
        if let Some((insertion_info, bucket, priority)) = self
            .transactions
            .get_insertion_info_and_bucket(&account, sequence_number)
        {
            Self::log_txn_latency(insertion_info, bucket.as_str(), stage, priority.as_str());
        }
    }

    fn log_commit_and_parked_latency(
        insertion_info: &InsertionInfo,
        bucket: &str,
        priority: &str,
        tracked_use_case: Option<(UseCaseKey, &String)>,
    ) {
        let parked_duration = if let Some(park_time) = insertion_info.park_time {
            let parked_duration = insertion_info
                .ready_time
                .duration_since(park_time)
                .unwrap_or(Duration::ZERO);
            counters::core_mempool_txn_commit_latency(
                counters::PARKED_TIME_LABEL,
                insertion_info.submitted_by_label(),
                bucket,
                parked_duration,
                priority,
            );
            parked_duration
        } else {
            Duration::ZERO
        };

        if let Ok(commit_duration) = SystemTime::now().duration_since(insertion_info.insertion_time)
        {
            let commit_minus_parked = commit_duration
                .checked_sub(parked_duration)
                .unwrap_or(Duration::ZERO);
            counters::core_mempool_txn_commit_latency(
                counters::NON_PARKED_COMMIT_ACCEPTED_LABEL,
                insertion_info.submitted_by_label(),
                bucket,
                commit_minus_parked,
                priority,
            );

            if insertion_info.park_time.is_none() {
                let use_case_label = tracked_use_case
                    .as_ref()
                    .map_or("entry_user_other", |(_, use_case_name)| {
                        use_case_name.as_str()
                    });

                counters::TXN_E2E_USE_CASE_COMMIT_LATENCY
                    .with_label_values(&[
                        use_case_label,
                        insertion_info.submitted_by_label(),
                        bucket,
                    ])
                    .observe(commit_duration.as_secs_f64());
            }
        }
    }

    fn log_commit_latency(
        &self,
        account: AccountAddress,
        sequence_number: u64,
        tracked_use_case: Option<(UseCaseKey, &String)>,
        block_timestamp: Duration,
    ) {
        if let Some((insertion_info, bucket, priority)) = self
            .transactions
            .get_insertion_info_and_bucket(&account, sequence_number)
        {
            Self::log_txn_latency(
                insertion_info,
                bucket.as_str(),
                counters::COMMIT_ACCEPTED_LABEL,
                priority.as_str(),
            );
            Self::log_commit_and_parked_latency(
                insertion_info,
                bucket.as_str(),
                priority.as_str(),
                tracked_use_case,
            );

            let insertion_timestamp =
                gaptos::aptos_infallible::duration_since_epoch_at(&insertion_info.insertion_time);
            if let Some(insertion_to_block) = block_timestamp.checked_sub(insertion_timestamp) {
                counters::core_mempool_txn_commit_latency(
                    counters::COMMIT_ACCEPTED_BLOCK_LABEL,
                    insertion_info.submitted_by_label(),
                    bucket.as_str(),
                    insertion_to_block,
                    priority.to_string().as_str(),
                );
            }
        }
    }

    pub(crate) fn get_by_hash(&self, hash: HashValue) -> Option<SignedTransaction> {
        self.transactions.get_by_hash(hash)
    }

    pub(crate) fn add_user_txns_batch(
        &mut self,
        txns_with_numbers: Vec<VerifiedTxnWithAccountSeqNum>,
        client_submitted: bool,
        timeline_state: TimelineState,
        priority: Option<BroadcastPeerPriority>,
    ) -> Vec<MempoolStatus> {
        let batch_size = txns_with_numbers.len();
        if batch_size == 0 {
            return vec![];
        }

        let mut results: Vec<Option<MempoolStatus>> = vec![None; batch_size];
        let mut valid_txns_to_insert: Vec<(usize, MempoolTransaction)> = Vec::with_capacity(batch_size);

        let now_system = SystemTime::now(); // Get time once for the batch
        let now_epoch_millis = gaptos::aptos_infallible::duration_since_epoch().as_millis() as u64; // Get epoch time once

        const ZERO_RANKING_SCORE: u64 = 10; // Use existing constant ranking score

        // --- 1. Validation and Preparation Phase ---
        // Iterate through input, validate, and prepare MempoolTransaction objects
        for (index, txn_with_number) in txns_with_numbers.into_iter().enumerate() {
            let txn = txn_with_number.txn; // VerifiedTxn
            let db_sequence_number = txn_with_number.account_seq_num;
            let sender = txn.sender();
            // Basic Validation (Sequence Number Check)
            if txn.seq_number() < db_sequence_number {
                let status = MempoolStatus::new(MempoolStatusCode::InvalidSeqNumber).with_message(format!(
                    "transaction sequence number is {}, current sequence number is {}",
                    txn.seq_number(),
                    db_sequence_number,
                ));
                // Record validation failure status immediately
                results[index] = Some(status.clone());
                continue; // Skip to the next transaction
            }

            // Prepare MempoolTransaction for valid ones
            let insertion_info = InsertionInfo::new(now_system, client_submitted, timeline_state);

            // TODO: Add bytes calculation if needed (as mentioned in original TODO)
            let mempool_txn = MempoolTransaction::new(
                txn.into(), // Takes ownership of txn
                timeline_state,
                insertion_info,
                priority.clone(), // Clone priority for each transaction
                ZERO_RANKING_SCORE, // Use the constant ranking score
                db_sequence_number,
            );

            // Store the prepared transaction along with its original index
            valid_txns_to_insert.push((index, mempool_txn));
        }

        // --- 2. Batch Insertion Phase ---
        if !valid_txns_to_insert.is_empty() {
            // Extract just the MempoolTransaction objects for the batch call
            let mempool_txns_batch: Vec<MempoolTransaction> = valid_txns_to_insert
                .iter()
                .map(|(_, txn)| txn.clone()) // Clone if insert_batch needs owned values
                .collect();

            // *** The Core Optimization ***
            // Call the hypothetical batch insertion method on the transaction store.
            // This assumes `self.transactions.insert_batch` locks ONCE internally.
            let insert_statuses = self.transactions.insert_batch(mempool_txns_batch);

            // --- 3. Process Results and Update Counters for the Batch ---
            let mut accepted_count = 0;

            // Iterate through the results of the batch insertion
            for ((original_index, processed_txn), status) in valid_txns_to_insert.iter().zip(insert_statuses.iter()) {
                // Store the result status from the insertion attempt
                results[*original_index] = Some(status.clone());

                let sender = processed_txn.sender();
                let ranking_score = processed_txn.ranking_score(); // Use score from MempoolTransaction
                let bucket = self.transactions.get_bucket(ranking_score, &sender);

                // Update counters based on insertion result
                if status.code == MempoolStatusCode::Accepted {
                    accepted_count += 1;
                }

                // Ranking score metric (for all attempts passed validation)
                 counters::core_mempool_txn_ranking_score(
                    counters::INSERT_LABEL,
                    status.code.to_string().as_str(),
                    bucket.as_str(),
                    ranking_score,
                );
            }

            // Increment the total added count by the number accepted in this batch
            counters::MEMPOOL_TXN_ADD_COUNT.inc_by(accepted_count as u64);
        }

        // --- 4. Finalize Results ---
        // Ensure all entries in results have a status (either from validation or insertion)
        results.into_iter().map(|opt_status| {
            opt_status.expect("Internal error: Transaction status should have been set")
        }).collect()
    }

    /// Used to add a transaction to the Mempool.
    /// Performs basic validation: checks account's sequence number.
    pub(crate) fn send_user_txn(
        &mut self,
        txn: VerifiedTxn,
        db_sequence_number: u64,
        timeline_state: TimelineState,
        client_submitted: bool,
        // The time at which the transaction was inserted into the mempool of the
        // downstream node (sender of the mempool transaction) in millis since epoch
        ready_time_at_sender: Option<u64>,
        // The prority of this node for the peer that sent the transaction
        priority: Option<BroadcastPeerPriority>,
    ) -> MempoolStatus {
        trace!(
            LogSchema::new(LogEntry::AddTxn)
                .txns(TxnsLog::new_txn(txn.sender(), txn.sequence_number())),
        );
        let sender = txn.sender();
        const ZERO_RANKING_SCORE: u64 = 10;
        // we use sequence number as ranking score, the smaller the sequence number, the higher the ranking score
        let ranking_score = ZERO_RANKING_SCORE;
        // don't accept old transactions (e.g. seq is less than account's current seq_number)
        if txn.sequence_number() < db_sequence_number {
            return MempoolStatus::new(MempoolStatusCode::InvalidSeqNumber).with_message(format!(
                "transaction sequence number is {}, current sequence number is  {}",
                txn.sequence_number(),
                db_sequence_number,
            ));
        }

        let now = SystemTime::now();
        let insertion_info = InsertionInfo::new(now, client_submitted, timeline_state);

        // TODO: add bytes to the transaction
        let txn_info: MempoolTransaction = MempoolTransaction::new(
            txn,
            timeline_state, 
            insertion_info, 
            priority.clone(),
            ranking_score,
            db_sequence_number);


        let submitted_by_label = txn_info.insertion_info().submitted_by_label();
        let status = self.transactions.insert(txn_info);
        let now = gaptos::aptos_infallible::duration_since_epoch().as_millis() as u64;

        if status.code == MempoolStatusCode::Accepted {
            counters::MEMPOOL_TXN_ADD_COUNT.inc();
            if let Some(ready_time_at_sender) = ready_time_at_sender {
                let bucket = self.transactions.get_bucket(ranking_score, &sender);
                counters::core_mempool_txn_commit_latency(
                    counters::BROADCAST_RECEIVED_LABEL,
                    submitted_by_label,
                    bucket.as_str(),
                    Duration::from_millis(now.saturating_sub(ready_time_at_sender)),
                    priority
                        .map_or_else(|| "Unknown".to_string(), |priority| priority.to_string())
                        .as_str(),
                );
            }
        }
        counters::core_mempool_txn_ranking_score(
            counters::INSERT_LABEL,
            status.code.to_string().as_str(),
            self.transactions
                .get_bucket(ranking_score, &sender)
                .as_str(),
            ranking_score,
        );
        status
    }

    /// Txn was already chosen, either in a local or remote previous pull (so now in consensus) or
    /// in the current pull.
    fn txn_was_chosen(
        account_address: AccountAddress,
        sequence_number: u64,
        inserted: &HashSet<(AccountAddress, u64)>,
        exclude_transactions: &BTreeMap<TransactionSummary, TransactionInProgress>,
    ) -> bool {
        if inserted.contains(&(account_address, sequence_number)) {
            return true;
        }

        let min_inclusive = TxnPointer::new(account_address, sequence_number, HashValue::zero());
        let max_exclusive = TxnPointer::new(
            account_address,
            sequence_number.saturating_add(1),
            HashValue::zero(),
        );

        exclude_transactions
            .range(min_inclusive..max_exclusive)
            .next()
            .is_some()
    }

    /// Fetches next block of transactions for consensus.
    /// `return_non_full` - if false, only return transactions when max_txns or max_bytes is reached
    ///                     Should always be true for Quorum Store.
    /// `include_gas_upgraded` - Return transactions that had gas upgraded, even if they are in
    ///                          exclude_transactions. Should only be true for Quorum Store.
    /// `exclude_transactions` - transactions that were sent to Consensus but were not committed yet
    ///  mempool should filter out such transactions.
    #[allow(clippy::explicit_counter_loop)]
    pub(crate) fn get_batch(
        &self,
        max_txns: u64,
        max_bytes: u64,
        return_non_full: bool,
        exclude_transactions: BTreeMap<TransactionSummary, TransactionInProgress>,
    ) -> Vec<SignedTransaction> {
        let start_time = Instant::now();
        let exclude_size = exclude_transactions.len();
        let mut inserted = HashSet::new();

        let gas_end_time = start_time.elapsed();

        let mut result = vec![];
        // Helper DS. Helps to mitigate scenarios where account submits several transactions
        // with increasing gas price (e.g. user submits transactions with sequence number 1, 2
        // and gas_price 1, 10 respectively)
        // Later txn has higher gas price and will be observed first in priority index iterator,
        // but can't be executed before first txn. Once observed, such txn will be saved in
        // `skipped` DS and rechecked once it's ancestor becomes available
        let mut skipped = HashSet::new();
        let mut total_bytes = 0;
        let mut txn_walked = 0usize;
        // iterate over the queue of transactions based on gas price
        'main: for txn in self.transactions.iter_queue() {
            txn_walked += 1;
            let txn_ptr = TxnPointer::from(txn);

            // TODO: removed gas upgraded logic. double check if it's needed
            if exclude_transactions.contains_key(&txn_ptr) {
                continue;
            }
            let tx_seq = txn.get_sequence_number();
            let txn_in_sequence = tx_seq > 0
                && Self::txn_was_chosen(txn.address, tx_seq - 1, &inserted, &exclude_transactions);
            let account_sequence_number = self.transactions.get_sequence_number(&txn.address);
            // include transaction if it's "next" for given account or
            // we've already sent its ancestor to Consensus.
            if txn_in_sequence || account_sequence_number == Some(&tx_seq) {
                inserted.insert((txn.address, tx_seq));
                result.push((txn.address, tx_seq));
                if (result.len() as u64) == max_txns {
                    break;
                }

                // check if we can now include some transactions
                // that were skipped before for given account
                let mut skipped_txn = (txn.address, tx_seq + 1);
                while skipped.remove(&skipped_txn) {
                    inserted.insert(skipped_txn);
                    result.push(skipped_txn);
                    if (result.len() as u64) == max_txns {
                        break 'main;
                    }
                    skipped_txn = (skipped_txn.0, skipped_txn.1 + 1);
                }
            } else {
                skipped.insert((txn.address, tx_seq));
            }
        }
        let result_size = result.len();
        let result_end_time = start_time.elapsed();
        let result_time = result_end_time.saturating_sub(gas_end_time);

        let mut block = Vec::with_capacity(result_size);
        let mut full_bytes = false;
        for (sender, sequence_number) in result {
            if let Some((txn, ranking_score)) = self
                .transactions
                .get_with_ranking_score(&sender, sequence_number)
            {
                let txn_size = txn.txn_bytes_len() as u64;
                if total_bytes + txn_size > max_bytes {
                    full_bytes = true;
                    break;
                }
                total_bytes += txn_size;
                block.push(txn);
                if total_bytes == max_bytes {
                    full_bytes = true;
                }
                counters::core_mempool_txn_ranking_score(
                    counters::CONSENSUS_PULLED_LABEL,
                    counters::CONSENSUS_PULLED_LABEL,
                    self.transactions
                        .get_bucket(ranking_score, &sender)
                        .as_str(),
                    ranking_score,
                );
            }
        }
        
        let block_end_time = start_time.elapsed();
        let block_time = block_end_time.saturating_sub(result_end_time);
        if result_size > 0 {
            debug!(
                LogSchema::new(LogEntry::GetBlock),
                seen_consensus = exclude_size,
                walked = txn_walked,
                // before size and non full check
                result_size = result_size,
                // before non full check
                byte_size = total_bytes,
                block_size = block.len(),
                return_non_full = return_non_full,
                result_time_ms = result_time.as_millis(),
                block_time_ms = block_time.as_millis(),
            );
        } else {
            sample!(
                SampleRate::Duration(Duration::from_secs(60)),
                debug!(
                    LogSchema::new(LogEntry::GetBlock),
                    seen_consensus = exclude_size,
                    walked = txn_walked,
                    // before size and non full check
                    result_size = result_size,
                    // before non full check
                    byte_size = total_bytes,
                    block_size = block.len(),
                    return_non_full = return_non_full,
                    result_time_ms = result_time.as_millis(),
                    block_time_ms = block_time.as_millis(),
                )
            );
        }

        if !return_non_full && !full_bytes && (block.len() as u64) < max_txns {
            block.clear();
        }
        counters::MEMPOOL_TXN_COUNT_IN_GET_BACTH.observe(block.len() as f64);
        counters::mempool_service_transactions(counters::GET_BLOCK_LABEL, block.len());
        counters::MEMPOOL_SERVICE_BYTES_GET_BLOCK.observe(total_bytes as f64);
        for transaction in &block {
            self.log_consensus_pulled_latency(transaction.sender(), transaction.sequence_number());
        }
        block
    }

    pub(crate) fn get_txn_count(&self) -> usize {
        self.transactions.txn_size()
    }

    pub(crate) fn priority_index_size(&self) -> usize {
        self.transactions.priority_index_size()
    }

    /// Returns block of transactions and new last_timeline_id. For each transaction, the output includes
    /// the transaction ready time in millis since epoch
    pub(crate) fn read_timeline(
        &self,
        sender_bucket: MempoolSenderBucket,
        timeline_id: &MultiBucketTimelineIndexIds,
        count: usize,
        before: Option<Instant>,
        priority_of_receiver: BroadcastPeerPriority,
    ) -> (Vec<(SignedTransaction, u64)>, MultiBucketTimelineIndexIds) {
        self.transactions.read_timeline(
            sender_bucket,
            timeline_id,
            count,
            before,
            priority_of_receiver,
        )
    }

    /// Read transactions from timeline from `start_id` (exclusive) to `end_id` (inclusive),
    /// along with their ready times in millis since poch
    pub(crate) fn timeline_range(
        &self,
        sender_bucket: MempoolSenderBucket,
        start_end_pairs: HashMap<TimelineIndexIdentifier, (u64, u64)>,
    ) -> Vec<(SignedTransaction, u64)> {
        self.transactions
            .timeline_range(sender_bucket, start_end_pairs)
    }

    pub(crate) fn timeline_range_of_message(
        &self,
        sender_start_end_pairs: HashMap<
            MempoolSenderBucket,
            HashMap<TimelineIndexIdentifier, (u64, u64)>,
        >,
    ) -> Vec<(SignedTransaction, u64)> {
        sender_start_end_pairs
            .iter()
            .flat_map(|(sender_bucket, start_end_pairs)| {
                self.transactions
                    .timeline_range(*sender_bucket, start_end_pairs.clone())
            })
            .collect()
    }

    #[cfg(test)]
    pub fn get_transaction_store(&self) -> &TransactionStore {
        &self.transactions
    }

    pub fn gen_snapshot(&self) -> Vec<SignedTransaction> {
        todo!()
    }
}
