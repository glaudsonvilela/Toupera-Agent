// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

interface Vm {
    function envAddress(string calldata name) external returns (address);
    function prank(address sender) external;
    function load(address target, bytes32 slot) external returns (bytes32);
    function addr(uint256 privateKey) external returns (address);
    function sign(uint256 privateKey, bytes32 digest) external returns (uint8, bytes32, bytes32);
}

interface IWCOP {
    function totalSupply() external view returns (uint256);
    function balanceOf(address account) external view returns (uint256);
    function allowance(address owner, address spender) external view returns (uint256);
    function approve(address spender, uint256 value) external returns (bool);
    function transfer(address recipient, uint256 value) external returns (bool);
    function transferFrom(address sender, address recipient, uint256 value) external returns (bool);
    function mint(address recipient, uint256 value) external;
    function burn(uint256 value) external;
    function burnFrom(address owner, uint256 value) external;
    function pause() external;
    function unpause() external;
    function paused() external view returns (bool);
    function DEFAULT_ADMIN_ROLE() external view returns (bytes32);
    function MINTER_ROLE() external view returns (bytes32);
    function PAUSER_ROLE() external view returns (bytes32);
    function UPGRADER_ROLE() external view returns (bytes32);
    function hasRole(bytes32 role, address account) external view returns (bool);
    function getRoleAdmin(bytes32 role) external view returns (bytes32);
    function grantRole(bytes32 role, address account) external;
    function revokeRole(bytes32 role, address account) external;
    function renounceRole(bytes32 role, address account) external;
    function name() external view returns (string memory);
    function symbol() external view returns (string memory);
    function decimals() external view returns (uint8);
    function supportsInterface(bytes4 interfaceId) external view returns (bool);
    function DOMAIN_SEPARATOR() external view returns (bytes32);
    function nonces(address owner) external view returns (uint256);
    function permit(address owner, address spender, uint256 value, uint256 deadline,
                    uint8 v, bytes32 r, bytes32 s) external;
    function proxiableUUID() external view returns (bytes32);
}

contract WCOPReadOnlyFork {
    address internal constant TOKEN = 0x8a1D45e102e886510e891d2Ec656a708991e2D76;
    bytes32 internal constant SLOT = 0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc;
    Vm internal constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));
    IWCOP internal constant WCOP = IWCOP(TOKEN);

    function actor(string memory name) internal returns (address) {
        return vm.envAddress(name);
    }

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
        address holder = actor("WCOP_HOLDER");
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
        address holder = actor("WCOP_HOLDER");
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

    function testAuthorizedMintBurnRoundTrip() public {
        address minter = actor("WCOP_MINTER");
        uint256 supply = WCOP.totalSupply();
        uint256 balance = WCOP.balanceOf(address(this));
        vm.prank(minter);
        WCOP.mint(address(this), 1);
        require(WCOP.totalSupply() == supply + 1, "mint supply");
        require(WCOP.balanceOf(address(this)) == balance + 1, "mint balance");
        WCOP.burn(1);
        require(WCOP.totalSupply() == supply, "burn supply");
        require(WCOP.balanceOf(address(this)) == balance, "burn balance");
    }

    function testBurnFromWithAllowance() public {
        address minter = actor("WCOP_MINTER");
        address holder = actor("WCOP_HOLDER");
        uint256 supply = WCOP.totalSupply();
        uint256 balance = WCOP.balanceOf(holder);
        vm.prank(minter);
        WCOP.mint(holder, 1);
        vm.prank(holder);
        require(WCOP.approve(address(this), 1), "approve burn");
        WCOP.burnFrom(holder, 1);
        require(WCOP.allowance(holder, address(this)) == 0, "burn allowance");
        require(WCOP.totalSupply() == supply, "burnFrom supply");
        require(WCOP.balanceOf(holder) == balance, "burnFrom balance");
    }

    function testAuthorizedPauseUnpause() public {
        address pauser = actor("WCOP_PAUSER");
        address minter = actor("WCOP_MINTER");
        address holder = actor("WCOP_HOLDER");
        uint256 supply = WCOP.totalSupply();
        if (WCOP.paused()) {
            vm.prank(pauser);
            WCOP.unpause();
        }
        vm.prank(minter);
        WCOP.mint(address(this), 1);
        vm.prank(pauser);
        WCOP.pause();
        require(WCOP.paused(), "not paused");
        (bool ok,) = TOKEN.call(abi.encodeWithSignature("transfer(address,uint256)", holder, 1));
        require(!ok, "transfer while paused");
        vm.prank(pauser);
        WCOP.unpause();
        require(!WCOP.paused(), "still paused");
        require(WCOP.transfer(holder, 1), "transfer after unpause");
        vm.prank(holder);
        WCOP.burn(1);
        require(WCOP.totalSupply() == supply, "supply not restored");
    }

    function testRoleGrantRevokeRenounce() public {
        address admin = actor("WCOP_ADMIN");
        bytes32 role = WCOP.PAUSER_ROLE();
        require(WCOP.getRoleAdmin(role) == WCOP.DEFAULT_ADMIN_ROLE(), "admin role changed");
        require(!WCOP.hasRole(role, address(this)), "already has role");
        vm.prank(admin);
        WCOP.grantRole(role, address(this));
        require(WCOP.hasRole(role, address(this)), "grant absent");
        vm.prank(admin);
        WCOP.revokeRole(role, address(this));
        require(!WCOP.hasRole(role, address(this)), "revoke absent");
        vm.prank(admin);
        WCOP.grantRole(role, address(this));
        WCOP.renounceRole(role, address(this));
        require(!WCOP.hasRole(role, address(this)), "renounce absent");
    }

    function testReinitializeRejected() public {
        uint256 supply = WCOP.totalSupply();
        (bool ok,) = TOKEN.call(abi.encodeWithSignature(
            "initialize(address,address,address,address,string,string)",
            address(this), address(this), address(this), address(this), "other", "OTHER"));
        require(!ok, "reinitialize succeeded");
        require(WCOP.totalSupply() == supply, "supply changed");
    }

    function testUnauthorizedUpgradeRejected() public {
        bytes32 beforeSlot = vm.load(TOKEN, SLOT);
        (bool ok,) = TOKEN.call(abi.encodeWithSignature("upgradeToAndCall(address,bytes)", address(0xCAFE), ""));
        require(!ok, "upgrade succeeded");
        require(vm.load(TOKEN, SLOT) == beforeSlot, "implementation changed");
    }

    function testAuthorizedInvalidUpgradeRejected() public {
        address upgrader = actor("WCOP_UPGRADER");
        bytes32 beforeSlot = vm.load(TOKEN, SLOT);
        vm.prank(upgrader);
        (bool ok,) = TOKEN.call(abi.encodeWithSignature("upgradeToAndCall(address,bytes)", address(0xCAFE), ""));
        require(!ok, "invalid implementation accepted");
        require(vm.load(TOKEN, SLOT) == beforeSlot, "implementation changed");
    }

    function testImplementationUUID() public {
        address implementation = actor("WCOP_IMPL");
        require(IWCOP(implementation).proxiableUUID() == SLOT, "implementation UUID");
        (bool ok,) = TOKEN.staticcall(abi.encodeWithSignature("proxiableUUID()"));
        require(!ok, "proxy UUID unexpectedly callable");
    }

    function testReadSurface() public {
        require(bytes(WCOP.name()).length > 0, "empty name");
        require(bytes(WCOP.symbol()).length > 0, "empty symbol");
        require(WCOP.decimals() == 18, "decimals changed");
        require(WCOP.MINTER_ROLE() == keccak256("MINTER_ROLE"), "minter hash");
        require(WCOP.PAUSER_ROLE() == keccak256("PAUSER_ROLE"), "pauser hash");
        require(WCOP.UPGRADER_ROLE() == keccak256("UPGRADER_ROLE"), "upgrader hash");
        require(WCOP.supportsInterface(0x01ffc9a7), "ERC165");
        (bool domainOk,) = TOKEN.staticcall(abi.encodeWithSignature("eip712Domain()"));
        require(domainOk, "EIP712 domain read");
        (bool versionOk,) = TOKEN.staticcall(abi.encodeWithSignature("UPGRADE_INTERFACE_VERSION()"));
        require(versionOk, "upgrade version read");
    }

    function testPermitSignature() public {
        uint256 key = 0xA11CE;
        address owner = vm.addr(key);
        address spender = address(0xCAFE);
        uint256 nonce = WCOP.nonces(owner);
        uint256 deadline = block.timestamp + 3600;
        bytes32 typeHash = keccak256("Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)");
        bytes32 structHash = keccak256(abi.encode(typeHash, owner, spender, 17, nonce, deadline));
        bytes32 digest = keccak256(abi.encodePacked("\x19\x01", WCOP.DOMAIN_SEPARATOR(), structHash));
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(key, digest);
        WCOP.permit(owner, spender, 17, deadline, v, r, s);
        require(WCOP.nonces(owner) == nonce + 1, "nonce");
        require(WCOP.allowance(owner, spender) == 17, "permit allowance");
        (bool replayOk,) = TOKEN.call(abi.encodeWithSignature(
            "permit(address,address,uint256,uint256,uint8,bytes32,bytes32)",
            owner, spender, 17, deadline, v, r, s));
        require(!replayOk, "replay accepted");
    }

    function testFuzzTransferConservesSupply(uint96 seed) public {
        address holder = actor("WCOP_HOLDER");
        uint256 initial = WCOP.balanceOf(holder);
        require(initial > 0, "no balance");
        uint256 ceiling = initial < 1e18 ? initial : 1e18;
        uint256 amount = 1 + uint256(seed) % ceiling;
        uint256 supply = WCOP.totalSupply();
        vm.prank(holder);
        require(WCOP.transfer(address(this), amount), "transfer");
        require(WCOP.balanceOf(holder) == initial - amount, "holder delta");
        require(WCOP.balanceOf(address(this)) == amount, "receiver delta");
        require(WCOP.totalSupply() == supply, "supply changed");
    }

    function testFuzzApprovalOverwrite(uint96 first, uint96 second) public {
        address spender = address(0xCAFE);
        uint256 supply = WCOP.totalSupply();
        require(WCOP.approve(spender, first), "first");
        require(WCOP.approve(spender, second), "second");
        require(WCOP.allowance(address(this), spender) == second, "overwrite");
        require(WCOP.approve(spender, 0), "reset");
        require(WCOP.allowance(address(this), spender) == 0, "not reset");
        require(WCOP.totalSupply() == supply, "supply changed");
    }

    function testFuzzLocalSequence(uint96 seedAmount, uint96 seedMove) public {
        address minter = actor("WCOP_MINTER");
        address holder = actor("WCOP_HOLDER");
        uint256 amount = 1 + uint256(seedAmount) % 1e12;
        uint256 moved = 1 + uint256(seedMove) % amount;
        uint256 supply = WCOP.totalSupply();
        uint256 holderBefore = WCOP.balanceOf(holder);
        vm.prank(minter);
        WCOP.mint(address(this), amount);
        require(WCOP.totalSupply() == supply + amount, "mint delta");
        require(WCOP.transfer(holder, moved), "move");
        vm.prank(holder);
        require(WCOP.transfer(address(this), moved), "move back");
        WCOP.burn(amount);
        require(WCOP.totalSupply() == supply, "supply restoration");
        require(WCOP.balanceOf(holder) == holderBefore, "holder restoration");
    }
}
