// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

import "../src/Target.sol";

contract Scenario {
    function testReproduce() public {
        RoundingVaultM7Fixture vault = new RoundingVaultM7Fixture(false);
        uint256 preview = vault.previewWithdraw(1);
        uint256 charged = vault.withdraw(1);
        assert(preview < charged);
    }

    function testFuzzBoundaries(uint256 raw) public {
        uint256 assets = 1 + (raw % 7);
        RoundingVaultM7Fixture vault = new RoundingVaultM7Fixture(false);
        uint256 preview = vault.previewWithdraw(assets);
        uint256 charged = vault.withdraw(assets);
        assert(preview == charged && charged > 0);
    }
}
