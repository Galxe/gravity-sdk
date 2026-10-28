// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Test-image-only control and evidence for protocol-aware Byzantine faults.

use anyhow::{bail, Context};
use gaptos::aptos_types::{
    account_address::AccountAddress,
    aggregate_signature::AggregateSignature,
    jwks::{
        jwk::{JWKMoveStruct, JWK},
        unsupported::UnsupportedJWK,
        ProviderJWKs, QuorumCertifiedUpdate,
    },
    validator_txn::ValidatorTransaction,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

const CONTROL_PATH_ENV: &str = "BFT_BYZANTINE_CONTROL_PATH";
const EVIDENCE_PATH_ENV: &str = "BFT_BYZANTINE_EVIDENCE_PATH";
const FIXTURE_ENABLED_ENV: &str = "BFT_BYZANTINE_FIXTURE_ENABLED";
const MAX_CONTROL_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ControlDocument {
    schema_version: u64,
    active: bool,
    fault_id: String,
    behavior: String,
    node_id: String,
    #[serde(default)]
    target_author: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct EquivocationRequest {
    pub fault_id: String,
    pub node_id: String,
}

#[derive(Clone, Debug)]
pub(crate) struct InvalidOracleQcRequest {
    pub fault_id: String,
    pub node_id: String,
}

/// The source is deliberately not registered as an Oracle task. A proposal may
/// contain this update, but no honest observer can independently produce it.
pub(crate) const INVALID_ORACLE_ISSUER: &str = "gravity://3/2001/price_feed";
pub(crate) const VALID_ORACLE_ISSUER_PREFIX: &str = "gravity://3/1004/price_feed?";
const INVALID_ORACLE_PRICE_PAYLOAD_HEX: &str =
    "0100000000000007d101b2e020018e23f2365f0000000000000009543637a800";

#[derive(Default)]
struct RuntimeState {
    fault_id: Option<String>,
    behavior: Option<String>,
    event_count: u64,
    claimed_epoch_round: Option<(u64, u64)>,
    invalid_oracle_proposals: Vec<InvalidOracleProposal>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InvalidOracleProposal {
    epoch: u64,
    round: u64,
    block_id: String,
    user_txn_count: usize,
    valid_oracle_issuer: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InvalidOracleEvidenceDocument<'a> {
    schema_version: u64,
    fault_id: &'a str,
    behavior: &'static str,
    node_id: &'a str,
    issuer: &'static str,
    proposals: &'a [InvalidOracleProposal],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceDocument<'a> {
    schema_version: u64,
    fault_id: &'a str,
    behavior: &'static str,
    node_id: &'a str,
    protocol_effect: ProtocolEffect<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProtocolEffect<'a> {
    observed: bool,
    behavior: &'static str,
    event_count: u64,
    epoch: u64,
    round: u64,
    distinct_message_count: u64,
    recipient_group_count: u64,
    first_message_id: &'a str,
    second_message_id: &'a str,
    first_recipient_count: usize,
    second_recipient_count: usize,
}

static RUNTIME_STATE: Lazy<Mutex<RuntimeState>> = Lazy::new(|| Mutex::new(RuntimeState::default()));

fn valid_token(value: &str) -> bool {
    !value.is_empty() &&
        value.len() <= 128 &&
        value.bytes().enumerate().all(|(index, byte)| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => true,
            b'.' | b'_' | b':' | b'-' => index > 0,
            _ => false,
        })
}

fn configured_path(variable: &str) -> anyhow::Result<Option<PathBuf>> {
    let Some(value) = env::var_os(variable) else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        bail!("{variable} must be an absolute path");
    }
    Ok(Some(path))
}

fn read_control() -> anyhow::Result<Option<ControlDocument>> {
    if env::var(FIXTURE_ENABLED_ENV).as_deref() != Ok("1") {
        return Ok(None);
    }
    let Some(path) = configured_path(CONTROL_PATH_ENV)? else {
        return Ok(None);
    };
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("reading Byzantine control metadata"),
    };
    if !metadata.is_file() {
        bail!("Byzantine control path is not a regular file");
    }
    if metadata.len() > MAX_CONTROL_BYTES {
        bail!("Byzantine control document exceeds 64 KiB");
    }

    let document: ControlDocument =
        serde_json::from_slice(&fs::read(&path).context("reading Byzantine control document")?)
            .context("parsing Byzantine control document")?;
    if document.schema_version != 1 {
        bail!("unsupported Byzantine control schema version");
    }
    if !valid_token(&document.fault_id) || !valid_token(&document.node_id) {
        bail!("Byzantine control document contains an invalid identifier");
    }
    if document.behavior != "equivocation" && document.behavior != "invalid_oracle_qc" {
        bail!("unsupported Byzantine behavior: {}", document.behavior);
    }
    if document.behavior == "invalid_oracle_qc" {
        let author =
            document.target_author.as_deref().context("invalid_oracle_qc requires targetAuthor")?;
        AccountAddress::from_hex_literal(author).context("invalid targetAuthor")?;
    }
    Ok(Some(document))
}

fn reset_state_for_control(state: &mut RuntimeState, control: &ControlDocument) {
    if state.fault_id.as_deref() != Some(&control.fault_id) ||
        state.behavior.as_deref() != Some(&control.behavior)
    {
        state.fault_id = Some(control.fault_id.clone());
        state.behavior = Some(control.behavior.clone());
        state.event_count = 0;
        state.claimed_epoch_round = None;
        state.invalid_oracle_proposals.clear();
    }
}

pub(crate) fn claim_equivocation(
    epoch: u64,
    round: u64,
) -> anyhow::Result<Option<EquivocationRequest>> {
    let Some(control) = read_control()? else {
        return Ok(None);
    };
    if !control.active {
        return Ok(None);
    }
    if control.behavior != "equivocation" {
        return Ok(None);
    }

    let mut state = RUNTIME_STATE
        .lock()
        .map_err(|_| anyhow::anyhow!("Byzantine runtime state lock is poisoned"))?;
    reset_state_for_control(&mut state, &control);
    if state.claimed_epoch_round == Some((epoch, round)) {
        return Ok(None);
    }
    state.claimed_epoch_round = Some((epoch, round));

    Ok(Some(EquivocationRequest { fault_id: control.fault_id, node_id: control.node_id }))
}

pub(crate) fn claim_invalid_oracle_qc(
    author: AccountAddress,
    epoch: u64,
    round: u64,
) -> anyhow::Result<Option<InvalidOracleQcRequest>> {
    let Some(control) = read_control()? else {
        return Ok(None);
    };
    if !control.active || control.behavior != "invalid_oracle_qc" {
        return Ok(None);
    }
    let target = AccountAddress::from_hex_literal(
        control.target_author.as_deref().context("missing targetAuthor")?,
    )?;
    if author != target {
        return Ok(None);
    }

    let mut state = RUNTIME_STATE
        .lock()
        .map_err(|_| anyhow::anyhow!("Byzantine runtime state lock is poisoned"))?;
    reset_state_for_control(&mut state, &control);
    if state.claimed_epoch_round == Some((epoch, round)) {
        return Ok(None);
    }
    state.claimed_epoch_round = Some((epoch, round));
    Ok(Some(InvalidOracleQcRequest { fault_id: control.fault_id, node_id: control.node_id }))
}

/// This wrapper would execute successfully through NativeOracle and the
/// PriceFeedResolver if the SDK did not verify the quorum certificate first.
pub(crate) fn invalid_oracle_transaction() -> anyhow::Result<ValidatorTransaction> {
    let price_payload = hex::decode(INVALID_ORACLE_PRICE_PAYLOAD_HEX)?;
    anyhow::ensure!(price_payload.len() == 32, "invalid Oracle test payload length");
    let mut wrapper = Vec::with_capacity(192);
    for value in [32u64, 1, 1_710_000_059_999, 96, 32] {
        let mut word = [0u8; 32];
        word[24..].copy_from_slice(&value.to_be_bytes());
        wrapper.extend_from_slice(&word);
    }
    wrapper.extend_from_slice(&price_payload);

    Ok(ValidatorTransaction::ObservedJWKUpdate(QuorumCertifiedUpdate {
        update: ProviderJWKs {
            issuer: INVALID_ORACLE_ISSUER.as_bytes().to_vec(),
            version: 1,
            jwks: vec![JWKMoveStruct::from(JWK::Unsupported(UnsupportedJWK {
                id: b"invalid-qc-e2e".to_vec(),
                payload: wrapper,
            }))],
        },
        multi_sig: AggregateSignature::empty(),
    }))
}

pub(crate) fn record_invalid_oracle_qc(
    request: &InvalidOracleQcRequest,
    epoch: u64,
    round: u64,
    block_id: &str,
    user_txn_count: usize,
    valid_oracle_issuer: &str,
) -> anyhow::Result<()> {
    let Some(path) = configured_path(EVIDENCE_PATH_ENV)? else {
        bail!("{EVIDENCE_PATH_ENV} is not configured");
    };
    let mut state = RUNTIME_STATE
        .lock()
        .map_err(|_| anyhow::anyhow!("Byzantine runtime state lock is poisoned"))?;
    if state.fault_id.as_deref() != Some(&request.fault_id) ||
        state.behavior.as_deref() != Some("invalid_oracle_qc")
    {
        bail!("Byzantine fault changed before Oracle QC evidence was recorded");
    }
    state.invalid_oracle_proposals.push(InvalidOracleProposal {
        epoch,
        round,
        block_id: block_id.to_string(),
        user_txn_count,
        valid_oracle_issuer: valid_oracle_issuer.to_string(),
    });
    let document = InvalidOracleEvidenceDocument {
        schema_version: 1,
        fault_id: &request.fault_id,
        behavior: "invalid_oracle_qc",
        node_id: &request.node_id,
        issuer: INVALID_ORACLE_ISSUER,
        proposals: &state.invalid_oracle_proposals,
    };
    write_json_atomically(&path, &document)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn record_equivocation(
    request: &EquivocationRequest,
    epoch: u64,
    round: u64,
    first_message_id: &str,
    second_message_id: &str,
    first_recipient_count: usize,
    second_recipient_count: usize,
) -> anyhow::Result<()> {
    let Some(path) = configured_path(EVIDENCE_PATH_ENV)? else {
        bail!("{EVIDENCE_PATH_ENV} is not configured");
    };
    if first_message_id == second_message_id {
        bail!("equivocation messages must have distinct IDs");
    }
    if first_recipient_count == 0 || second_recipient_count == 0 {
        bail!("equivocation recipient groups must be non-empty");
    }

    let event_count = {
        let mut state = RUNTIME_STATE
            .lock()
            .map_err(|_| anyhow::anyhow!("Byzantine runtime state lock is poisoned"))?;
        if state.fault_id.as_deref() != Some(&request.fault_id) {
            bail!("Byzantine fault changed before evidence was recorded");
        }
        state.event_count =
            state.event_count.checked_add(1).context("Byzantine event counter overflow")?;
        state.event_count
    };

    let document = EvidenceDocument {
        schema_version: 1,
        fault_id: &request.fault_id,
        behavior: "equivocation",
        node_id: &request.node_id,
        protocol_effect: ProtocolEffect {
            observed: true,
            behavior: "equivocation",
            event_count,
            epoch,
            round,
            distinct_message_count: 2,
            recipient_group_count: 2,
            first_message_id,
            second_message_id,
            first_recipient_count,
            second_recipient_count,
        },
    };
    write_json_atomically(&path, &document)
}

fn write_json_atomically<T: Serialize>(path: &Path, document: &T) -> anyhow::Result<()> {
    let parent = path.parent().context("Byzantine evidence path has no parent directory")?;
    if !parent.is_dir() {
        bail!("Byzantine evidence parent directory does not exist");
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("Byzantine evidence path has an invalid file name")?;
    let temporary = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec(document).context("serializing Byzantine evidence")?;
    fs::write(&temporary, bytes).context("writing temporary Byzantine evidence")?;
    fs::rename(&temporary, path).context("publishing Byzantine evidence")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as TestMutex;
    use tempfile::TempDir;

    static ENV_LOCK: TestMutex<()> = TestMutex::new(());

    fn write_control(directory: &TempDir, active: bool, fault_id: &str) -> PathBuf {
        let path = directory.path().join("control.json");
        fs::write(
            &path,
            serde_json::json!({
                "schemaVersion": 1,
                "active": active,
                "faultId": fault_id,
                "behavior": "equivocation",
                "nodeId": "validator-1"
            })
            .to_string(),
        )
        .unwrap();
        path
    }

    #[test]
    fn claims_once_per_round_and_records_typed_evidence() {
        let _guard = ENV_LOCK.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let control = write_control(&directory, true, "fault-1");
        let evidence = directory.path().join("evidence.json");
        env::set_var(FIXTURE_ENABLED_ENV, "1");
        env::set_var(CONTROL_PATH_ENV, &control);
        env::set_var(EVIDENCE_PATH_ENV, &evidence);

        let request = claim_equivocation(7, 11).unwrap().unwrap();
        assert!(claim_equivocation(7, 11).unwrap().is_none());
        record_equivocation(&request, 7, 11, "0x01", "0x02", 2, 2).unwrap();

        let document: serde_json::Value =
            serde_json::from_slice(&fs::read(evidence).unwrap()).unwrap();
        assert_eq!(document["faultId"], "fault-1");
        assert_eq!(document["protocolEffect"]["observed"], true);
        assert_eq!(document["protocolEffect"]["eventCount"], 1);
        assert_eq!(document["protocolEffect"]["distinctMessageCount"], 2);

        env::remove_var(CONTROL_PATH_ENV);
        env::remove_var(EVIDENCE_PATH_ENV);
        env::remove_var(FIXTURE_ENABLED_ENV);
    }

    #[test]
    fn inactive_control_does_not_claim() {
        let _guard = ENV_LOCK.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let control = write_control(&directory, false, "fault-2");
        env::set_var(FIXTURE_ENABLED_ENV, "1");
        env::set_var(CONTROL_PATH_ENV, control);
        assert!(claim_equivocation(9, 13).unwrap().is_none());
        env::remove_var(CONTROL_PATH_ENV);
        env::remove_var(FIXTURE_ENABLED_ENV);
    }

    #[test]
    fn invalid_oracle_fixture_has_a_valid_delivery_but_no_certificate() {
        let ValidatorTransaction::ObservedJWKUpdate(qc) = invalid_oracle_transaction().unwrap()
        else {
            panic!("expected an Oracle validator transaction");
        };
        assert_eq!(qc.update.issuer, INVALID_ORACLE_ISSUER.as_bytes());
        assert_eq!(qc.multi_sig, AggregateSignature::empty());
        let JWK::Unsupported(entry) = JWK::try_from(&qc.update.jwks[0]).unwrap() else {
            panic!("expected an UnsupportedJWK delivery");
        };
        // This is the independently specified PriceFeedResolver V1 golden
        // vector: sourceId=2001, nonce=1, and a canonical 192-byte wrapper.
        let expected = hex::decode(concat!(
            "0000000000000000000000000000000000000000000000000000000000000020",
            "0000000000000000000000000000000000000000000000000000000000000001",
            "0000000000000000000000000000000000000000000000000000018e23f2365f",
            "0000000000000000000000000000000000000000000000000000000000000060",
            "0000000000000000000000000000000000000000000000000000000000000020",
            "0100000000000007d101b2e020018e23f2365f0000000000000009543637a800",
        ))
        .unwrap();
        assert_eq!(entry.payload, expected);
    }

    #[test]
    fn invalid_oracle_control_targets_only_the_selected_proposer() {
        let _guard = ENV_LOCK.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let control = directory.path().join("control.json");
        let evidence = directory.path().join("evidence.json");
        let author = AccountAddress::from_hex_literal("0x1").unwrap();
        fs::write(
            &control,
            serde_json::json!({
                "schemaVersion": 1,
                "active": true,
                "faultId": "bad-oracle-qc-test",
                "behavior": "invalid_oracle_qc",
                "nodeId": "node1",
                "targetAuthor": "0x1",
            })
            .to_string(),
        )
        .unwrap();
        env::set_var(FIXTURE_ENABLED_ENV, "1");
        env::set_var(CONTROL_PATH_ENV, &control);
        env::set_var(EVIDENCE_PATH_ENV, &evidence);

        assert!(claim_invalid_oracle_qc(AccountAddress::ZERO, 7, 10).unwrap().is_none());
        let request = claim_invalid_oracle_qc(author, 7, 10).unwrap().unwrap();
        assert!(claim_invalid_oracle_qc(author, 7, 10).unwrap().is_none());
        record_invalid_oracle_qc(
            &request,
            7,
            10,
            "0000000000000000000000000000000000000000000000000000000000000001",
            1,
            VALID_ORACLE_ISSUER_PREFIX,
        )
        .unwrap();
        let document: serde_json::Value =
            serde_json::from_slice(&fs::read(evidence).unwrap()).unwrap();
        assert_eq!(document["issuer"], INVALID_ORACLE_ISSUER);
        assert_eq!(document["proposals"][0]["userTxnCount"], 1);

        env::remove_var(CONTROL_PATH_ENV);
        env::remove_var(EVIDENCE_PATH_ENV);
        env::remove_var(FIXTURE_ENABLED_ENV);
    }
}
