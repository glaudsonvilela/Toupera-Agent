// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

import {LatamStable} from "src/LatamStable.sol";
import {LocalProxy} from "test/WcopCoverage.t.sol";

contract WCOPFormal {
    LatamStable private token;

    function setUp() public {
        LatamStable implementation = new LatamStable();
        bytes memory initData = abi.encodeWithSelector(
            LatamStable.initialize.selector, address(this), address(this), address(this),
            address(this), "Local wCOP", "wCOP");
        token = LatamStable(address(new LocalProxy(address(implementation), initData)));
    }

    function check_MintBurnRestoresSupply(uint96 amount) public {
        uint256 beforeSupply = token.totalSupply();
        token.mint(address(this), uint256(amount));
        token.burn(uint256(amount));
        assert(token.totalSupply() == beforeSupply);
    }
}
