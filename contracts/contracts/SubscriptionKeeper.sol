// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/IERC20Metadata.sol";

interface IRouter {
    function getAmountsOut(uint256 amountIn, address[] calldata path)
        external
        view
        returns (uint256[] memory amounts);

    function swapExactTokensForTokensSupportingFeeOnTransferTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external;
}

/**
 * @title SubscriptionKeeper
 * @notice Permissionless renewal collector for Polyclaw subscriptions.
 *
 * Requirements implemented:
 * - collectFor(address) returns bool and never intentionally reverts on operational failures
 * - Emits SubRenewalFailed(agent, reason) and returns false on failure
 * - BudgetAuth with cyclesRemaining (type(uint256).max = unlimited)
 * - getSubscriptionStatus(agent) view helper
 * - Emits SubRenewed(agent, nextDue, amountCharged)
 */
contract SubscriptionKeeper is Ownable {
    // Base WETH
    address public constant WPEG = 0x4200000000000000000000000000000000000006;

    uint256 public constant INTERVAL = 30 days;
    uint256 private constant BPS = 10_000;
    uint256 private constant PROTOCOL_FEE_BPS = 500; // 5%

    address public immutable STABLE;
    address public feeRecipient;

    IERC20 private immutable ISTABLE;
    uint256 private immutable ONE_STABLE;

    // Pool plan config (price is in whole STABLE units, scaled by ONE_STABLE during charge)
    mapping(address => uint256) public PLAN;
    mapping(address => uint256) public PLAN_LEFT;
    mapping(address => uint256) public PLAN_LIMIT;

    struct AgentSub {
        address agent;
        address pool;       // zero = direct subscription
        address router;     // payment token router
        address[] path;     // path[0]=payment token, path[-1]=STABLE
        uint256 sellFeeBps; // slippage buffer in bps
        uint256 nextRenewal;
        bool stop;
    }

    struct BudgetAuth {
        uint256 maxPerCycle;     // spend ceiling per collect cycle
        uint256 cyclesRemaining; // decremented on success; max uint = unlimited
    }

    mapping(address => AgentSub) private _subs;
    mapping(address => BudgetAuth) private _budgets;

    // Min-heap by nextRenewal (earliest first)
    address[] private _heap;
    mapping(address => uint256) private _heapIndex; // 1-indexed

    event Subscribed(address indexed agent, address indexed pool, uint256 pricePerCycle);
    event SubRenewed(address indexed agent, uint256 nextDue, uint256 amountCharged);
    event SubRenewalFailed(address indexed agent, string reason);
    event BudgetSet(address indexed agent, uint256 maxPerCycle, uint256 cyclesRemaining);
    event FeeRecipientUpdated(address indexed newRecipient);

    constructor(address stable, address _feeRecipient) Ownable(msg.sender) {
        require(stable != address(0) && _feeRecipient != address(0), "Zero address");

        STABLE = stable;
        feeRecipient = _feeRecipient;
        ISTABLE = IERC20(stable);
        ONE_STABLE = 10 ** IERC20Metadata(stable).decimals();
    }

    receive() external payable {}

    // ─────────────────────────────────────────────────────────────────────────
    // Budget authorization
    // ─────────────────────────────────────────────────────────────────────────

    /**
     * @notice Agent authorizes per-cycle spend ceiling and number of cycles.
     * @dev cyclesRemaining = type(uint256).max means unlimited cycles.
     */
    function authorizeBudget(uint256 maxPerCycle, uint256 cyclesRemaining) external {
        require(maxPerCycle > 0, "Zero budget");

        _budgets[msg.sender] = BudgetAuth({
            maxPerCycle: maxPerCycle,
            cyclesRemaining: cyclesRemaining
        });

        emit BudgetSet(msg.sender, maxPerCycle, cyclesRemaining);
    }

    function getBudget(address agent) external view returns (uint256 maxPerCycle, uint256 cyclesRemaining) {
        BudgetAuth memory b = _budgets[agent];
        return (b.maxPerCycle, b.cyclesRemaining);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Owner configuration
    // ─────────────────────────────────────────────────────────────────────────

    function setPool(address pool, uint256 price, uint256 limit) external onlyOwner {
        PLAN[pool] = price;
        PLAN_LEFT[pool] = limit;
        PLAN_LIMIT[pool] = limit;
    }

    function setFeeRecipient(address _feeRecipient) external onlyOwner {
        require(_feeRecipient != address(0), "Zero address");
        feeRecipient = _feeRecipient;
        emit FeeRecipientUpdated(_feeRecipient);
    }

    /**
     * @notice Owner configures (or reconfigures) an agent subscription.
     */
    function subscribe(
        address agent,
        address pool,
        address router,
        address[] calldata path,
        uint256 sellFeeBps
    ) external onlyOwner {
        require(agent != address(0), "Zero agent");
        require(router != address(0), "Zero router");
        require(path.length >= 2 && path[path.length - 1] == STABLE, "Bad path");
        require(sellFeeBps < BPS, "Bad sell fee");
        require(pool == address(0) || PLAN[pool] != 0, "Pool inactive");

        AgentSub storage existing = _subs[agent];
        if (existing.agent != address(0) && existing.pool != address(0) && PLAN[existing.pool] != 0) {
            PLAN_LEFT[existing.pool]++;
        }

        if (pool != address(0)) {
            require(PLAN_LEFT[pool] > 0, "Pool full");
            PLAN_LEFT[pool]--;
        }

        AgentSub storage sub = _subs[agent];
        sub.agent = agent;
        sub.pool = pool;
        sub.router = router;
        sub.path = path;
        sub.sellFeeBps = sellFeeBps;
        sub.nextRenewal = block.timestamp + INTERVAL;
        sub.stop = false;

        if (_heapIndex[agent] == 0) {
            _heapPush(agent);
        } else {
            _heapUpdate(agent);
        }

        emit Subscribed(agent, pool, PLAN[pool]);
    }

    function stopRenewal(bool state) external {
        require(_subs[msg.sender].agent != address(0), "No sub");
        _subs[msg.sender].stop = state;
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Keeper collection
    // ─────────────────────────────────────────────────────────────────────────

    /**
     * @notice Permissionless renewal attempt.
     * @return success true if renewal completed, false otherwise.
     */
    function collectFor(address agent) external returns (bool success) {
        AgentSub storage sub = _subs[agent];

        if (sub.agent == address(0)) return _fail(agent, "No sub");
        if (sub.stop) return _fail(agent, "Renewal stopped");
        if (block.timestamp < sub.nextRenewal) return _fail(agent, "Not due");
        if (sub.path.length < 2 || sub.path[sub.path.length - 1] != STABLE) return _fail(agent, "Bad path");
        if (sub.sellFeeBps >= BPS) return _fail(agent, "Bad sell fee");

        uint256 planPrice = PLAN[sub.pool];
        if (planPrice == 0) return _fail(agent, "Pool inactive");

        if (planPrice > type(uint256).max / ONE_STABLE) return _fail(agent, "Amount overflow");
        uint256 amountRequired = planPrice * ONE_STABLE;

        BudgetAuth storage budget = _budgets[agent];
        if (budget.maxPerCycle == 0) return _fail(agent, "Budget not authorized");
        if (budget.cyclesRemaining == 0) {
            sub.stop = true;
            return _fail(agent, "Budget exhausted");
        }
        if (amountRequired > budget.maxPerCycle) return _fail(agent, "Exceeds budget");

        uint256 amountIn = amountRequired;
        if (sub.path[0] != STABLE) {
            (uint256 quotedIn, bool quotedOk) = _quoteInputForStable(sub, amountRequired);
            if (!quotedOk || quotedIn == 0) return _fail(agent, "Quote failed");
            amountIn = quotedIn;
        }

        (uint256 payerBalance, bool balOk) = _safeBalanceOf(sub.path[0], agent);
        if (!balOk || payerBalance < amountIn) return _fail(agent, "Insufficient balance");

        (uint256 allowance, bool allowanceOk) = _safeAllowance(sub.path[0], agent, address(this));
        if (!allowanceOk || allowance < amountIn) return _fail(agent, "Insufficient allowance");

        (uint256 pulled, bool pulledOk) = _safeTransferFrom(sub.path[0], agent, address(this), amountIn);
        if (!pulledOk || pulled == 0) return _fail(agent, "Payment pull failed");

        uint256 stableReceived;

        if (sub.path[0] == STABLE) {
            stableReceived = pulled;
        } else {
            if (!_forceApprove(sub.path[0], sub.router, pulled)) {
                _safeTransfer(sub.path[0], agent, pulled);
                return _fail(agent, "Approve failed");
            }

            uint256 minOut;
            {
                try IRouter(sub.router).getAmountsOut(pulled, sub.path) returns (uint256[] memory amountsOut) {
                    if (amountsOut.length == 0) {
                        _safeTransfer(sub.path[0], agent, pulled);
                        return _fail(agent, "Swap quote failed");
                    }
                    uint256 expectedOut = amountsOut[amountsOut.length - 1];
                    minOut = (expectedOut * (BPS - sub.sellFeeBps)) / BPS;
                } catch {
                    _safeTransfer(sub.path[0], agent, pulled);
                    return _fail(agent, "Swap quote failed");
                }
            }

            (uint256 stableBefore, bool stableBeforeOk) = _safeBalanceOf(STABLE, address(this));
            if (!stableBeforeOk) {
                _safeTransfer(sub.path[0], agent, pulled);
                return _fail(agent, "Stable balance read failed");
            }

            try IRouter(sub.router).swapExactTokensForTokensSupportingFeeOnTransferTokens(
                pulled,
                minOut,
                sub.path,
                address(this),
                block.timestamp
            ) {
                (uint256 stableAfter, bool stableAfterOk) = _safeBalanceOf(STABLE, address(this));
                if (!stableAfterOk || stableAfter < stableBefore) {
                    return _fail(agent, "Swap accounting failed");
                }
                stableReceived = stableAfter - stableBefore;
            } catch {
                _safeTransfer(sub.path[0], agent, pulled);
                return _fail(agent, "Swap failed");
            }
        }

        if (stableReceived < amountRequired) {
            if (stableReceived > 0) _safeTransfer(STABLE, agent, stableReceived);
            return _fail(agent, "Underpaid");
        }

        // Refund any excess stable to the agent.
        uint256 excess = stableReceived - amountRequired;
        if (excess > 0) {
            _safeTransfer(STABLE, agent, excess);
        }

        // Distribute charged amount.
        if (sub.pool != address(0)) {
            uint256 poolShare = (amountRequired * (BPS - PROTOCOL_FEE_BPS)) / BPS;
            uint256 protocolShare = amountRequired - poolShare;

            if (!_safeTransfer(STABLE, sub.pool, poolShare)) {
                return _fail(agent, "Pool payout failed");
            }
            if (!_safeTransfer(STABLE, feeRecipient, protocolShare)) {
                return _fail(agent, "Fee payout failed");
            }
        } else {
            if (!_safeTransfer(STABLE, feeRecipient, amountRequired)) {
                return _fail(agent, "Fee payout failed");
            }
        }

        // Renewal success.
        sub.nextRenewal = block.timestamp + INTERVAL;
        _heapUpdate(agent);

        if (budget.cyclesRemaining != type(uint256).max) {
            budget.cyclesRemaining -= 1;
            if (budget.cyclesRemaining == 0) {
                sub.stop = true;
            }
        }

        emit SubRenewed(agent, sub.nextRenewal, amountRequired);
        return true;
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Views
    // ─────────────────────────────────────────────────────────────────────────

    function getSubscriptionStatus(address agent)
        external
        view
        returns (bool active, uint256 nextDueAt, uint256 budgetRemaining, uint256 cyclesLeft)
    {
        AgentSub storage sub = _subs[agent];
        BudgetAuth memory budget = _budgets[agent];

        nextDueAt = sub.nextRenewal;
        budgetRemaining = budget.maxPerCycle;
        cyclesLeft = budget.cyclesRemaining;

        if (sub.agent == address(0) || sub.stop) {
            return (false, nextDueAt, budgetRemaining, cyclesLeft);
        }

        uint256 planPrice = PLAN[sub.pool];
        if (planPrice == 0 || planPrice > type(uint256).max / ONE_STABLE) {
            return (false, nextDueAt, budgetRemaining, cyclesLeft);
        }

        uint256 amountRequired = planPrice * ONE_STABLE;

        bool cyclesOk = budget.cyclesRemaining == type(uint256).max || budget.cyclesRemaining > 0;
        active = budget.maxPerCycle >= amountRequired && cyclesOk;
    }

    function getSub(address agent) external view returns (AgentSub memory) {
        return _subs[agent];
    }

    function heapSize() external view returns (uint256) {
        return _heap.length;
    }

    function nextDue() external view returns (address agent, uint256 renewalTime) {
        if (_heap.length == 0) return (address(0), 0);
        agent = _heap[0];
        renewalTime = _subs[agent].nextRenewal;
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Heap helpers
    // ─────────────────────────────────────────────────────────────────────────

    function _heapPush(address agent) private {
        _heap.push(agent);
        _heapIndex[agent] = _heap.length; // 1-indexed
        _bubbleUp(_heap.length - 1);
    }

    function _heapUpdate(address agent) private {
        uint256 idx = _heapIndex[agent];
        if (idx == 0) return;
        _bubbleUp(idx - 1);
        _bubbleDown(idx - 1);
    }

    function _bubbleUp(uint256 idx) private {
        while (idx > 0) {
            uint256 parent = (idx - 1) / 2;
            if (_subs[_heap[parent]].nextRenewal <= _subs[_heap[idx]].nextRenewal) break;
            _heapSwap(parent, idx);
            idx = parent;
        }
    }

    function _bubbleDown(uint256 idx) private {
        uint256 n = _heap.length;
        while (true) {
            uint256 smallest = idx;
            uint256 l = 2 * idx + 1;
            uint256 r = 2 * idx + 2;

            if (l < n && _subs[_heap[l]].nextRenewal < _subs[_heap[smallest]].nextRenewal) {
                smallest = l;
            }
            if (r < n && _subs[_heap[r]].nextRenewal < _subs[_heap[smallest]].nextRenewal) {
                smallest = r;
            }
            if (smallest == idx) break;

            _heapSwap(idx, smallest);
            idx = smallest;
        }
    }

    function _heapSwap(uint256 a, uint256 b) private {
        address tmp = _heap[a];
        _heap[a] = _heap[b];
        _heap[b] = tmp;
        _heapIndex[_heap[a]] = a + 1;
        _heapIndex[_heap[b]] = b + 1;
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Internal helpers
    // ─────────────────────────────────────────────────────────────────────────

    function _fail(address agent, string memory reason) private returns (bool) {
        emit SubRenewalFailed(agent, reason);
        return false;
    }

    function _quoteInputForStable(AgentSub storage sub, uint256 stableAmount)
        private
        view
        returns (uint256 amountIn, bool ok)
    {
        address[] memory rev = _reverse(sub.path);

        try IRouter(sub.router).getAmountsOut(ONE_STABLE, rev) returns (uint256[] memory amounts) {
            if (amounts.length == 0) return (0, false);
            uint256 oneStableInPayment = amounts[amounts.length - 1];
            if (oneStableInPayment == 0) return (0, false);
            if (sub.sellFeeBps >= BPS) return (0, false);

            // Scale quote to desired stableAmount.
            if (stableAmount > type(uint256).max / oneStableInPayment) return (0, false);
            uint256 baseAmount = (stableAmount * oneStableInPayment + ONE_STABLE - 1) / ONE_STABLE;

            // Gross up for sellFeeBps.
            uint256 denominator = BPS - sub.sellFeeBps;
            amountIn = (baseAmount * BPS + denominator - 1) / denominator;
            return (amountIn, true);
        } catch {
            return (0, false);
        }
    }

    function _safeBalanceOf(address token, address account) private view returns (uint256 bal, bool ok) {
        try IERC20(token).balanceOf(account) returns (uint256 value) {
            return (value, true);
        } catch {
            return (0, false);
        }
    }

    function _safeAllowance(address token, address holder, address spender)
        private
        view
        returns (uint256 allowance_, bool ok)
    {
        try IERC20(token).allowance(holder, spender) returns (uint256 value) {
            return (value, true);
        } catch {
            return (0, false);
        }
    }

    function _safeTransfer(address token, address to, uint256 amount) private returns (bool) {
        if (amount == 0) return true;

        (bool success, bytes memory data) = token.call(
            abi.encodeWithSelector(IERC20.transfer.selector, to, amount)
        );

        return success && (data.length == 0 || abi.decode(data, (bool)));
    }

    function _safeTransferFrom(address token, address from, address to, uint256 amount)
        private
        returns (uint256 received, bool ok)
    {
        (uint256 beforeBal, bool beforeOk) = _safeBalanceOf(token, to);
        if (!beforeOk) return (0, false);

        (bool success, bytes memory data) = token.call(
            abi.encodeWithSelector(IERC20.transferFrom.selector, from, to, amount)
        );

        if (!success || (data.length != 0 && !abi.decode(data, (bool)))) {
            return (0, false);
        }

        (uint256 afterBal, bool afterOk) = _safeBalanceOf(token, to);
        if (!afterOk || afterBal < beforeBal) return (0, false);

        return (afterBal - beforeBal, true);
    }

    function _forceApprove(address token, address spender, uint256 amount) private returns (bool) {
        (bool success0, bytes memory data0) = token.call(
            abi.encodeWithSelector(IERC20.approve.selector, spender, 0)
        );
        bool ok0 = success0 && (data0.length == 0 || abi.decode(data0, (bool)));

        (bool success, bytes memory data) = token.call(
            abi.encodeWithSelector(IERC20.approve.selector, spender, amount)
        );
        bool ok = success && (data.length == 0 || abi.decode(data, (bool)));

        return ok0 && ok;
    }

    function _reverse(address[] memory path) private pure returns (address[] memory rev) {
        rev = new address[](path.length);
        for (uint256 i = path.length; i != 0; i--) {
            rev[path.length - i] = path[i - 1];
        }
    }
}
