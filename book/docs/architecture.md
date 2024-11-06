# Gravity SDK Architecture Design

## 1. System Overview

Gravity SDK is a high-performance consensus engine built on the Aptos-BFT consensus algorithm. It implements a three-phase consensus pipeline architecture, providing a modular and scalable framework for blockchain systems.

## 2. Core Architecture Components

### 2.1 Pre-Consensus Phase (Quorum Store. WIP)

#### Components:
- **Transaction Aggregator**: Collects and batches incoming transactions
- **Signature Distribution System**: Broadcasts transaction network-wide and collect signature
- **POS (Proof of Store) Generator**: Creates cryptographic proofs for transaction validation

#### Workflow:
1. Transaction batch formation
2. Network-wide transaction distribution and collect signature
3. POS generation upon reaching signature threshold

### 2.2 Consensus Phase (Aptos-BFT Core)

#### Components:
- **View Manager**: Handles leader rotation and view changes
- **Block Generator**: Creates block proposals with validated transactions or POS
- **QC (Quorum Certificate) Aggregator**: Collects and verifies voting certificates
- **Block Sequencer**: Establishes final block ordering

#### Workflow:
1. Leader election through view change mechanism
2. Block proposal generation with parent block reference
3. Network-wide proposal distribution
4. QC collection and verification
5. Block sequence finalization

### 2.3 Post-Consensus Phase (Execution Consensus)

#### Components:
- **Execution Result Aggregator**: Collects and validates execution results and broadcast execution results
- **Signature Management System**: Handles validator signatures
- **Commitment Processor**: Manages final block commitment

#### Workflow:
1. Result collection from execution layer
2. Signature gathering and validation
3. Threshold signature verification
4. Block finalization and commitment

## 4. Pipeline Architecture

```
Transaction Flow:
[Transaction Input] → [Pre-Consensus] → [Consensus] → [Post-Consensus] → [Commitment]
                          ↓                 ↓               ↓
                    [Quorum Store] → [Aptos-BFT Core] → [Result Processing]
                          ↓                 ↓               ↓
                     [POS Gen] → [Block Ordering] → [Block Commitment]
```

## 5. Performance Characteristics

- **Throughput**: Optimized for high transaction processing
- **Latency**: Minimized block confirmation time
- **Scalability**: Support for dynamic validator sets
- **Fault Tolerance**: Byzantine fault tolerance up to f=(n-1)/3