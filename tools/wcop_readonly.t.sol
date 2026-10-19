// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

interface Vm {
    function envAddress(string calldata name) external returns (address);
    function prank(address sender) external;
}

interface IWCOP {
    function totalSupply() external view returns (uint256);
    function balanceOf(address account) external view returns (uint256);
    function allowance(address owner, address spender) external view returns (uint256);
    function approve(address spender, uint256 value) external returns (bool);
    function transfer(address recipient, uint256 value) external returns (bool);
    function transferFrom(address sender, address recipient, uint256 value) external returns (bool);
}

contract WCOPReadOnlyFork {
    address internal constant TOKEN = 0x8a1D45e102e886510e891d2Ec656a708991e2D76;
    Vm internal constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));
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

    function testTransferRoundTrip() public {
        address holder = vm.envAddress("WCOP_HOLDER");
        uint256 beforeHolder = WCOP.balanceOf(holder);
        uint256 beforeLocal = WCOP.balanceOf(address(this));
        uint256 beforeSupply = WCOP.totalSupply();
        require(beforeHolder >= 1, "holder has no balance");
        vm.prank(holder);
        require(WCOP.transfer(address(this), 1), "transfer failed");
        require(WCOP.balanceOf(holder) == beforeHolder - 1, "holder delta");
        require(WCOP.balanceOf(address(this)) == beforeLocal + 1, "local delta");
        require(WCOP.transfer(holder, 1), "return failed");
        require(WCOP.balanceOf(holder) == beforeHolder, "holder not restored");
        require(WCOP.balanceOf(address(this)) == beforeLocal, "local not restored");
        require(WCOP.totalSupply() == beforeSupply, "supply changed");
    }

    function testAllowanceSpendAndReset() public {
        address holder = vm.envAddress("WCOP_HOLDER");
        address spender = address(0xCAFE);
        uint256 beforeHolder = WCOP.balanceOf(holder);
        uint256 beforeLocal = WCOP.balanceOf(address(this));
        uint256 beforeSupply = WCOP.totalSupply();
        require(beforeHolder >= 1, "holder has no balance");
        vm.prank(holder);
        require(WCOP.approve(spender, 1), "approval failed");
        vm.prank(spender);
        require(WCOP.transferFrom(holder, address(this), 1), "spend failed");
        require(WCOP.allowance(holder, spender) == 0, "allowance not consumed");
        require(WCOP.balanceOf(holder) == beforeHolder - 1, "holder delta");
        require(WCOP.balanceOf(address(this)) == beforeLocal + 1, "local delta");
        require(WCOP.transfer(holder, 1), "return failed");
        require(WCOP.balanceOf(holder) == beforeHolder, "holder not restored");
        require(WCOP.balanceOf(address(this)) == beforeLocal, "local not restored");
        require(WCOP.totalSupply() == beforeSupply, "supply changed");
    }
}
