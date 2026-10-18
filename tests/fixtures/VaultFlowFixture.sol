// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

// Analysis fixture only: accounting flow, not a deployable vault.
contract VaultFlowFixture {
    uint256 public totalAssets;
    uint256 public totalSupply;

    modifier active() {
        require(totalAssets > 0, "inactive");
        _;
    }

    function convertToShares(uint256 assets) public view returns (uint256 shares) {
        if (totalSupply == 0) {
            return assets;
        }
        return assets * totalSupply / totalAssets;
    }

    function previewDeposit(uint256 assets) external view active returns (uint256) {
        return convertToShares(assets);
    }
}
