// SPDX-License-Identifier: MIT
pragma solidity ^0.8.18;

contract RandomDice {
    
    // 
    address public lastRoller;
    uint256 public lastRollResult;
    uint256 public lastSeedUsed;

    event DiceRolled(address indexed roller, uint256 result, uint256 seed);

    /**
     * @dev 
     * */
    function rollDice() public {
        // 
        uint256 seed = block.difficulty; 

        // 
        uint256 result = (seed % 6) + 1;

        // 
        lastRoller = msg.sender;
        lastRollResult = result;
        lastSeedUsed = seed;

        emit DiceRolled(msg.sender, result, seed);
    }

    /**
     * @dev 
     * */
    function getLatestRoll() public view returns (address, uint256, uint256) {
        return (lastRoller, lastRollResult, lastSeedUsed);
    }
}