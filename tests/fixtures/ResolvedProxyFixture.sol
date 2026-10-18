// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

// Analysis fixture only: source evidence for a fixed implementation.
contract FixedVaultLogic {
    function convertToShares(uint256 assets) external pure returns (uint256) {
        return assets;
    }
}

contract ResolvedProxyFixture {
    address public immutable implementation;

    constructor() {
        implementation = address(new FixedVaultLogic());
    }

    fallback() external payable {
        (bool ok, ) = implementation.delegatecall(msg.data);
        require(ok, "delegatecall failed");
    }
}
