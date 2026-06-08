// SPDX-License-Identifier: MIT
pragma solidity ^0.8.18;

/// @title GravityPrevRandao
/// @notice Thin Solidity wrapper around Gravity's prevRandao-by-height precompile.
/// @dev Usage overview:
/// - `prevRandao()` is the default low-cost current-block path. It reads
///   Solidity's native `block.prevrandao` value and does not call the precompile.
/// - `tryHistoricalPrevRandaoAt(height)` is the non-reverting historical path.
///   It returns `(false, 0)` when the target header is unavailable.
/// - `historicalPrevRandaoAt(height)` is the reverting historical path for
///   applications that require a historical value to exist.
/// - `derive(...)`, `uint256Value(...)`, and range helpers derive application
///   randomness from a raw prevRandao value plus domain, consumer, caller, chain
///   id, current block number, and nonce.
/// - Applications that need off-chain replay should record the consumed nonce
///   and execution block number together with the randomness height.
///
/// The precompile lives at 0x00000000000000000000000000000001625f5002.
/// Input: `uint256 block height`.
/// Output: `(uint256 found, bytes32 prevRandao)`, where `found == 1` means the
/// header exists and `prevRandao` is the header mixHash / prevRandao value.
///
/// Security notes:
/// - prevRandao is suitable for low-to-medium value randomness, but validators
///   may still have influence over block production. Do not use it as the only
///   entropy source for high-value adversarial games.
/// - Always use domain separation per feature, for example
///   `keccak256("MY_APP_LOOT_DROP")`.
/// - Prefer current-block helpers when possible. Historical helpers are useful
///   when the application explicitly needs a previous block's value.
library GravityPrevRandao {
    address internal constant PREVRANDAO_BY_HEIGHT_PRECOMPILE = 0x00000000000000000000000000000001625f5002;

    bytes32 internal constant DEFAULT_DOMAIN = keccak256("GRAVITY_PREVRANDAO");

    error PrevRandaoLookupFailed(uint256 height);
    error EmptyRandomnessRange();
    error RejectionSamplingFailed();

    /// @notice Return the current block prevRandao using Solidity's native opcode.
    /// @dev This is the default, lowest-cost path. Use the historical helpers
    /// only when the application explicitly needs prevRandao from another block.
    function prevRandao() internal view returns (bytes32) {
        return bytes32(block.prevrandao);
    }

    /// @notice Query historical block prevRandao by block height.
    /// @dev Uses Gravity's by-height precompile. Returns `(false, 0)` when the
    /// precompile is unavailable, the block is unknown, or the response shape
    /// is invalid.
    function tryHistoricalPrevRandaoAt(uint256 height) internal view returns (bool found, bytes32 value) {
        (bool ok, bytes memory out) = PREVRANDAO_BY_HEIGHT_PRECOMPILE.staticcall(abi.encode(height));

        if (!ok || out.length != 64) {
            return (false, bytes32(0));
        }

        uint256 foundWord;
        (foundWord, value) = abi.decode(out, (uint256, bytes32));
        found = foundWord == 1;
        if (!found) {
            value = bytes32(0);
        }
    }

    /// @notice Query historical block prevRandao and revert if it is unavailable.
    function historicalPrevRandaoAt(uint256 height) internal view returns (bytes32 value) {
        bool found;
        (found, value) = tryHistoricalPrevRandaoAt(height);
        if (!found) {
            revert PrevRandaoLookupFailed(height);
        }
    }

    /// @notice Derive one 32-byte random value from raw block prevRandao.
    /// @dev Mirrors Aptos' pattern of domain separation plus transaction-local
    /// entropy. Solidity cannot access the tx hash, so callers should pass a
    /// per-use nonce or inherit GravityPrevRandaoConsumer below.
    function derive(bytes32 rawPrevRandao, bytes32 domain, address consumer, address caller, uint256 nonce)
        internal
        view
        returns (bytes32)
    {
        return keccak256(
            abi.encodePacked(
                DEFAULT_DOMAIN, domain, rawPrevRandao, block.chainid, consumer, caller, block.number, nonce
            )
        );
    }

    /// @notice Derive a uint256 from current block prevRandao.
    function uint256Value(bytes32 domain, address consumer, address caller, uint256 nonce)
        internal
        view
        returns (uint256)
    {
        return uint256(derive(prevRandao(), domain, consumer, caller, nonce));
    }

    /// @notice Derive a uint256 from historical block prevRandao at `height`.
    function historicalUint256At(uint256 height, bytes32 domain, address consumer, address caller, uint256 nonce)
        internal
        view
        returns (uint256)
    {
        return uint256(derive(historicalPrevRandaoAt(height), domain, consumer, caller, nonce));
    }

    /// @notice Derive an unbiased value in [minInclusive, maxExclusive).
    function range(
        uint256 minInclusive,
        uint256 maxExclusive,
        bytes32 domain,
        address consumer,
        address caller,
        uint256 nonce
    ) internal view returns (uint256) {
        (uint256 value,) = rangeWithSample(minInclusive, maxExclusive, domain, consumer, caller, nonce);
        return value;
    }

    /// @notice Same as `range`, also returning the accepted random sample.
    function rangeWithSample(
        uint256 minInclusive,
        uint256 maxExclusive,
        bytes32 domain,
        address consumer,
        address caller,
        uint256 nonce
    ) internal view returns (uint256 value, bytes32 sampleBytes) {
        return _rangeWithSample(prevRandao(), minInclusive, maxExclusive, domain, consumer, caller, nonce);
    }

    /// @notice Derive an unbiased value from historical block prevRandao.
    function historicalRangeAt(
        uint256 height,
        uint256 minInclusive,
        uint256 maxExclusive,
        bytes32 domain,
        address consumer,
        address caller,
        uint256 nonce
    ) internal view returns (uint256) {
        (uint256 value,) = historicalRangeAtWithSample(
            height, minInclusive, maxExclusive, domain, consumer, caller, nonce
        );
        return value;
    }

    /// @notice Same as `historicalRangeAt`, also returning the accepted random sample.
    function historicalRangeAtWithSample(
        uint256 height,
        uint256 minInclusive,
        uint256 maxExclusive,
        bytes32 domain,
        address consumer,
        address caller,
        uint256 nonce
    ) internal view returns (uint256 value, bytes32 sampleBytes) {
        return _rangeWithSample(
            historicalPrevRandaoAt(height), minInclusive, maxExclusive, domain, consumer, caller, nonce
        );
    }

    function _rangeWithSample(
        bytes32 raw,
        uint256 minInclusive,
        uint256 maxExclusive,
        bytes32 domain,
        address consumer,
        address caller,
        uint256 nonce
    ) private view returns (uint256 value, bytes32 sampleBytes) {
        if (maxExclusive <= minInclusive) {
            revert EmptyRandomnessRange();
        }

        uint256 span = maxExclusive - minInclusive;

        // Rejection sampling avoids modulo bias. This will almost always return
        // on the first iteration for practical spans.
        uint256 limit = type(uint256).max - (type(uint256).max % span);
        for (uint256 salt = 0; salt < 256;) {
            sampleBytes = derive(raw, keccak256(abi.encodePacked(domain, salt)), consumer, caller, nonce);
            uint256 sample = uint256(sampleBytes);
            if (sample < limit) {
                value = minInclusive + (sample % span);
                return (value, sampleBytes);
            }
            unchecked {
                salt++;
            }
        }

        revert RejectionSamplingFailed();
    }
}

/// @notice Stateless convenience base contract for applications that consume prevRandao.
/// @dev The randomness framework does not own a global nonce. Applications should
/// pass a business identifier such as `requestId`, `rollId`, `roundId`, or
/// `ticketId` as the `nonce` argument. This keeps delayed randomness
/// replayable and prevents unrelated workflows from affecting each other.
///
/// Recommended application pattern:
/// 1. Inherit `GravityPrevRandaoConsumer`.
/// 2. Define one domain constant per random workflow.
/// 3. Allocate and persist a business nonce/request id before consuming randomness.
/// 4. Use `_random...` helpers for the common current-block path.
/// 5. Use `_historical...At` helpers only when the user explicitly asks for a
///    historical block height.
abstract contract GravityPrevRandaoConsumer {
    using GravityPrevRandao for uint256;

    /// @notice Derive current-block random bytes using an application-owned nonce.
    function _randomBytes32(bytes32 domain, uint256 nonce) internal view returns (bytes32) {
        return GravityPrevRandao.derive(GravityPrevRandao.prevRandao(), domain, address(this), msg.sender, nonce);
    }

    /// @notice Derive random bytes from a historical block using an application-owned nonce.
    function _historicalRandomBytes32At(uint256 height, bytes32 domain, uint256 nonce) internal view returns (bytes32) {
        return GravityPrevRandao.derive(
            GravityPrevRandao.historicalPrevRandaoAt(height), domain, address(this), msg.sender, nonce
        );
    }

    /// @notice Derive a current-block uint256.
    function _randomUint256(bytes32 domain, uint256 nonce) internal view returns (uint256) {
        return uint256(_randomBytes32(domain, nonce));
    }

    /// @notice Derive a historical uint256.
    function _historicalRandomUint256At(uint256 height, bytes32 domain, uint256 nonce) internal view returns (uint256) {
        return uint256(_historicalRandomBytes32At(height, domain, nonce));
    }

    /// @notice Return an unbiased current-block value in [minInclusive, maxExclusive).
    function _randomRange(uint256 minInclusive, uint256 maxExclusive, bytes32 domain, uint256 nonce)
        internal
        view
        returns (uint256)
    {
        (uint256 value,) = _randomRangeWithSample(minInclusive, maxExclusive, domain, nonce);
        return value;
    }

    /// @notice Return a current-block boolean.
    function _randomBool(bytes32 domain, uint256 nonce) internal view returns (bool) {
        return _randomRange(0, 2, domain, nonce) == 1;
    }

    /// @notice Return a current-block array index in [0, length).
    function _randomIndex(uint256 length, bytes32 domain, uint256 nonce) internal view returns (uint256) {
        return _randomRange(0, length, domain, nonce);
    }

    /// @notice Return a current-block range value plus the accepted sample bytes.
    function _randomRangeWithSample(uint256 minInclusive, uint256 maxExclusive, bytes32 domain, uint256 nonce)
        internal
        view
        returns (uint256, bytes32)
    {
        return GravityPrevRandao.rangeWithSample(minInclusive, maxExclusive, domain, address(this), msg.sender, nonce);
    }

    /// @notice Return an unbiased historical value in [minInclusive, maxExclusive).
    function _historicalRandomRangeAt(
        uint256 height,
        uint256 minInclusive,
        uint256 maxExclusive,
        bytes32 domain,
        uint256 nonce
    ) internal view returns (uint256) {
        (uint256 value,) = _historicalRandomRangeAtWithSample(height, minInclusive, maxExclusive, domain, nonce);
        return value;
    }

    /// @notice Return a historical boolean.
    function _historicalRandomBoolAt(uint256 height, bytes32 domain, uint256 nonce) internal view returns (bool) {
        return _historicalRandomRangeAt(height, 0, 2, domain, nonce) == 1;
    }

    /// @notice Return a historical array index in [0, length).
    function _historicalRandomIndexAt(uint256 height, uint256 length, bytes32 domain, uint256 nonce)
        internal
        view
        returns (uint256)
    {
        return _historicalRandomRangeAt(height, 0, length, domain, nonce);
    }

    /// @notice Return a historical range value plus the accepted sample bytes.
    function _historicalRandomRangeAtWithSample(
        uint256 height,
        uint256 minInclusive,
        uint256 maxExclusive,
        bytes32 domain,
        uint256 nonce
    ) internal view returns (uint256, bytes32) {
        return GravityPrevRandao.historicalRangeAtWithSample(
            height, minInclusive, maxExclusive, domain, address(this), msg.sender, nonce
        );
    }
}

/// @notice Example consumer that rolls a six-sided dice using Gravity prevRandao.
contract GravityRandomDice is GravityPrevRandaoConsumer {
    bytes32 private constant ROLL_DICE_DOMAIN = keccak256("GRAVITY_RANDOM_DICE_ROLL");

    uint256 public nextRollId;
    address public lastRoller;
    uint256 public lastRollResult;
    uint256 public lastRandomnessHeight;
    uint256 public lastExecutionHeight;
    uint256 public lastNonceUsed;
    bytes32 public lastDomain;
    bytes32 public lastRandomness;

    event DiceRolled(
        address indexed roller,
        bytes32 indexed domain,
        uint256 indexed randomnessHeight,
        uint256 executionHeight,
        uint256 nonce,
        uint256 result,
        bytes32 randomness
    );

    function rollDice() external {
        uint256 rollId = _nextRollId();
        (uint256 result, bytes32 randomness) = _randomRangeWithSample(1, 7, ROLL_DICE_DOMAIN, rollId);
        _rollDice(block.number, rollId, result, randomness);
    }

    function rollDiceAtCurrentBlock() external {
        uint256 rollId = _nextRollId();
        (uint256 result, bytes32 randomness) = _randomRangeWithSample(1, 7, ROLL_DICE_DOMAIN, rollId);
        _rollDice(block.number, rollId, result, randomness);
    }

    function rollDiceAtParentBlock() external {
        rollDiceFromHistory(block.number == 0 ? 0 : block.number - 1);
    }

    function rollDiceFromHistory(uint256 height) public {
        uint256 rollId = _nextRollId();
        (uint256 result, bytes32 randomness) =
            _historicalRandomRangeAtWithSample(height, 1, 7, ROLL_DICE_DOMAIN, rollId);
        _rollDice(height, rollId, result, randomness);
    }

    function _nextRollId() private returns (uint256 rollId) {
        rollId = nextRollId;
        unchecked {
            nextRollId = rollId + 1;
        }
    }

    function _rollDice(uint256 randomnessHeight, uint256 nonce, uint256 result, bytes32 randomness) private {
        lastRoller = msg.sender;
        lastRollResult = result;
        lastRandomnessHeight = randomnessHeight;
        lastExecutionHeight = block.number;
        lastNonceUsed = nonce;
        lastDomain = ROLL_DICE_DOMAIN;
        lastRandomness = randomness;

        emit DiceRolled(msg.sender, ROLL_DICE_DOMAIN, randomnessHeight, block.number, nonce, result, randomness);
    }

    function getLatestRoll()
        external
        view
        returns (
            address roller,
            uint256 result,
            uint256 randomnessHeight,
            uint256 executionHeight,
            uint256 nonce,
            bytes32 domain,
            bytes32 randomness
        )
    {
        return (
            lastRoller,
            lastRollResult,
            lastRandomnessHeight,
            lastExecutionHeight,
            lastNonceUsed,
            lastDomain,
            lastRandomness
        );
    }
}

/// @notice Test harness that exposes the library and consumer helpers for e2e
/// coverage. Production applications should use GravityPrevRandao directly or
/// inherit GravityPrevRandaoConsumer with application-owned nonces.
contract GravityPrevRandaoHarness is GravityPrevRandaoConsumer {
    bytes32 private constant HARNESS_DOMAIN = keccak256("GRAVITY_PREVRANDAO_HARNESS");

    function prevRandaoExternal() external view returns (bytes32 prevRandao) {
        return GravityPrevRandao.prevRandao();
    }

    function prevRandaoWithHeightExternal() external view returns (uint256 height, bytes32 prevRandao) {
        return (block.number, GravityPrevRandao.prevRandao());
    }

    function tryHistoricalPrevRandaoAtExternal(uint256 height) external view returns (bool found, bytes32 prevRandao) {
        return GravityPrevRandao.tryHistoricalPrevRandaoAt(height);
    }

    function historicalPrevRandaoAtExternal(uint256 height) external view returns (bytes32 prevRandao) {
        return GravityPrevRandao.historicalPrevRandaoAt(height);
    }

    function deriveExternal(bytes32 rawRandomness, bytes32 domain, address consumer, address caller, uint256 nonce)
        external
        view
        returns (bytes32)
    {
        return GravityPrevRandao.derive(rawRandomness, domain, consumer, caller, nonce);
    }

    function uint256External(bytes32 domain, address consumer, address caller, uint256 nonce)
        external
        view
        returns (uint256)
    {
        return GravityPrevRandao.uint256Value(domain, consumer, caller, nonce);
    }

    function historicalUint256AtExternal(
        uint256 height,
        bytes32 domain,
        address consumer,
        address caller,
        uint256 nonce
    ) external view returns (uint256) {
        return GravityPrevRandao.historicalUint256At(height, domain, consumer, caller, nonce);
    }

    function rangeExternal(
        uint256 minInclusive,
        uint256 maxExclusive,
        bytes32 domain,
        address consumer,
        address caller,
        uint256 nonce
    ) external view returns (uint256) {
        return GravityPrevRandao.range(minInclusive, maxExclusive, domain, consumer, caller, nonce);
    }

    function historicalRangeAtExternal(
        uint256 height,
        uint256 minInclusive,
        uint256 maxExclusive,
        bytes32 domain,
        address consumer,
        address caller,
        uint256 nonce
    ) external view returns (uint256) {
        return GravityPrevRandao.historicalRangeAt(height, minInclusive, maxExclusive, domain, consumer, caller, nonce);
    }

    function rangeWithSampleExternal(
        uint256 minInclusive,
        uint256 maxExclusive,
        bytes32 domain,
        address consumer,
        address caller,
        uint256 nonce
    ) external view returns (uint256 value, bytes32 sampleBytes) {
        return GravityPrevRandao.rangeWithSample(minInclusive, maxExclusive, domain, consumer, caller, nonce);
    }

    function historicalRangeAtWithSampleExternal(
        uint256 height,
        uint256 minInclusive,
        uint256 maxExclusive,
        bytes32 domain,
        address consumer,
        address caller,
        uint256 nonce
    ) external view returns (uint256 value, bytes32 sampleBytes) {
        return GravityPrevRandao.historicalRangeAtWithSample(
            height, minInclusive, maxExclusive, domain, consumer, caller, nonce
        );
    }

    function randomBytes32External(uint256 nonce) external view returns (bytes32) {
        return _randomBytes32(HARNESS_DOMAIN, nonce);
    }

    function historicalRandomBytes32AtExternal(uint256 height, uint256 nonce) external view returns (bytes32) {
        return _historicalRandomBytes32At(height, HARNESS_DOMAIN, nonce);
    }

    function randomUint256External(uint256 nonce) external view returns (uint256) {
        return _randomUint256(HARNESS_DOMAIN, nonce);
    }

    function historicalRandomUint256AtExternal(uint256 height, uint256 nonce) external view returns (uint256) {
        return _historicalRandomUint256At(height, HARNESS_DOMAIN, nonce);
    }

    function randomRangeExternal(uint256 minInclusive, uint256 maxExclusive, uint256 nonce)
        external
        view
        returns (uint256)
    {
        return _randomRange(minInclusive, maxExclusive, HARNESS_DOMAIN, nonce);
    }

    function randomBoolExternal(uint256 nonce) external view returns (bool) {
        return _randomBool(HARNESS_DOMAIN, nonce);
    }

    function randomIndexExternal(uint256 length, uint256 nonce) external view returns (uint256) {
        return _randomIndex(length, HARNESS_DOMAIN, nonce);
    }

    function historicalRandomRangeAtExternal(uint256 height, uint256 minInclusive, uint256 maxExclusive, uint256 nonce)
        external
        view
        returns (uint256)
    {
        return _historicalRandomRangeAt(height, minInclusive, maxExclusive, HARNESS_DOMAIN, nonce);
    }

    function historicalRandomBoolAtExternal(uint256 height, uint256 nonce) external view returns (bool) {
        return _historicalRandomBoolAt(height, HARNESS_DOMAIN, nonce);
    }

    function historicalRandomIndexAtExternal(uint256 height, uint256 length, uint256 nonce)
        external
        view
        returns (uint256)
    {
        return _historicalRandomIndexAt(height, length, HARNESS_DOMAIN, nonce);
    }

    function randomRangeWithSampleExternal(uint256 minInclusive, uint256 maxExclusive, uint256 nonce)
        external
        view
        returns (uint256 value, bytes32 sampleBytes)
    {
        return _randomRangeWithSample(minInclusive, maxExclusive, HARNESS_DOMAIN, nonce);
    }

    function historicalRandomRangeAtWithSampleExternal(
        uint256 height,
        uint256 minInclusive,
        uint256 maxExclusive,
        uint256 nonce
    ) external view returns (uint256 value, bytes32 sampleBytes) {
        return _historicalRandomRangeAtWithSample(height, minInclusive, maxExclusive, HARNESS_DOMAIN, nonce);
    }
}
