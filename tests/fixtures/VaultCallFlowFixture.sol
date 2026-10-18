// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

// Analysis fixture only: source call paths, not a deployable vault.
interface ICallFlowOracle {
    function latestPrice() external view returns (uint256);
}

contract VaultCallFlowFixture {
    ICallFlowOracle public oracle;
    uint256 public totalAssets;
    uint256 public totalSupply;

    function deposit(uint256 assets) external returns (uint256 shares) {
        uint256 price = readPrice();
        shares = convertToShares(assets * price);
        totalAssets += assets;
        totalSupply += shares;
    }

    function readPrice() internal view returns (uint256) {
        return rawPrice();
    }

    function rawPrice() internal view returns (uint256) {
        return oracle.latestPrice();
    }

    function convertToShares(uint256 assets) public view returns (uint256) {
        return totalSupply == 0 ? assets : assets * totalSupply / totalAssets;
    }
}
