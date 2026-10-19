// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

import {LatamStable} from "src/LatamStable.sol";

interface VmCoverage {
    function prank(address sender) external;
    function addr(uint256 privateKey) external returns (address);
    function sign(uint256 privateKey, bytes32 digest) external returns (uint8, bytes32, bytes32);
}

contract LocalProxy {
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

contract WCOPSourceCoverage {
    VmCoverage private constant vm = VmCoverage(address(uint160(uint256(keccak256("hevm cheat code")))));
    address private constant ALICE = address(0xA11CE);
    address private constant BOB = address(0xB0B);
    LatamStable private token;
    LatamStable private implementation;

    function setUp() public {
        implementation = new LatamStable();
        bytes memory initData = abi.encodeWithSelector(
            LatamStable.initialize.selector, address(this), address(this), address(this),
            address(this), "Local wCOP", "wCOP");
        token = LatamStable(address(new LocalProxy(address(implementation), initData)));
    }

    function testSourceReadAndInitGuard() public {
        require(bytes(token.name()).length > 0, "name");
        require(bytes(token.symbol()).length > 0, "symbol");
        require(token.decimals() == 18, "decimals");
        require(token.getRoleAdmin(token.MINTER_ROLE()) == token.DEFAULT_ADMIN_ROLE(), "admin");
        require(token.supportsInterface(0x01ffc9a7), "interface");
        require(implementation.proxiableUUID() ==
            0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc, "uuid");
        (bool ok,) = address(token).call(abi.encodeWithSelector(
            LatamStable.initialize.selector, ALICE, ALICE, ALICE, ALICE, "other", "OTHER"));
        require(!ok, "reinit");
    }

    function testSourceMintTransferBurn() public {
        token.mint(ALICE, 17);
        require(token.totalSupply() == 17, "mint supply");
        vm.prank(ALICE);
        token.transfer(BOB, 5);
        vm.prank(BOB);
        token.approve(ALICE, 3);
        vm.prank(ALICE);
        token.transferFrom(BOB, ALICE, 3);
        vm.prank(ALICE);
        token.burn(15);
        vm.prank(BOB);
        token.approve(address(this), 2);
        token.burnFrom(BOB, 2);
        require(token.totalSupply() == 0, "burn supply");
        require(token.balanceOf(ALICE) == 0 && token.balanceOf(BOB) == 0, "balances");
    }

    function testSourcePauseAndRoles() public {
        token.mint(ALICE, 1);
        token.pause();
        vm.prank(ALICE);
        (bool ok,) = address(token).call(abi.encodeWithSelector(bytes4(keccak256("transfer(address,uint256)")), BOB, 1));
        require(!ok, "paused transfer");
        token.unpause();
        token.grantRole(token.PAUSER_ROLE(), ALICE);
        require(token.hasRole(token.PAUSER_ROLE(), ALICE), "grant");
        vm.prank(ALICE);
        token.pause();
        vm.prank(ALICE);
        token.unpause();
        token.revokeRole(token.PAUSER_ROLE(), ALICE);
        require(!token.hasRole(token.PAUSER_ROLE(), ALICE), "revoke");
        vm.prank(ALICE);
        token.transfer(BOB, 1);
        vm.prank(BOB);
        token.burn(1);
        require(token.totalSupply() == 0, "supply");
    }

    function testSourceUpgradeGuards() public {
        vm.prank(ALICE);
        (bool unauthorized,) = address(token).call(abi.encodeWithSignature(
            "upgradeToAndCall(address,bytes)", address(implementation), ""));
        require(!unauthorized, "unauthorized upgrade");
        (bool invalid,) = address(token).call(abi.encodeWithSignature(
            "upgradeToAndCall(address,bytes)", BOB, ""));
        require(!invalid, "invalid implementation");
    }

    function testSourcePermitAndReplay() public {
        uint256 key = 0xA11CE;
        address owner = vm.addr(key);
        uint256 deadline = block.timestamp + 3600;
        bytes32 digest = keccak256(abi.encodePacked(
            "\x19\x01", token.DOMAIN_SEPARATOR(),
            keccak256(abi.encode(
                keccak256("Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)"),
                owner, BOB, 7, token.nonces(owner), deadline))));
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(key, digest);
        token.permit(owner, BOB, 7, deadline, v, r, s);
        require(token.allowance(owner, BOB) == 7, "allowance");
        (bool replay,) = address(token).call(abi.encodeWithSelector(
            bytes4(keccak256("permit(address,address,uint256,uint256,uint8,bytes32,bytes32)")), owner, BOB, 7, deadline, v, r, s));
        require(!replay, "replay");
    }

    function testFuzzSourceSequence(uint96 rawMint, uint96 rawTransfer) public {
        uint256 amount = 1 + uint256(rawMint) % 1e12;
        uint256 moved = 1 + uint256(rawTransfer) % amount;
        token.mint(ALICE, amount);
        vm.prank(ALICE);
        token.transfer(BOB, moved);
        vm.prank(BOB);
        token.transfer(ALICE, moved);
        vm.prank(ALICE);
        token.burn(amount);
        require(token.totalSupply() == 0, "supply");
    }
}
