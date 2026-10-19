// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

interface IWCOP {
    function totalSupply() external view returns (uint256);
    function allowance(address owner, address spender) external view returns (uint256);
    function approve(address spender, uint256 value) external returns (bool);
}

contract WCOPReadOnlyFork {
    address internal constant TOKEN = 0x8a1D45e102e886510e891d2Ec656a708991e2D76;
    IWCOP internal constant WCOP = IWCOP(TOKEN);

    function testUnauthorizedMintCannotChangeSupply() public {
        uint256 beforeSupply = WCOP.totalSupply();
        (bool ok,) = TOKEN.call(abi.encodeWithSignature("mint(address,uint256)", address(this), 1));
        require(!ok, "unexpected mint success");
        require(WCOP.totalSupply() == beforeSupply, "supply changed");
    }

    function testUnauthorizedPauseIsRejected() public {
        (bool ok,) = TOKEN.call(abi.encodeWithSignature("pause()"));
        require(!ok, "unexpected pause success");
    }

    function testApprovalDoesNotChangeSupply() public {
        uint256 beforeSupply = WCOP.totalSupply();
        require(WCOP.approve(address(0xCAFE), 17), "approve failed");
        require(WCOP.allowance(address(this), address(0xCAFE)) == 17, "allowance mismatch");
        require(WCOP.totalSupply() == beforeSupply, "supply changed");
    }
}
