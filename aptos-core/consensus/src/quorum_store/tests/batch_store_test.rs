// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::{
    network::QuorumStoreSender,
    quorum_store::{
        batch_requester::BatchRequester,
        batch_store::{BatchReader, BatchReaderImpl, BatchStore, BatchWriter, QuotaManager},
        quorum_store_db::QuorumStoreDB,
        types::{Batch, BatchKey, BatchRequest, BatchResponse, PersistedValue, StorageMode},
    },
};
use aptos_consensus_types::{
    common::Author,
    proof_of_store::{BatchId, BatchInfo, ProofOfStore, SignedBatchInfo},
};
use claims::{assert_err, assert_ok, assert_ok_eq};
use gaptos::{
    aptos_config::network_id::{NetworkId, PeerNetworkId},
    aptos_crypto::HashValue,
    aptos_temppath::TempPath,
    aptos_types::{
        account_address::AccountAddress, transaction::SignedTransaction,
        validator_verifier::random_validator_verifier,
    },
};
use once_cell::sync::Lazy;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::task::spawn_blocking;

static TEST_REQUEST_ACCOUNT: Lazy<AccountAddress> = Lazy::new(AccountAddress::random);

pub fn batch_store_for_test(memory_quota: usize) -> Arc<BatchStore> {
    let tmp_dir = TempPath::new();
    let db = Arc::new(QuorumStoreDB::new(&tmp_dir));
    let (signers, _validator_verifier) = random_validator_verifier(4, None, false);

    Arc::new(BatchStore::new(
        10, // epoch
        10, // last committed round
        db,
        memory_quota, // memory_quota
        2001,         // db quota
        2001,         // batch quota
        signers[0].clone(),
    ))
}

fn request_for_test(
    digest: &HashValue,
    round: u64,
    num_bytes: u64,
    maybe_payload: Option<Vec<SignedTransaction>>,
) -> PersistedValue {
    PersistedValue::new(
        BatchInfo::new(
            *TEST_REQUEST_ACCOUNT, // make sure all request come from the same account
            BatchId::new_for_test(1),
            10,
            round,
            *digest,
            10,
            num_bytes,
            0,
        ),
        maybe_payload,
    )
}

#[test]
fn test_insert_expire() {
    let batch_store = batch_store_for_test(30);

    let digest = HashValue::random();

    assert_ok_eq!(batch_store.insert_to_cache(&request_for_test(&digest, 15, 10, None)), true);
    assert_ok_eq!(batch_store.insert_to_cache(&request_for_test(&digest, 30, 10, None)), true);
    assert_ok_eq!(batch_store.insert_to_cache(&request_for_test(&digest, 25, 10, None)), false);
    let expired = batch_store.clear_expired_payload(27);
    assert!(expired.is_empty());
    let expired = batch_store.clear_expired_payload(29);
    assert!(expired.is_empty());
    assert_eq!(batch_store.clear_expired_payload(30), vec![BatchKey::new(10, digest)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_extend_expiration_vs_save() {
    let num_experiments = 2000;
    let batch_store = batch_store_for_test(2001);

    let batch_store_clone1 = batch_store.clone();
    let batch_store_clone2 = batch_store.clone();

    let digests: Vec<HashValue> = (0..num_experiments).map(|_| HashValue::random()).collect();
    let later_exp_values: Vec<PersistedValue> = (0..num_experiments)
        .map(|i| {
            // Pre-insert some of them.
            if i % 2 == 0 {
                assert_ok!(batch_store.save(&request_for_test(
                    &digests[i],
                    i as u64 + 30,
                    1,
                    None
                )));
            }

            request_for_test(&digests[i], i as u64 + 40, 1, None)
        })
        .collect();

    // Marshal threads to start at the same time.
    let start_flag = Arc::new(AtomicUsize::new(0));
    let start_clone1 = start_flag.clone();
    let start_clone2 = start_flag.clone();

    let save_error = Arc::new(AtomicBool::new(false));
    let save_error_clone1 = save_error.clone();
    let save_error_clone2 = save_error.clone();

    // Thread that extends expiration by saving.
    spawn_blocking(move || {
        for (i, later_exp_value) in later_exp_values.into_iter().enumerate() {
            // Wait until both threads are ready for next experiment.
            loop {
                let flag_val = start_clone1.load(Ordering::Acquire);
                if flag_val == 3 * i + 1 || flag_val == 3 * i + 2 {
                    break;
                }
            }

            if batch_store_clone1.save(&later_exp_value).is_err() {
                // Save in a separate flag and break so test doesn't hang.
                save_error_clone1.store(true, Ordering::Release);
                break;
            }
            start_clone1.fetch_add(1, Ordering::Relaxed);
        }
    });

    // Thread that expires.
    spawn_blocking(move || {
        for i in 0..num_experiments {
            // Wait until both threads are ready for next experiment.
            loop {
                let flag_val = start_clone2.load(Ordering::Acquire);
                if flag_val == 3 * i + 1 ||
                    flag_val == 3 * i + 2 ||
                    save_error_clone2.load(Ordering::Acquire)
                {
                    break;
                }
            }

            batch_store_clone2.update_certified_timestamp(i as u64 + 30);
            start_clone2.fetch_add(1, Ordering::Relaxed);
        }
    });

    for (i, &digest) in digests.iter().enumerate().take(num_experiments) {
        // Set the conditions for experiment (both threads waiting).
        while start_flag.load(Ordering::Acquire) % 3 != 0 {
            assert!(!save_error.load(Ordering::Acquire));
        }

        if i % 2 == 1 {
            assert_ok!(batch_store.save(&request_for_test(&digest, i as u64 + 30, 1, None)));
        }

        // Unleash the threads.
        start_flag.fetch_add(1, Ordering::Relaxed);
    }
    // Finish the experiment
    while start_flag.load(Ordering::Acquire) % 3 != 0 {}

    // Expire everything, call for higher times as well.
    for i in 35..50 {
        batch_store.update_certified_timestamp((i + num_experiments) as u64);
    }
}

#[test]
fn test_quota_manager() {
    let mut qm = QuotaManager::new(20, 10, 7);
    assert_ok_eq!(qm.update_quota(5), StorageMode::MemoryAndPersisted);
    assert_ok_eq!(qm.update_quota(3), StorageMode::MemoryAndPersisted);
    assert_ok_eq!(qm.update_quota(2), StorageMode::MemoryAndPersisted);
    assert_ok_eq!(qm.update_quota(1), StorageMode::PersistedOnly);
    assert_ok_eq!(qm.update_quota(2), StorageMode::PersistedOnly);
    assert_ok_eq!(qm.update_quota(7), StorageMode::PersistedOnly);
    // 6 batches, fully used quotas

    // exceed storage quota.
    assert_err!(qm.update_quota(2));

    qm.free_quota(5, StorageMode::MemoryAndPersisted);
    // 5 batches, available memory and db quota: 5

    // exceed storage quota
    assert_err!(qm.update_quota(6));
    assert_ok_eq!(qm.update_quota(3), StorageMode::MemoryAndPersisted);

    // exceed storage quota
    assert_err!(qm.update_quota(3));
    assert_ok_eq!(qm.update_quota(1), StorageMode::MemoryAndPersisted);
    // 7 batches, available memory and DB quota: 1

    // Exceed batch quota
    assert_err!(qm.update_quota(1));

    qm.free_quota(1, StorageMode::PersistedOnly);
    // 6 batches, available memory quota: 1, available DB quota: 2

    // exceed storage quota
    assert_err!(qm.update_quota(3));
    assert_ok_eq!(qm.update_quota(2), StorageMode::PersistedOnly);
    // 7 batches, available memory quota: 1, available DB quota: 0

    qm.free_quota(2, StorageMode::MemoryAndPersisted);
    // 6 batches, available memory quota: 3, available DB quota: 2

    // while there is available memory quota, DB quota isn't enough.
    assert_err!(qm.update_quota(3));
    assert_ok_eq!(qm.update_quota(2), StorageMode::MemoryAndPersisted);
}

#[test]
fn test_get_local_batch() {
    let store = batch_store_for_test(30);

    let digest_1 = HashValue::random();
    let request_1 = request_for_test(&digest_1, 50, 20, Some(vec![]));
    // Should be stored in memory and DB.
    assert!(!store.persist(vec![request_1]).is_empty());

    store.update_certified_timestamp(40);

    let digest_2 = HashValue::random();
    assert!(digest_2 != digest_1);
    // Expiration is before 40.
    let request_2_expired = request_for_test(&digest_2, 30, 20, Some(vec![]));
    assert!(store.persist(vec![request_2_expired]).is_empty());
    // Proper (in the future) expiration.
    let request_2 = request_for_test(&digest_2, 55, 20, Some(vec![]));
    // Should be stored in DB only
    assert!(!store.persist(vec![request_2]).is_empty());

    let digest_3 = HashValue::random();
    assert!(digest_3 != digest_1);
    assert!(digest_3 != digest_2);
    let request_3 = request_for_test(&digest_3, 56, 1970, Some(vec![]));
    // Out of quota - should not be stored
    assert!(store.persist(vec![request_3.clone()]).is_empty());

    assert_ok!(store.get_batch_from_local(&BatchKey::new(10, digest_1)));
    assert_ok!(store.get_batch_from_local(&BatchKey::new(10, digest_2)));
    store.update_certified_timestamp(51);
    // Expired value w. digest_1.
    assert_err!(store.get_batch_from_local(&BatchKey::new(10, digest_1)));
    assert_ok!(store.get_batch_from_local(&BatchKey::new(10, digest_2)));

    // Value w. digest_3 was never persisted
    assert_err!(store.get_batch_from_local(&BatchKey::new(10, digest_3)));
    // Since payload is cleared, we can now persist value w. digest_3
    assert!(!store.persist(vec![request_3]).is_empty());
    assert_ok!(store.get_batch_from_local(&BatchKey::new(10, digest_3)));

    store.update_certified_timestamp(52);
    assert_ok!(store.get_batch_from_local(&BatchKey::new(10, digest_2)));
    assert_ok!(store.get_batch_from_local(&BatchKey::new(10, digest_3)));

    store.update_certified_timestamp(55);
    // Expired value w. digest_2
    assert_err!(store.get_batch_from_local(&BatchKey::new(10, digest_2)));
    assert_ok!(store.get_batch_from_local(&BatchKey::new(10, digest_3)));

    store.update_certified_timestamp(56);
    // Expired value w. digest_3
    assert_err!(store.get_batch_from_local(&BatchKey::new(10, digest_1)));
    assert_err!(store.get_batch_from_local(&BatchKey::new(10, digest_2)));
    assert_err!(store.get_batch_from_local(&BatchKey::new(10, digest_3)));
}

// ---------------------------------------------------------------------------
// Regression for Galxe/gravity-audit#707 (F5):
// BatchStore::persist's reject branch (batch expired vs last_certified_time)
// skips notify_subscribers, so a concurrent in-flight batch fetcher that
// subscribed via subscribe() is never woken and falls through to a
// request_batch timeout -- even when the batch is, in fact, locally available.
// ---------------------------------------------------------------------------

/// Mock network sender whose `request_batch` future never resolves. This forces
/// the only viable delivery path inside `BatchRequester::request_batch` to be the
/// subscription channel (`subscriber_rx`), which is fired by
/// `BatchStore::notify_subscribers`. If `persist` skips notification, the fetcher
/// never completes.
#[derive(Clone)]
struct NeverRespondingSender;

#[async_trait::async_trait]
impl QuorumStoreSender for NeverRespondingSender {
    async fn request_batch(
        &self,
        _request: BatchRequest,
        _recipient: PeerNetworkId,
        _timeout: Duration,
    ) -> anyhow::Result<BatchResponse> {
        // Never resolve: the in-flight fetcher can only be served via the
        // persist-subscription notification, never via the network.
        std::future::pending::<()>().await;
        unreachable!()
    }

    async fn send_signed_batch_info_msg(
        &self,
        _signed_batch_infos: Vec<SignedBatchInfo>,
        _recipients: Vec<Author>,
    ) {
        unimplemented!()
    }

    async fn broadcast_batch_msg(&mut self, _batches: Vec<Batch>) {
        unimplemented!()
    }

    async fn broadcast_proof_of_store_msg(&mut self, _proof_of_stores: Vec<ProofOfStore>) {
        unimplemented!()
    }

    async fn send_proof_of_store_msg_to_self(&mut self, _proof_of_stores: Vec<ProofOfStore>) {
        unimplemented!()
    }

    fn get_available_peers(&self) -> anyhow::Result<Vec<PeerNetworkId>> {
        unimplemented!()
    }
}

/// Builds a `BatchStore` for test with an explicit `last_certified_time`, plus the
/// matching `ValidatorSigner` set (so we can install a `BatchRequester` and a
/// `BatchReaderImpl` on top of it).
fn batch_store_with_last_certified_time(
    last_certified_time: u64,
) -> (Arc<BatchStore>, Vec<gaptos::aptos_types::validator_signer::ValidatorSigner>) {
    let tmp_dir = TempPath::new();
    let db = Arc::new(QuorumStoreDB::new(&tmp_dir));
    let (signers, _verifier) = random_validator_verifier(4, None, false);
    let store = Arc::new(BatchStore::new(
        10, // epoch
        last_certified_time,
        db,
        2001, // memory quota
        2001, // db quota
        2001, // batch quota
        signers[0].clone(),
    ));
    (store, signers)
}

/// F5 reproduction. A batch that is *already locally available* (persisted to the
/// quorum-store DB, exactly as the recovery fetch path does via
/// `save_fetched_batch_to_db`) is then re-`persist`ed with an expiration that is
/// `<= last_certified_time`. `save()` rejects it, `persist_inner` returns `None`,
/// and `persist` filters it out -- so `notify_subscribers` is never called. The
/// concurrent fetcher that subscribed first is therefore never woken.
///
/// Correct behavior (what this test asserts): the fetcher IS served, because the
/// batch is locally retrievable. On the current code the fetcher hangs and this
/// test fails by timing out on `rx`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_persist_reject_branch_notifies_subscriber_707() {
    // last_certified_time is far ahead, so any batch whose expiration <= 1000 is
    // rejected by save().
    let last_certified_time = 1000u64;
    let (batch_store, signers) = batch_store_with_last_certified_time(last_certified_time);

    // A real (validator) batch reader on top of the store. my_peer_id is a
    // validator that is NOT one of the signers, so the request goes "to peers".
    let my_peer_id = PeerNetworkId::new(NetworkId::Validator, AccountAddress::random());
    let (_unused_signers, verifier) = random_validator_verifier(1, None, false);
    let batch_requester = BatchRequester::new(
        10, // epoch
        my_peer_id,
        1,      // request_num_peers
        100,    // retry_limit (large; we never want a network-driven timeout here)
        10_000, // retry_interval_ms (large; first retry is far in the future)
        10_000, // rpc_timeout_ms
        NeverRespondingSender,
        Arc::new(verifier),
    );
    let reader = BatchReaderImpl::new(batch_store.clone(), batch_requester);

    // The batch the (replayed) ordered block needs. Expiration is in the PAST
    // relative to last_certified_time, so the redundant persist() will be rejected.
    let digest = HashValue::random();
    let batch_author = signers[1].author();
    let payload: Vec<SignedTransaction> = vec![];
    let info = BatchInfo::new(
        batch_author,
        BatchId::new_for_test(1),
        10,  // epoch
        500, // expiration <= last_certified_time(1000) => save() rejects
        digest,
        0,
        0,
        0,
    );
    let key = BatchKey::new(10, digest);

    // Start the in-flight fetcher first (it subscribes internally because the
    // batch is not yet in the in-memory cache). The signer list points at a real
    // peer so request_batch enters the wait loop (network never responds).
    let signers_for_request = vec![batch_author];
    let mut rx = reader.get_batch(key.clone(), 500, signers_for_request);

    // Give the spawned fetcher a moment to register its subscription before the
    // batch becomes available + the (rejecting) persist runs.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // The batch IS locally available: persist it directly to the DB, exactly as
    // the recovery fetch path does via save_fetched_batch_to_db.
    batch_store
        .save_fetched_batch_to_db(PersistedValue::new(info.clone(), Some(payload.clone())))
        .expect("save_fetched_batch_to_db should succeed");

    // Now the redundant persist() that the fetch path also performs. save() rejects
    // it (expiration 500 <= last_certified_time 1000), so on the buggy code
    // notify_subscribers is skipped and the subscriber is never woken.
    let signed = batch_store.persist(vec![PersistedValue::new(info.clone(), Some(payload))]);
    assert!(signed.is_empty(), "save() must reject the expired-vs-last_certified_time batch");

    // The fetcher must be served, because the batch is locally retrievable.
    let result = tokio::time::timeout(Duration::from_secs(3), &mut rx).await;

    match result {
        Ok(Ok(Ok(_txns))) => {
            // Correct behavior: subscriber notified for a locally-available batch.
        }
        Ok(Ok(Err(e))) => panic!(
            "F5 repro: fetcher got an error instead of the locally-available batch: {:?}",
            e
        ),
        Ok(Err(e)) => panic!("F5 repro: fetcher channel dropped: {:?}", e),
        Err(_) => panic!(
            "F5 repro (Galxe/gravity-audit#707): in-flight fetcher was NEVER notified after \
             persist() rejected the expired-vs-last_certified_time batch (notify_subscribers \
             skipped on the reject branch), even though the batch is locally available -- the \
             fetcher hung and timed out"
        ),
    }
}
