// SPDX-License-Identifier: MIT
pragma solidity ^0.8.21;

contract HistoryReader {
    address constant HISTORY = 0x0000F90827F1C53a10cb7A02335B175320002935;

    function getHash(uint256 n) external view returns (bytes32) {
        (bool ok, bytes memory ret) = HISTORY.staticcall(abi.encode(n));
        require(ok && ret.length == 32, "history miss");
        return abi.decode(ret, (bytes32));
    }
}
