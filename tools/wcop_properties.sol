// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

import {LatamStable} from "src/LatamStable.sol";

contract PropertyProxy {
    bytes32 private constant SLOT = 0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc;

    constructor(address implementation, bytes memory initData) {
        assembly { sstore(SLOT, implementation) }
        (bool ok,) = implementation.delegatecall(initData);
        require(ok, "init failed");
    }

    fallback() external payable {
        assembly {
            let implementation := sload(SLOT)
            calldatacopy(0, 0, calldatasize())
            let ok := delegatecall(gas(), implementation, 0, calldatasize(), 0, 0)
            returndatacopy(0, 0, returndatasize())
            switch ok
            case 0 { revert(0, returndatasize()) }
            default { return(0, returndatasize()) }
        }
    }
}

contract WCOPProperties {
    LatamStable private token;
    address private constant ALICE = address(0xA11CE);
    uint256 private expectedSupply;

    constructor() {
        LatamStable implementation = new LatamStable();
        bytes memory initData = abi.encodeWithSelector(
            LatamStable.initialize.selector, address(this), address(this), address(this),
            address(this), "Local wCOP", "wCOP");
        token = LatamStable(address(new PropertyProxy(address(implementation), initData)));
    }

    function issue(uint96 raw) external {
        uint256 amount = uint256(raw) % 1e18;
        token.mint(address(this), amount);
        expectedSupply += amount;
    }

    function issueAlice(uint96 raw) external {
        uint256 amount = uint256(raw) % 1e18;
        token.mint(ALICE, amount);
        expectedSupply += amount;
    }

    function reduce(uint96 raw) external {
        uint256 balance = token.balanceOf(address(this));
        uint256 amount = uint256(raw) % (balance + 1);
        token.burn(amount);
        expectedSupply -= amount;
    }

    function moveToAlice(uint96 raw) external {
        uint256 balance = token.balanceOf(address(this));
        uint256 amount = uint256(raw) % (balance + 1);
        token.transfer(ALICE, amount);
    }

    function setPaused(bool value) external {
        if (value && !token.paused()) token.pause();
        if (!value && token.paused()) token.unpause();
    }

    function echidna_supply_matches() public view returns (bool) {
        return token.totalSupply() == expectedSupply;
    }

    function echidna_balances_match() public view returns (bool) {
        return token.balanceOf(address(this)) + token.balanceOf(ALICE) == expectedSupply;
    }
}
