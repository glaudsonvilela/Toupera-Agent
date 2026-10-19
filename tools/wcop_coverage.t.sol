// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

import {LatamStable} from "src/LatamStable.sol";

interface VmCoverage {
    function prank(address sender) external;
    function load(address target, bytes32 slot) external view returns (bytes32);
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

    function testSourceInvalidValuePaths() public {
        token.mint(ALICE, 5);
        (bool zeroMint,) = address(token).call(abi.encodeWithSelector(token.mint.selector, address(0), 1));
        require(!zeroMint, "zero mint");
        vm.prank(ALICE);
        (bool zeroTransfer,) = address(token).call(abi.encodeWithSelector(token.transfer.selector, address(0), 1));
        require(!zeroTransfer, "zero receiver");
        vm.prank(ALICE);
        (bool excessTransfer,) = address(token).call(abi.encodeWithSelector(token.transfer.selector, BOB, 6));
        require(!excessTransfer, "excess transfer");
        vm.prank(ALICE);
        (bool zeroSpender,) = address(token).call(abi.encodeWithSelector(token.approve.selector, address(0), 1));
        require(!zeroSpender, "zero spender");
        vm.prank(ALICE);
        token.approve(BOB, 2);
        vm.prank(BOB);
        (bool excessAllowance,) = address(token).call(abi.encodeWithSelector(token.transferFrom.selector, ALICE, BOB, 3));
        require(!excessAllowance, "excess allowance");
        vm.prank(ALICE);
        (bool excessBurn,) = address(token).call(abi.encodeWithSelector(token.burn.selector, 6));
        require(!excessBurn, "excess burn");
        require(token.totalSupply() == 5 && token.balanceOf(ALICE) == 5, "state changed");
    }

    function testSourceInfiniteAllowanceAndSelfTransfer() public {
        token.mint(ALICE, 9);
        vm.prank(ALICE);
        token.approve(BOB, type(uint256).max);
        vm.prank(BOB);
        token.transferFrom(ALICE, BOB, 4);
        require(token.allowance(ALICE, BOB) == type(uint256).max, "infinite allowance");
        vm.prank(BOB);
        token.transfer(BOB, 1);
        require(token.balanceOf(BOB) == 4 && token.totalSupply() == 9, "self transfer");
        vm.prank(ALICE);
        token.approve(BOB, 0);
        require(token.allowance(ALICE, BOB) == 0, "reset");
    }

    function testSourceRoleAndPauseFailures() public {
        vm.prank(ALICE);
        (bool noMint,) = address(token).call(abi.encodeWithSelector(token.mint.selector, ALICE, 1));
        require(!noMint, "unauthorized mint");
        vm.prank(ALICE);
        (bool noPause,) = address(token).call(abi.encodeWithSelector(token.pause.selector));
        require(!noPause, "unauthorized pause");
        token.pause();
        (bool repeatPause,) = address(token).call(abi.encodeWithSelector(token.pause.selector));
        require(!repeatPause, "repeat pause");
        token.unpause();
        (bool repeatUnpause,) = address(token).call(abi.encodeWithSelector(token.unpause.selector));
        require(!repeatUnpause, "repeat unpause");
        token.grantRole(token.MINTER_ROLE(), ALICE);
        token.grantRole(token.MINTER_ROLE(), ALICE);
        vm.prank(BOB);
        (bool badConfirmation,) = address(token).call(abi.encodeWithSelector(token.renounceRole.selector, token.MINTER_ROLE(), ALICE));
        require(!badConfirmation, "bad confirmation");
        token.revokeRole(token.MINTER_ROLE(), ALICE);
        token.revokeRole(token.MINTER_ROLE(), ALICE);
        require(!token.hasRole(token.MINTER_ROLE(), ALICE), "role remains");
    }

    function testSourceValidUpgradeKeepsState() public {
        bytes32 slot = 0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc;
        token.mint(ALICE, 11);
        token.grantRole(token.MINTER_ROLE(), BOB);
        LatamStable nextImplementation = new LatamStable();
        token.upgradeToAndCall(address(nextImplementation), "");
        require(address(uint160(uint256(vm.load(address(token), slot)))) == address(nextImplementation), "implementation slot");
        require(token.balanceOf(ALICE) == 11 && token.totalSupply() == 11, "balance changed");
        require(token.hasRole(token.MINTER_ROLE(), BOB), "role changed");
        vm.prank(BOB);
        token.mint(ALICE, 1);
        require(token.balanceOf(ALICE) == 12, "post upgrade mint");
        token.upgradeToAndCall(address(implementation), abi.encodeWithSelector(token.pause.selector));
        require(token.paused() && token.totalSupply() == 12, "upgrade with call");
    }

    function testSourceProxyContext() public {
        bytes32 slot = 0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc;
        (bool proxiedUUID,) = address(token).call(abi.encodeWithSelector(token.proxiableUUID.selector));
        require(!proxiedUUID, "proxy UUID");
        (bool directUpgrade,) = address(implementation).call(abi.encodeWithSelector(token.upgradeToAndCall.selector, address(implementation), bytes("")));
        require(!directUpgrade, "direct upgrade");
        bytes32 beforeSlot = vm.load(address(token), slot);
        (bool invalid,) = address(token).call(abi.encodeWithSelector(token.upgradeToAndCall.selector, BOB, bytes("")));
        require(!invalid && vm.load(address(token), slot) == beforeSlot, "invalid upgrade changed state");
    }

    function testSourcePermitInvalidSignatureAndDeadline() public {
        uint256 key = 0xA11CE;
        address owner = vm.addr(key);
        uint256 nonce = token.nonces(owner);
        (bool badSig,) = address(token).call(abi.encodeWithSelector(token.permit.selector,
            owner, BOB, 5, block.timestamp + 1, uint8(27), bytes32(0), bytes32(0)));
        require(!badSig, "invalid signature");
        (bool expired,) = address(token).call(abi.encodeWithSelector(token.permit.selector,
            owner, BOB, 5, block.timestamp - 1, uint8(27), bytes32(0), bytes32(0)));
        require(!expired, "expired permit");
        require(token.nonces(owner) == nonce && token.allowance(owner, BOB) == 0, "permit changed state");
    }

    function testSourceUpgradeKeepsNamedState() public {
        uint256 key = 0xA11CE;
        address owner = vm.addr(key);
        uint256 deadline = block.timestamp + 3600;
        bytes32 beforeDomain = token.DOMAIN_SEPARATOR();
        bytes32 digest = keccak256(abi.encodePacked(
            "\x19\x01", beforeDomain,
            keccak256(abi.encode(
                keccak256("Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)"),
                owner, BOB, 7, token.nonces(owner), deadline))));
        (uint8 v, bytes32 r, bytes32 signatureS) = vm.sign(key, digest);
        token.permit(owner, BOB, 7, deadline, v, r, signatureS);
        token.mint(ALICE, 11);
        vm.prank(ALICE);
        token.approve(BOB, 4);
        token.grantRole(token.MINTER_ROLE(), BOB);
        token.pause();
        LatamStable nextImplementation = new LatamStable();
        token.upgradeToAndCall(address(nextImplementation), "");
        require(token.paused(), "pause changed");
        require(token.balanceOf(ALICE) == 11 && token.totalSupply() == 11, "balance changed");
        require(token.allowance(ALICE, BOB) == 4, "allowance changed");
        require(token.allowance(owner, BOB) == 7 && token.nonces(owner) == 1, "permit changed");
        require(token.DOMAIN_SEPARATOR() == beforeDomain, "domain changed");
        require(token.hasRole(token.MINTER_ROLE(), BOB), "role changed");
        token.unpause();
        vm.prank(ALICE);
        token.transfer(BOB, 1);
        require(token.balanceOf(BOB) == 1, "post upgrade transfer");
    }

    function testSourceRejectsWrongUUID() public {
        bytes32 slot = 0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc;
        bytes32 beforeSlot = vm.load(address(token), slot);
        WrongUUID wrong = new WrongUUID();
        (bool ok,) = address(token).call(abi.encodeWithSelector(token.upgradeToAndCall.selector, address(wrong), bytes("")));
        require(!ok && vm.load(address(token), slot) == beforeSlot, "wrong UUID accepted");
    }
}

contract WrongUUID {
    function proxiableUUID() external pure returns (bytes32) { return bytes32(0); }
}
