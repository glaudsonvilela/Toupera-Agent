// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

// Analysis fixture only: inheritance and economic paths, not a deployable vault.
interface IInheritedOracle {
    function latestPrice() external view returns (uint256);
}

abstract contract BaseVault {
    IInheritedOracle public oracle;
    uint256 public totalAssets;
    uint256 public totalSupply;

    constructor(IInheritedOracle oracle_) {
        oracle = oracle_;
    }

    modifier active() {
        require(totalAssets > 0, "inactive");
        _;
    }

    function readPrice() internal view returns (uint256) {
        return oracle.latestPrice();
    }

    function convertToShares(uint256 assets) public view virtual returns (uint256) {
        return totalSupply == 0 ? assets : assets * totalSupply / totalAssets;
    }
}

contract InheritedVaultFixture is BaseVault {
    constructor(IInheritedOracle oracle_) BaseVault(oracle_) {}

    function deposit(uint256 assets) external active returns (uint256 shares) {
        shares = convertToShares(assets * readPrice());
        totalAssets += assets;
        totalSupply += shares;
    }
}
