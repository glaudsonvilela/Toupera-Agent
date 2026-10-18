// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

/// @dev Synthetic, self-contained accounting fixture for the M7 local pilot.
/// There is no token, custody, or production deployment behind this contract.
contract RoundingVaultM7Fixture {
    uint256 public totalAssets;
    uint256 public totalSupply;
    uint256 public feeBps;
    bool public faultyPreview;

    constructor(bool faulty) {
        totalAssets = 9;
        totalSupply = 10;
        feeBps = 100;
        faultyPreview = faulty;
    }

    function feeFor(uint256 assets) public view returns (uint256) {
        return (assets * feeBps + 9_999) / 10_000;
    }

    function previewWithdraw(uint256 assets) public view returns (uint256) {
        uint256 numerator = (assets + feeFor(assets)) * totalSupply;
        if (faultyPreview) return numerator / totalAssets;
        return (numerator + totalAssets - 1) / totalAssets;
    }

    function withdraw(uint256 assets) public returns (uint256 shares) {
        require(assets > 0 && assets <= 7, "valid synthetic amount");
        uint256 numerator = (assets + feeFor(assets)) * totalSupply;
        shares = (numerator + totalAssets - 1) / totalAssets;
        require(shares <= totalSupply, "valid synthetic shares");
        totalAssets -= assets;
        totalSupply -= shares;
    }
}
