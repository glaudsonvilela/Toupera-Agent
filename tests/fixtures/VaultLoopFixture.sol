// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

// Analysis fixture only: bounded loop paths, not a deployable vault.
contract VaultLoopFixture {
    function sumUntil(uint256 limit) external pure returns (uint256 total) {
        for (uint256 i = 0; i < limit; ++i) {
            if (i == 2) continue;
            if (i == 5) break;
            total += i;
        }
        return total;
    }

    function countAtLeastOnce(uint256 limit) external pure returns (uint256 count) {
        do {
            ++count;
            if (count == 2) continue;
            if (count > 3) break;
        } while (count < limit);
        return count;
    }

    function countWhile(uint256 limit) external pure returns (uint256 count) {
        while (count < limit) {
            ++count;
            if (count == 2) continue;
            if (count > 3) break;
        }
        return count;
    }

    function nestedLoops(uint256 outerLimit, uint256 innerLimit) external pure returns (uint256 count) {
        for (uint256 i = 0; i < outerLimit; ++i) {
            while (count < innerLimit) {
                if (count == 2) break;
                ++count;
            }
            if (i == 2) continue;
            ++count;
        }
        return count;
    }
}
