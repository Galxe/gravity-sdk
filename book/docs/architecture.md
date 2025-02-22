# Gravity SDK Architecture Design

## 1. System Overview

Gravity SDK is a high-performance consensus engine built on the Aptos-BFT consensus algorithm. It implements a three-phase consensus pipeline architecture, providing a modular and scalable framework for blockchain systems.

## 2. Core Architecture Components

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
### 4.3. Core Pipeline Components

The workflow between the execution layer, GCEI protocol, and consensus layer follows a structured pattern. The process can be divided into three distinct stages:

1. **Pre-Consensus Stage**
2. **Consensus Stage**
3. **Post-Consensus Stage**

Each stage is responsible for specific tasks, as outlined below.

```
    Stage     Execution Layer    GCEI Protocol       Consensus Layer       Block Status
┌────────────┐       │                  │                      │          B[n-1] B[n] B[n+1]
│    Pre-    │       │                  │   Request Batch      │           ┌──┐  ┌──┐  ┌──┐
│ Consensus  │       │                  │<─────────────────────│           │✓ │  │QC│  │N │
│            │       │<─────────────-───│     Get Batch        │           └──┘  └──┘  └──┘
│            │       │   Return Batch   │                      │          ✓: committed
│            │       │─────────────────>│    Return Batch      │          N: new block
│            │       │                  │─────────────────────>│       
│            │       │                  │                      │
├────────────┤       │                  │                      │          B[n-1] B[n] B[n+1]
│            │       │                  │    Order Block       │           ┌──┐  ┌──┐  ┌──┐
│ Consensus  │       │                  │<─────────────────────│           │✓ │  │⚡│  │QC│
│            │       │<─────────────────│    Execute Block     │           └──┘  └──┘  └──┘
│            │       │   Return Result  │                      │          ⚡:  executable
│            │       │─────────────────>│    Return Result     │          QC: has a quorum cert
│            │       │                  │─────────────────────>│
│            │       │                  │                      │
├────────────┤       │                  │                      │          B[n-1] B[n] B[n+1]
│   Post-    │       │                  │    Commit Block      │           ┌──┐  ┌──┐  ┌──┐
│ Consensus  │       │                  │<─────────────────────│           │✓ │  │✓ │  │QC│
│            │       │<─────────────────│    Commit Block      │           └──┘  └──┘  └──┘
│            │       │                  │                      │          
└────────────┘       │                  │                      │
```

```
Time 

t0     Pre         Consensus         Pos           Block Status
     Block N                                       N   → [QuruomStore]
        │                                          N+1 → [Pending]
        │                                          N+2 → [Pending]
        ▼
t1   Block N+1     Block N                         N   → [Consensus]
        │              │                           N+1 → [QuruomStore]
        │              │                           N+2 → [Pending]
        ▼              ▼
t2   Block N+1      Block N+1      Block N         N   → [Execution]
        |              │              │            N+1 → [Consensus]
        │              │              │            N+2 → [QuruomStore]
        ▼              ▼              ▼
t3                  Block N+2      Block N+1       N   → [Commited]
                                                   N+1 → [Execution]
                                                   N+2 → [Consensus]
```

### Detailed Stages

### 1. Pre-Consensus Stage

The **Pre-Consensus Stage** focuses on preparing transactions for the consensus process. This stage ensures that transactions are available and ready for consensus by collecting signatures that verify their availability.

Key Components:

- **Transaction Broadcasting:**

  Incoming transactions are broadcast to all peer nodes in the network. This process makes the network aware of new transactions that will eventually be proposed for consensus.

- **Quorum Store (Work in Progress):**

  The protocol aggregates quorum signatures from validators, generating a **Proof of Available (PoAv)**. The **PoAv** is a cryptographic proof that confirms the availability and accessibility of a transaction batch. This means that a sufficient number of validators have acknowledged that they can access the transactions, ensuring the batch is ready for consensus without needing to send the actual transactions again.


### 2. Consensus Stage

The **Consensus Stage** is where the network reaches agreement on the next block to be executed, ensuring the block complies with the 2-chain safety rules.

Key Components:

- **View Change:**

  If the current leader fails to propose a block within the expected time, a view change is triggered to elect a new leader. This process follows the **2-chain safety rules**, ensuring the new leader proposes a block consistent with the previous quorum certificate (QC) or timeout certificate (TC).

- **Block Proposal and Validation:**

  The leader selects transactions and packages them into a block for proposal. If the transactions have a **Proof of Available (PoAv)** in the **Quorum Store**, the leader can propose the **PoAv** instead of the full transactions. This optimizes the consensus process by reducing the data validators need to check.

  To ensure 2-chain safety:

    1. The new block can only be proposed if it follows a block with a QC or TC.
    2. A block is valid for ordering only if its child block has a QC or TC, ensuring chain continuity.
- **Quorum Certificate (QC) Formation:**

  Once the block (or **PoAv**) is validated, signatures from a sufficient number of validators are collected to form a **Quorum Certificate (QC)**, confirming the block's acceptance by the majority.

- **Execution Request:**

  After reaching consensus, the block or PoAv is sent to the execution engine for processing, adhering to the **2-chain safety rules**.


### 3. Post-Consensus Stage

The **Post-Consensus Stage** is responsible for finalizing blocks and synchronizing state across the network. It operates in parallel with the consensus process, improving system throughput by allowing multiple stages (execution, result broadcasting, and commitment) to run simultaneously.

- **Execution Phase**:

  Once a block receives a Quorum Certificate (QC), its parent block is sent to the execution engine. The engine processes transactions independently from consensus, and the execution results are signed with the node’s private key. This separation ensures that consensus can continue on new blocks while previous ones are being executed.

- **Result Broadcasting**:

  After executing a block, each node broadcasts its signed execution results to peers. This reduces the computational burden, as nodes can rely on each other's results, ensuring faster convergence.

- **Signature Collection**:

  Nodes collect execution result signatures from peers. A threshold of **2f + 1** valid signatures is required to finalize the block, ensuring fault tolerance. Signatures are verified against validator public keys to maintain security.

- **Commitment**:

  Once enough signatures are collected, the block is marked as **committed**. At this point, the block’s state becomes final and immutable. Nodes trigger state synchronization to ensure all peers have the correct state, and a commit notification is broadcast across the network.


This stage leverages **pipeline** between consensus, execution, and commitment. While the consensus layer processes new blocks, the execution engine processes previously validated blocks, and nodes collect signatures for commitment in parallel. This pipelined approach maximizes throughput and scalability, ensuring that multiple blocks are processed at different stages simultaneously.

The network message flow for this stage can be visualized as follows:

```
Time    N1           N2           N3      
|       |            |            |       
|    ┌─────────────────────────────┐       # Each node executes and signs the block
|    |     Execute + Sign          |       
|    └─────────────────────────────┘       
|       |            |            |       
|    ←  ↓  →     ←   ↓  →     ←   ↓  →     # Nodes broadcast signed results to all peers
|       |            |            |       
|    ┌─────────────────────────────┐       # Collect 2f+1 valid signatures
|    |   Collect 2f+1 Signatures   |       
|    └─────────────────────────────┘       
|       |            |            |       
|    ┌─────────────────────────────┐       # Commit block after threshold signatures
|    |       Commit Block          |       
|    └─────────────────────────────┘       
|       |            |            |       
```

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
