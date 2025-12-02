use alloy_primitives::{address, Address};
use std::fmt::{Debug, Formatter};

pub const VALIDATOR_MANAGER_ADDRESS: Address =
    address!("0x0000000000000000000000000000000000002013");

// Define contract interface using alloy_sol_macro
alloy_sol_macro::sol! {
    // Commission structure
    struct Commission {
        uint64 rate; // the commission rate charged to delegators(10000 is 100%)
        uint64 maxRate; // maximum commission rate which validator can ever charge
        uint64 maxChangeRate; // maximum daily increase of the validator commission
    }

    enum ValidatorStatus {
        PENDING_ACTIVE, // 0
        ACTIVE, // 1
        PENDING_INACTIVE, // 2
        INACTIVE // 3
    }

    // Validator registration parameters
    struct ValidatorRegistrationParams {
        bytes consensusPublicKey;
        bytes blsProof; // BLS proof
        Commission commission; // Changed from uint64 commissionRate to Commission struct
        string moniker;
        address initialOperator;
        address initialBeneficiary; // Passed directly to StakeCredit
        // Network addresses for Aptos compatibility
        bytes validatorNetworkAddresses; // BCS serialized Vec<NetworkAddress>
        bytes fullnodeNetworkAddresses; // BCS serialized Vec<NetworkAddress>
        bytes aptosAddress; // Aptos validator address
    }

    struct ValidatorInfo {
        // Basic information (from ValidatorManager)
        bytes consensusPublicKey;
        Commission commission;
        string moniker;
        bool registered;
        address stakeCreditAddress;
        ValidatorStatus status;
        uint256 votingPower; // Changed from uint64 to uint256 to prevent overflow
        uint256 validatorIndex;
        uint256 updateTime;
        address operator;
        bytes validatorNetworkAddresses; // BCS serialized Vec<NetworkAddress>
        bytes fullnodeNetworkAddresses; // BCS serialized Vec<NetworkAddress>
        bytes aptosAddress; // Aptos validator address
    }

    struct ValidatorSetData {
        uint256 totalVotingPower; // Total voting power - Changed from uint128 to uint256
        uint256 totalJoiningPower; // Total pending voting power - Changed from uint128 to uint256
    }

    struct ValidatorSet {
        ValidatorInfo[] activeValidators; // Active validators for the current epoch
        ValidatorInfo[] pendingInactive; // Pending validators to leave in next epoch (still active)
        ValidatorInfo[] pendingActive; // Pending validators to join in next epoch
        uint256 totalVotingPower; // Current total voting power
        uint256 totalJoiningPower; // Total voting power waiting to join in the next epoch
    }

    contract ValidatorManager {
        function registerValidator(
            ValidatorRegistrationParams calldata params
        ) external payable;

        function joinValidatorSet(address validator) external;

        function leaveValidatorSet(address validator) external;

        function getValidatorInfo(
            address validator
        ) external view returns (ValidatorInfo memory);

        function isValidatorRegistered(address validator) external view returns (bool);

        function getValidatorStatus(address validator) external view returns (uint8);

        function getValidatorSetData() external view returns (ValidatorSetData memory);

        function getValidatorSet() external view returns (ValidatorSet memory);

        event ValidatorRegistered(
            address indexed validator,
            address indexed operator,
            bytes consensusPublicKey,
            string moniker
        );

        event ValidatorJoinRequested(
            address indexed validator,
            uint256 votingPower,
            uint64 epoch
        );

        event ValidatorLeaveRequested(
            address indexed validator,
            uint64 epoch
        );

        event ValidatorStatusChanged(
            address indexed validator,
            uint8 oldStatus,
            uint8 newStatus,
            uint64 epoch
        );
    }
}

impl Debug for ValidatorStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidatorStatus::PENDING_ACTIVE => write!(f, "PENDING_ACTIVE"),
            ValidatorStatus::ACTIVE => write!(f, "ACTIVE"),
            ValidatorStatus::PENDING_INACTIVE => write!(f, "PENDING_INACTIVE"),
            ValidatorStatus::INACTIVE => write!(f, "INACTIVE"),
            _ => write!(f, "UNKNOWN"),
        }
    }
}
