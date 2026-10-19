// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

import "../src/Target.sol";

/// @dev Inheritance avoids Foundry's deployCode cheat code in a symbolic run.
contract SymbolicScenario is RoundingVaultM7Fixture {
    constructor() RoundingVaultM7Fixture(true) {}

    function check_previewMatchesExecution(uint256 assets) public {
        if (assets != 1) return;
        uint256 preview = previewWithdraw(assets);
        uint256 charged = withdraw(assets);
        assert(preview == charged);
    }
}
