// SPDX-License-Identifier: MIT
pragma solidity ^0.8.21;

contract Counter {
    uint256 public value;

    function set(uint256 x) external {
        value = x;
    }

    function get() external view returns (uint256) {
        return value;
    }
}
