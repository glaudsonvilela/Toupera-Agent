// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

// Analysis fixture only: modifier effects, not a deployable vault.
contract ModifierFlowFixture {
    uint256 public totalAssets;
    uint256 public totalSupply;

    modifier checkedAndAccounted(uint256 minAssets) {
        require(totalAssets >= minAssets, "assets");
        assert(totalSupply < 1000);
        _;
        totalSupply += 1;
    }

    modifier charged() {
        totalSupply += 2;
        _;
        totalSupply += 3;
    }

    function deposit(uint256 assets) external checkedAndAccounted(assets) returns (uint256 shares) {
        totalAssets += assets;
        return assets;
    }


    function mint(uint256 assets) external charged checkedAndAccounted(assets) returns (uint256 shares) {
        totalAssets += assets;
        return assets;
    }
}
