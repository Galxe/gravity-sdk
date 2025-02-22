# Gravity SDK Architecture Design

## 1. System Overview

Gravity SDK is a high-performance consensus engine built on the Aptos-BFT consensus algorithm. It implements a three-phase consensus pipeline architecture, providing a modular and scalable framework for blockchain systems.

## 2. Consensus Algorithm

### 2.1 AptosBFT

To better understand how Aptos BFT operates, we’ll walk through a concrete example of the block proposal and commitment workflow. This explanation will cover how blocks are proposed, validated, and committed, while emphasizing key concepts such as **Quorum Certificates (QC)**, **2-Chain Safety Rules**, and the **Pipeline**.

#### AptosBFT Workflow

```
╔═══════════════════ AptosBFT Consensus Workflow ══════════════════╗
║                                                                  ║
║  1. PROPOSE                                                      ║
║     ┌──────────────────── Leader ───────────────────────┐        ║
║     │    [Block A] ───────QC A──> [Block B](Proposed)   │        ║
║     └───────────────────── ┬────────────────────────────┘        ║
║                            │                                     ║
║  2. BROADCAST             ╱│╲                                    ║
║                          ╱ │ ╲                                   ║
║     ┌──────────────────── Validators ──────────────────┐         ║
║     │                                                  │         ║
║     │    Leader*        Follower-2        Follower-3   │         ║
║     │      ↓               ↓                 ↓         │         ║
║     │      └───────────────┼─────────────────┘         │         ║
║     │                      │                           │         ║
║     └──────────────────────┼───────────────────────────┘         ║
║                            │                                     ║
║  3. VOTE                   ↓                                     ║
║     ┌──────────────── Block B (Pending) ───────────────┐         ║
║     │    • Collecting Validator Votes                  │         ║
║     │    • Including Leader's Vote                     │         ║
║     └──────────────────────┼───────────────────────────┘         ║
║                            │                                     ║
║  4. QC FORMATION           ↓                                     ║
║     ┌──────────────── Block B (QC Formed) ─────────────┐         ║
║     │    ✓ 2f+1 Votes Collected                        │         ║
║     │    ✓ QC B Generated                              │         ║
║     └──────────────────────┼───────────────────────────┘         ║
║                            │                                     ║
║  5. 2-CHAIN COMMIT         ↓                                     ║
║     Block A ────────> Block B                                    ║
║     (finalized) <───────── QC                                    ║
║                                                                  ║
║  * Leader acts as both proposer and validator                    ║
╚══════════════════════════════════════════════════════════════════╝
```

##### 1. Block Proposal (Propose)

- The **Leader** selects uncommitted transactions and creates a new block, referred to as Block B, which is then broadcast to other validators.
- Block B includes:
    - Round information (a counter that helps validators determine which round they are in).
    - Transaction data (a set of transactions to be executed).
    - The QC (Quorum Certificate) for Block A, which confirms that Block A was validated by more than two-thirds of the validators.

##### 2. Voting Phase (Vote)

- After receiving Block B, validators proceed with the following validation steps:
    1. Ensure the block format is correct (e.g., it follows the expected data structure).
    2. Verify the parent block reference (i.e., Block A) to ensure that the block is extending a valid chain.
    3. Validate the transactions within the block to ensure they are correct and executable.
- Upon successful validation:
    1. Validators sign Block B with their private key, essentially voting for it.
    2. They send their signed votes to the leader of the next round, who is responsible for collecting the votes and forming the next QC.

##### 3. Forming the QC (Certify)

- When Block B gathers signatures from more than two-thirds (2f+1, where f is the maximum number of faulty validators) of the validators:
    - A **Quorum Certificate (QC)** for Block B is created.
    - The QC acts as proof that consensus has been reached for Block B, confirming that a supermajority of validators agree on the validity of Block B.

##### 4. 2-Chain Commitment (Ordered)

- Once Block B receives a QC, **Block A is marked as ordered**. This is based on the **2-chain safety rule**, which states that for a block to be committed (i.e., finalized), the next two blocks (Block B and Block C) must have valid QCs.
- This rule ensures that the blockchain maintains safety by only committing blocks that have sufficient agreement from the network over multiple rounds.

#### Workflow:
1. Result collection from execution layer
2. Signature gathering and validation
3. Threshold signature verification
4. Block finalization and commitment


```
Transaction Flow:
[Transaction Input] → [Pre-Consensus] → [Consensus] → [Post-Consensus] → [Commitment]
                          ↓                 ↓               ↓
                    [Quorum Store] → [Aptos-BFT Core] → [Result Processing]
                          ↓                 ↓               ↓
                     [POS Gen] → [Block Ordering] → [Block Commitment]
```


## 3 Gravity SDK Architecture

The Gravity SDK is a modular blockchain execution engine designed with a focus on generality, scalability, and high performance, while ensuring security and decentralization. This document outlines the core architecture of the SDK and the specific roles of its modules in the transaction lifecycle.

---

### 3.1 Mempool

The **Mempool** is the entry point for transactions into the execution engine. Designed with minimalism in mind, it prioritizes compatibility and protocol independence while serving as a staging area for transactions before further processing.

#### **Transaction Reception and Validation**
- The Mempool receives **ready transactions** from the execution layer.  
- All validation, such as signature verification, nonce checks, gas limits, and format checks, is completed at the execution layer before a transaction is forwarded to the Mempool.  
- The Mempool in Gravity SDK only processes transactions that are pre-validated and marked as ready.

#### **Transaction Storage**
- Upon receiving a transaction, the Mempool stores only the **essential pieces of data**, minimizing overhead:
  - **Account Address**: Identifies the sender of the transaction.
  - **Account Nonce**: Tracks the sender's latest transaction sequence.
  - **Transaction Nonce**: Ensures the transaction's order and prevents replay.
  - **Transaction Bytes**: Contains the raw transaction data without parsing or interpreting its contents.

- By avoiding transaction content parsing, the Mempool maintains protocol independence, allowing the SDK to work with diverse blockchain protocols and VM implementations.

#### **Order and Replay Protection**
- The nonce mechanism ensures that:
  - Transactions are executed in the correct order.
  - Duplicate or replayed transactions are effectively rejected.

With its minimal and general-purpose design, the Mempool provides a lightweight yet powerful foundation for transaction handling, enabling seamless integration with various blockchain systems while optimizing storage and throughput.

---

### 3.2 Quorum Store

The **Quorum Store** introduces a pre-consensus mechanism to enhance network efficiency and reduce bandwidth consumption. By focusing on transaction propagation and availability, it ensures that transactions are reliably distributed and prepared for consensus.

#### **Batching Transactions**
- Transactions in the Mempool are grouped into **batches**, reducing the overhead of broadcasting individual transactions.

#### **Weak Broadcast**
- Transaction batches are propagated across the network using a **weak broadcast protocol**, which:
  - Does not enforce strict consistency during data dissemination.
  - Ensures that the majority of nodes receive the transaction batches.

#### **Transaction Confirmation**
- When a node receives a transaction batch:
  - If the transactions are already stored locally, the node sends a confirmation message.
  - If any transactions are missing, the node requests them, stores them locally, and then confirms their availability.

#### **Proof of Store**
- Once a node collects a sufficient number of confirmations (typically from a quorum of nodes), it generates a **Proof of Store**.  
- The Proof of Store serves as a **proof of transaction availability**, confirming that the transaction batch is widely stored across the network.

The Quorum Store ensures efficient transaction propagation while preparing transactions for consensus, reducing the load on the consensus mechanism and improving overall system performance.

---

### 3.3 Consensus

The **Consensus** module is responsible for ordering the Proof of Store objects generated by the Quorum Store. It establishes a deterministic order for transactions, which is essential for maintaining consistent state transitions across the network.

#### **Validating Proof of Store**
- The consensus layer first verifies the validity of each Proof of Store to ensure it satisfies the requirements of the pre-consensus mechanism.

#### **Ordering Proof of Store**
- Proofs of Store are ordered using a consensus algorithm (Aptos BFT).  
- The result is an **Ordered Block**, which contains a list of transactions arranged in a deterministic sequence.

#### **Ensuring Deterministic State Transitions**
- The strict ordering of transactions within the Ordered Block guarantees that all nodes execute transactions in the same sequence, preventing inconsistencies or state divergence.

The Consensus module's design ensures compatibility with various consensus algorithms, providing flexibility for diverse blockchain implementations while maintaining deterministic and reliable state transitions.

---

### 3.4 Execution Pipeline

The **Execution Pipeline** is the coordination layer that processes Ordered Blocks and converts the deterministic transaction order into a consistent, shared state. It ensures that all nodes in the network reach agreement on the resulting state.

#### **Receiving and Forwarding Ordered Blocks**
- The Execution Pipeline receives Ordered Blocks from the Consensus module.  
- It forwards the transactions in the specified order to the Virtual Machine (VM) for execution.

#### **Collecting and Verifying Execution Results**
- Each node independently processes the transactions in the Ordered Block and generates a **Compute Result**.  
- Nodes broadcast their Compute Results to the network.  
- Each node verifies the Compute Results from others.  
- Once a sufficient number of identical Compute Results (typically **2/3+1**) are received, the network reaches **execution result consensus**.

#### **State Commitment**
- After reaching execution result consensus:
  1. The block is committed in sequence, following its block number.
  2. The state is atomically updated, ensuring consistency across all nodes.
  3. The external VM is notified to finalize the state update.

#### **Key Features of the Execution Pipeline**
- **Decoupled Execution and Consensus**: Execution logic is separated from consensus logic, enabling modularity and easier system optimization.  
- **Support for Multiple VMs**: The pipeline supports various VM implementations, allowing flexibility for different blockchain protocols.  
- **Result Consensus for Fault Tolerance**: Even with faulty or malicious nodes, the system ensures state consistency by requiring consensus on execution results.

The Execution Pipeline transforms ordered transactions into a reliable, deterministic state, ensuring the integrity and consistency of the blockchain across all nodes.

---

### Summary

The Gravity SDK architecture provides a modular and efficient framework for processing transactions, from initial reception to final state commitment:

1. **Mempool**: Handles transaction reception and storage while maintaining protocol independence.  
2. **Quorum Store**: Optimizes transaction propagation with pre-consensus mechanisms to improve network efficiency.  
3. **Consensus**: Establishes a deterministic transaction order, ensuring state consistency and reliability.  
4. **Execution Pipeline**: Converts ordered transactions into a shared, deterministic state while supporting modular VM integration.

This design ensures high performance, scalability, and compatibility, making the Gravity SDK a powerful and versatile solution for a wide range of blockchain applications.


## 5. Recovery Mechanism

### 5.1. Local Execution Layer Recovery

1. When restarted, the consensus layer reads the latest execution block number from the execution layer and finds the corresponding block in the consensus layer database as the root block.
2. It then iterates over all blocks, finds the block that is newer than the root block (Round larger), and has achieved a committed QC, and plays back to the execution layer.

### 5.2. Block Sync

1. When a new node is added or an old node is restarted after a long period of time and receives the consensus layer information of other nodes, compare the Round of the node first. If the current Round of the node is smaller than the Round of the message, Block Sync is initiated.
2. Consensus layer information carries SyncInfo by default, which records highest_committed_qc and highest_quorum_qc. The current node will first determine whether the block corresponding to highest_committed_qc is local or not. If it is not, it will initiate the first Block Sync, which will synchronize the execution layer blocks of other nodes.
3. After the execution layer of the current node is synchronized to highest_committed_qc, the second block sync will be initiated. This time, block sync mainly synchronizes the consensus layer blocks between highest_committed_qc and highest_quorum_qc of other nodes.

## 6. Performance Characteristics

- **Throughput**: Optimized for high transaction processing
- **Latency**: Minimized block confirmation time
- **Scalability**: Support for dynamic validator sets
- **Fault Tolerance**: Byzantine fault tolerance up to f=(n-1)/3
