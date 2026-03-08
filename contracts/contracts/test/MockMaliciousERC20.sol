// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

interface ISubscriptionKeeperHookTarget {
    function stopRenewal(bool state) external;
    function authorizeBudget(uint256 maxPerCycle, uint256 cyclesRemaining) external;
}

contract MockMaliciousERC20 is ERC20 {
    enum AttackMode {
        None,
        StopRenewal,
        AuthorizeBudget
    }

    uint8 private _dec;
    address public keeper;
    AttackMode public attackMode;
    bool public revertOnHookFailure;
    bool public hookArmed;

    constructor(string memory name, string memory symbol, uint8 dec) ERC20(name, symbol) {
        _dec = dec;
    }

    function decimals() public view override returns (uint8) {
        return _dec;
    }

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }

    function configureHook(address keeper_, AttackMode mode, bool revertOnFailure) external {
        keeper = keeper_;
        attackMode = mode;
        revertOnHookFailure = revertOnFailure;
    }

    function armHook(bool armed) external {
        hookArmed = armed;
    }

    function transferFrom(address from, address to, uint256 value) public override returns (bool) {
        _runHook();
        return super.transferFrom(from, to, value);
    }

    function transfer(address to, uint256 value) public override returns (bool) {
        _runHook();
        return super.transfer(to, value);
    }

    function _runHook() private {
        if (!hookArmed || keeper == address(0) || attackMode == AttackMode.None) {
            return;
        }

        bool ok;
        if (attackMode == AttackMode.StopRenewal) {
            (ok,) = keeper.call(
                abi.encodeWithSelector(ISubscriptionKeeperHookTarget.stopRenewal.selector, false)
            );
        } else {
            (ok,) = keeper.call(
                abi.encodeWithSelector(
                    ISubscriptionKeeperHookTarget.authorizeBudget.selector,
                    1,
                    type(uint256).max
                )
            );
        }

        if (revertOnHookFailure && !ok) {
            revert("hook failed");
        }
    }
}
