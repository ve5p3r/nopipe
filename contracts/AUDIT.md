# Nopipe Contracts — Red Team Audit
**Date:** 2026-03-04  
**Scope:** OperatorNFT.sol, SwapExecutor.sol, SubscriptionKeeper.sol  
**Tool:** Slither 0.11.5 + manual review  
**Status:** Pre-deploy, NOT production-ready until all Critical/High fixed

---

## CRITICAL

### C1 — Locked ETH in SubscriptionKeeper
**File:** SubscriptionKeeper.sol:92  
**Finding:** `receive() external payable {}` accepts ETH but the contract has zero withdrawal mechanism. Any ETH sent (accidentally or otherwise) is permanently locked.  
**Impact:** Permanent loss of funds.  
**Fix:** Remove `receive()` entirely — this contract has no reason to accept ETH.

### C2 — Cross-function reentrancy via missing `nonReentrant` on state-mutating functions  
**File:** SubscriptionKeeper.sol — `subscribe()`, `stopRenewal()`, `authorizeBudget()`  
**Finding:** `collectFor()` has `nonReentrant` but these three do not. During token transfers (e.g. excess STABLE refund, swap callback, input token pull), a malicious ERC20 could trigger a callback that calls one of these functions. Since `collectFor()` writes `sub.nextRenewal` and `budget.cyclesRemaining` *after* the external calls, state set by the callback can be overwritten by `collectFor()`'s final writes.  
**Realistic vector:** Agent's input token has a `transfer` hook → callback calls `stopRenewal(false)` to re-enable a stopped sub → `collectFor()` proceeds and renews as if nothing happened, extracting an extra cycle.  
**Fix:** Add `nonReentrant` to `subscribe()`, `stopRenewal()`, `authorizeBudget()`.

---

## HIGH

### H1 — Missing zero-address guard on constructor/setFeeRecipient
**Files:** SwapExecutor.sol, SubscriptionKeeper.sol  
**Finding:** Neither contract validates that `feeRecipient != address(0)` on construction or in `setFeeRecipient`. Fees silently burned to zero address.  
**Fix:** Add `require(feeRecipient != address(0), "Zero address")` in both constructors and setters.

---

## MEDIUM

### M1 — `owner` local variable shadows `Ownable.owner()` in OperatorNFT
**File:** OperatorNFT.sol — `tokensOfOwner()`, `burn()`, `_removeFromOwnerTokens()`, `_incrementTier()`, `_decrementTier()`  
**Finding:** Local variable named `owner` shadows the inherited `owner()` function. Benign now but a footgun in future modifications — a dev could accidentally use the local var thinking they have the contract owner.  
**Fix:** Rename locals to `tokenOwner` or `holder`.

### M2 — SwapExecutor: `tradeFor` reentrancy-balance (Slither medium, actual false positive)
**File:** SwapExecutor.sol:104  
**Finding:** Slither flags `balanceBefore` as a "stale" balance used after external call. This is intentional: the balance-diff pattern is the correct approach for fee-on-transfer tokens. `nonReentrant` is present. **Not a real bug.** Documented for auditor awareness.

---

## LOW / INFORMATIONAL

### L1 — OZ library pragma warnings
Only affects library interfaces, not production code. All our contracts target ^0.8.28 and compile with 0.8.28.

### L2 — `SubscriptionKeeper` uninitialized locals flagged by Slither  
`stableReceived` and `minOut` are initialized in all live code paths before use. Slither false positives due to scoped block assignment.

### L3 — MockRouter unchecked transfer returns  
Test-only contract. Not deployed.

---

## Tests to add before deploy
- [ ] Reentrancy attack via malicious ERC20 on SubscriptionKeeper.collectFor
- [ ] Verify ETH receive reverts after fix
- [ ] Zero feeRecipient construction attempt
- [ ] Verify highestTier is 0 after burn of last NFT (smoke test existing logic)
- [ ] stopRenewal cross-function reentrancy simulation
