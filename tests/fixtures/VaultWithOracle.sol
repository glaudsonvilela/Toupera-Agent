// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

// Analysis fixture only: no token transfers; never deploy as a vault.
interface IPriceOracle {
    function latestPrice() external view returns (uint256);
}

contract VaultWithOracle {
    IPriceOracle public oracle;
    uint256 public totalAssets;
    uint256 public totalSupply;
    mapping(address => uint256) public balanceOf;

    constructor(IPriceOracle oracle_) {
        oracle = oracle_;
    }

    function deposit(uint256 assets) external returns (uint256 shares) {
        uint256 price = oracle.latestPrice();
        require(price > 0, "price");
        shares = totalSupply == 0 ? assets : assets * totalSupply / totalAssets;
        totalAssets += assets;
        totalSupply += shares;
        balanceOf[msg.sender] += shares;
    }

    function redeem(uint256 shares) external returns (uint256 assets) {
        require(balanceOf[msg.sender] >= shares, "shares");
        assets = shares * totalAssets / totalSupply;
        balanceOf[msg.sender] -= shares;
        totalSupply -= shares;
        totalAssets -= assets;
    }
}
