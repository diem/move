//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "./FortyTwo.sol";
import "./Revert.sol";

contract ExternalCall_Sol {
    function call_forty_two(address addr) public pure returns (uint64) {
        FortyTwo_Sol fortytwo = FortyTwo_Sol(addr);
        return fortytwo.forty_two();
    }

    function call_revertWithMessage(address addr) public pure {
        Revert_Sol revert_contract = Revert_Sol(addr);
        revert_contract.revertWithMessage();
    }

    function try_call_forty_two(address addr) public pure returns (uint64) {
        FortyTwo_Sol fortytwo = FortyTwo_Sol(addr);
        try fortytwo.forty_two() returns (uint64 res) {
            return res;
        } catch Error(string memory reason) {
            revert(reason);
        } catch {
            revert("not implemented");
        }
    }

    function try_call_revertWithMessage(address addr) public pure {
        Revert_Sol revert_contract = Revert_Sol(addr);
        revert_contract.revertWithMessage();
        try revert_contract.revertWithMessage() {
            revert("not reverted");
        } catch Error(string memory reason) {
            revert(reason);
        } catch {
            revert("not implemented");
        }
    }
}
