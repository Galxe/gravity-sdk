use alloy_primitives::{address, Address};
use std::fmt::{Debug, Formatter};

/// ValidatorManagement contract address (from SystemAddresses.VALIDATOR_MANAGER)
pub const VALIDATOR_MANAGER_ADDRESS: Address = address!("00000000000000000000000000000001625F2001");

/// Staking contract address (from SystemAddresses.STAKING)
pub const STAKING_ADDRESS: Address = address!("00000000000000000000000000000001625F2000");

// Define contract interface using alloy_sol_macro
alloy_sol_macro::sol! {
    // ============================================================================
    // VALIDATOR STATUS
    // ============================================================================

    /// Validator lifecycle status (v2 - note the order!)
    enum ValidatorStatus {
        INACTIVE,         // 0: Not in validator set
        PENDING_ACTIVE,   // 1: Queued to join next epoch
        ACTIVE,           // 2: Currently validating
        PENDING_INACTIVE  // 3: Queued to leave next epoch
    }

    // ============================================================================
    // VALIDATOR TYPES (from Types.sol)
    // ============================================================================

    /// Validator consensus info (for consensus engine)
    struct ValidatorConsensusInfo {
        address validator;           // Validator identity address (= stakePool)
        bytes consensusPubkey;       // BLS public key for consensus
        bytes consensusPop;          // Proof of possession for BLS key
        uint256 votingPower;         // Voting power derived from bond
        uint64 validatorIndex;       // Index in active validator array
        bytes networkAddresses;      // Network addresses for P2P
        bytes fullnodeAddresses;     // Fullnode addresses for sync
    }

    /// Full validator record
    struct ValidatorRecord {
        address validator;           // Immutable validator identity address
        string moniker;              // Display name (max 31 bytes)
        uint8 status;                // ValidatorStatus enum value
        uint256 bond;                // Current validator bond (voting power snapshot)
        bytes consensusPubkey;       // BLS consensus public key
        bytes consensusPop;          // Proof of possession for BLS key
        bytes networkAddresses;      // Network addresses for P2P
        bytes fullnodeAddresses;     // Fullnode addresses
        address feeRecipient;        // Current fee recipient address
        address pendingFeeRecipient; // Pending fee recipient (applied next epoch)
        address stakingPool;         // Address of the StakePool
        uint64 validatorIndex;       // Index in active validator array
    }

    // ============================================================================
    // VALIDATOR MANAGEMENT CONTRACT (v2)
    // ============================================================================

    contract ValidatorManagement {
        // === Registration ===
        function registerValidator(
            address stakePool,
            string calldata moniker,
            bytes calldata consensusPubkey,
            bytes calldata consensusPop,
            bytes calldata networkAddresses,
            bytes calldata fullnodeAddresses
        ) external;

        // === Lifecycle ===
        function joinValidatorSet(address stakePool) external;
        function leaveValidatorSet(address stakePool) external;

        // === Operator Functions ===
        function rotateConsensusKey(
            address stakePool,
            bytes calldata newPubkey,
            bytes calldata newPop
        ) external;
        function setFeeRecipient(address stakePool, address newRecipient) external;

        // === View Functions ===
        function getValidator(address stakePool) external view returns (ValidatorRecord memory);
        function getActiveValidators() external view returns (ValidatorConsensusInfo[] memory);
        function getActiveValidatorByIndex(uint64 index) external view returns (ValidatorConsensusInfo memory);
        function getTotalVotingPower() external view returns (uint256);
        function getActiveValidatorCount() external view returns (uint256);
        function isValidator(address stakePool) external view returns (bool);
        function getValidatorStatus(address stakePool) external view returns (uint8);
        function getCurrentEpoch() external view returns (uint64);
        function getPendingActiveValidators() external view returns (ValidatorConsensusInfo[] memory);
        function getPendingInactiveValidators() external view returns (ValidatorConsensusInfo[] memory);

        // === Events ===
        event ValidatorRegistered(address indexed stakePool, string moniker);
        event ValidatorJoinRequested(address indexed stakePool);
        event ValidatorActivated(address indexed stakePool, uint64 validatorIndex, uint256 votingPower);
        event ValidatorLeaveRequested(address indexed stakePool);
        event ValidatorDeactivated(address indexed stakePool);
        event ConsensusKeyRotated(address indexed stakePool, bytes newPubkey);
        event FeeRecipientUpdated(address indexed stakePool, address newRecipient);
        event EpochProcessed(uint64 epoch, uint256 activeCount, uint256 totalVotingPower);
    }

    // ============================================================================
    // STAKING CONTRACT (for creating StakePools)
    // ============================================================================

    contract Staking {
        /// Create a new StakePool
        function createPool(
            address owner,
            address staker,
            address operator,
            address voter,
            uint64 lockedUntil
        ) external payable returns (address pool);

        /// Check if an address is a valid pool
        function isPool(address pool) external view returns (bool);

        /// Get pool's voting power at a given time
        function getPoolVotingPower(address pool, uint64 atTime) external view returns (uint256);

        /// Get pool's current voting power
        function getPoolVotingPowerNow(address pool) external view returns (uint256);

        /// Get pool's operator
        function getPoolOperator(address pool) external view returns (address);

        /// Get pool's owner
        function getPoolOwner(address pool) external view returns (address);

        /// Get pool's lockup expiration
        function getPoolLockedUntil(address pool) external view returns (uint64);

        /// Get pool's active stake
        function getPoolActiveStake(address pool) external view returns (uint256);

        /// Get total pool count
        function getPoolCount() external view returns (uint256);

        /// Get pool by index
        function getPool(uint256 index) external view returns (address);

        /// Get all pools
        function getAllPools() external view returns (address[] memory);

        // === Events ===
        event PoolCreated(
            address indexed creator,
            address indexed pool,
            address indexed owner,
            address staker,
            uint256 poolIndex
        );
    }
}

impl Debug for ValidatorStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidatorStatus::INACTIVE => write!(f, "INACTIVE"),
            ValidatorStatus::PENDING_ACTIVE => write!(f, "PENDING_ACTIVE"),
            ValidatorStatus::ACTIVE => write!(f, "ACTIVE"),
            ValidatorStatus::PENDING_INACTIVE => write!(f, "PENDING_INACTIVE"),
            _ => write!(f, "UNKNOWN"),
        }
    }
}

/// Helper to convert u8 to ValidatorStatus
pub fn status_from_u8(value: u8) -> ValidatorStatus {
    match value {
        0 => ValidatorStatus::INACTIVE,
        1 => ValidatorStatus::PENDING_ACTIVE,
        2 => ValidatorStatus::ACTIVE,
        3 => ValidatorStatus::PENDING_INACTIVE,
        _ => ValidatorStatus::__Invalid,
    }
}
