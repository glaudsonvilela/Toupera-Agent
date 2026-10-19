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

    function check_TransferConservesSupply(uint96 minted, uint96 requested) public {
        uint256 amount = uint256(minted);
        uint256 moved = uint256(requested) % (amount + 1);
        token.mint(address(this), amount);
        token.transfer(address(0xA11CE), moved);
        assert(token.totalSupply() == amount);
        assert(token.balanceOf(address(this)) + token.balanceOf(address(0xA11CE)) == amount);
    }

    function check_PauseBlocksMovement(uint96 amount) public {
        token.mint(address(this), uint256(amount));
        token.pause();
        (bool moved,) = address(token).call(abi.encodeWithSelector(token.transfer.selector, address(0xA11CE), uint256(amount)));
        assert(!moved);
        assert(token.totalSupply() == uint256(amount));
        assert(token.balanceOf(address(this)) == uint256(amount));
    }
}
