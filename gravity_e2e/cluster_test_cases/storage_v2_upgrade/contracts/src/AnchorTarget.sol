// SPDX-License-Identifier: MIT
pragma solidity ^0.8.21;

/// Minimal history generator for the storage_v2_baseline case.
///
/// One storage slot (slot 0) that the case rewrites at distinct blocks so
/// historical eth_getStorageAt reads (the primary changeset consumers) have
/// different expected values per block, plus an event per write so the
/// eth_getLogs / receipt-log anchors are non-trivially populated.
contract AnchorTarget {
    uint256 public value; // slot 0

    event ValueSet(address indexed setter, uint256 oldValue, uint256 newValue);

    function set(uint256 x) external {
        emit ValueSet(msg.sender, value, x);
        value = x;
    }
}
