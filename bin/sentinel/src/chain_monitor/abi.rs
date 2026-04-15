//! ABI definitions for bridge contract events using alloy sol! macro.

alloy_sol_macro::sol! {
    // ========================================================================
    // GBridgeSender events (Ethereum side)
    // ========================================================================

    event TokensLocked(address indexed from, address indexed recipient, uint256 amount, uint128 indexed nonce);
    event EmergencyWithdraw(address indexed recipient, uint256 amount);
    event ERC20Recovered(address indexed token, address indexed recipient, uint256 amount);

    // ========================================================================
    // GravityPortal events (Ethereum side)
    // ========================================================================

    event MessageSent(uint128 indexed nonce, uint256 indexed block_number, bytes payload);
    event FeeConfigUpdated(uint256 baseFee, uint256 feePerByte);
    event FeeRecipientUpdated(address indexed oldRecipient, address indexed newRecipient);
    event FeesWithdrawn(address indexed recipient, uint256 amount);

    // ========================================================================
    // GBridgeReceiver events (Gravity side)
    // ========================================================================

    event NativeMinted(address indexed recipient, uint256 amount, uint128 indexed nonce);

    // ========================================================================
    // NativeOracle events (Gravity side)
    // ========================================================================

    event CallbackFailed(uint32 indexed sourceType, uint256 indexed sourceId, uint128 nonce, address callback, bytes reason);
    event DataRecorded(uint32 indexed sourceType, uint256 indexed sourceId, uint128 nonce, uint256 dataLength);

    // ========================================================================
    // OpenZeppelin Ownable2Step events
    // ========================================================================

    event OwnershipTransferStarted(address indexed previousOwner, address indexed newOwner);
    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);

    // ========================================================================
    // ERC20 balanceOf for vault balance monitoring
    // ========================================================================

    function balanceOf(address account) external view returns (uint256);

    // ========================================================================
    // Epoch monitoring (Gravity system contracts)
    // ========================================================================

    function currentEpoch() external view returns (uint64);
    function lastReconfigurationTime() external view returns (uint64);
    function epochIntervalMicros() external view returns (uint64);
}
