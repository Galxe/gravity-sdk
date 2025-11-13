// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract SimpleStorage {
    uint256 private value;
    
    event ValueChanged(uint256 indexed newValue, address indexed changer);
    
    constructor() {
        value = 42;
    }
    
    function getValue() external view returns (uint256) {
        return value;
    }
    
    function setValue(uint256 _value) external {
        value = _value;
        emit ValueChanged(_value, msg.sender);
    }
}