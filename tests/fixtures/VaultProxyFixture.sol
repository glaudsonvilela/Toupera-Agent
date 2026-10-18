// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

// Analysis fixture only: no access control; never deploy as a proxy.
contract VaultProxyFixture {
    address public implementation;

    constructor(address implementation_) {
        implementation = implementation_;
    }

    fallback() external payable {
        (bool ok, ) = implementation.delegatecall(msg.data);
        require(ok, "delegatecall failed");
    }
}
