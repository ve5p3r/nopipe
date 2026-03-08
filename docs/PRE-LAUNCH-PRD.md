# Nopipe Pre-Launch PRD
**Status:** Gate 1 approved — problem defined  
**Target:** Launch tonight (2026-03-05 evening ET)  
**Blocker threshold:** C-items block launch. B-items block genesis close. A-items block Phase 2.

---

## Missing Items — ICO Vet Audit Response

### C1 — Failure Semantics + SLA (LAUNCH BLOCKER)
**Problem:** No documented retry logic, failure modes, or execution guarantees. Agents paying $0.10/call need to know what happens when a tx drops.  
**Deliverable:** `docs/EXECUTION-POLICY.md` — covers:
- Retry logic (how many attempts, backoff interval)
- Failure codes and what they mean for the agent
- What "non-custodial" means in terms of execution guarantees (we relay, we don't guarantee fills)
- Refund policy on failed submissions (none — agents own retry logic)

**Gate:** Written and linked from site before Gauntlet opens.

### C2 — RPC Redundancy (LAUNCH BLOCKER)
**Problem:** Single Alchemy dependency is a single point of failure. 200 active agents at production volume will hit Growth tier limits.  
**Deliverable:** Cluster config updated with dual RPC (Alchemy primary + Infura fallback, round-robin on 429). Env var: `BASE_RPC_HTTP_FALLBACK`.  
**Gate:** Both RPC endpoints configured and smoke-tested before deploy.

### C3 — Phase 2 Token Language Removal (LAUNCH BLOCKER)
**Problem:** Whitepaper and any public material mentioning Phase 2 token creates securities law exposure. Every mention builds an expectation record.  
**Deliverable:** Remove all Phase 2 token language from WHITEPAPER.md, site, and any public Discord posts. Internal-only: keep in INFRA-PITCH.md (not public).  
**Gate:** Zero token mentions in any publicly accessible document.

---

### B1 — Leaderboard Spec (GENESIS CLOSE BLOCKER)
**Problem:** Leaderboard mechanic is described but not specced. "Optionally attributed" needs to be built.  
**Deliverable:**
- Agent can optionally link wallet to a handle/name on submit
- Public leaderboard at `nopipe.io/leaderboard` — shows: handle (or anon), execution count, avg latency, win rate, optional Sharpe estimate
- 90-day genesis window displayed as countdown
- Raw P&L intentionally excluded (securities optics) — show execution metrics only
- Attribution toggle: agent can flip public/anon at any time during window

**Gate:** Spec written and wired before genesis window closes. MVP can be static or simple API.

### B2 — ZeroClaw P&L Disclosure (GENESIS CLOSE BLOCKER)
**Problem:** Claiming "real capital, P&L tracked" with zero data is a credibility gap. The reviewer flagged this. The data is the strongest card.  
**Deliverable:** Sanitized P&L snapshot from Ash/Ember/Flint/Cinder/Wisp for the genesis period. Not dollar amounts — risk-adjusted metrics: Sharpe, max drawdown, win rate, autonomous renewal count.  
**Gate:** Published at `nopipe.io/reference` or in WHITEPAPER appendix before cohort 2 opens.

### B3 — Execution Policy for Customization Tickets (GENESIS CLOSE BLOCKER)
**Problem:** x402 + ACP customization flow is described but pricing, SLA, and scope are undefined. Enterprise operators will ask "how long does a new chain integration take and what does it cost?"  
**Deliverable:** Ticket type catalog:
| Ticket Type | Cost (USDC) | SLA | Tier Required |
|-------------|-------------|-----|---------------|
| Chain add (new EVM) | 500 | 5 business days | Enterprise |
| Strategy param update | 50 | 24h | Pro+ |
| Execution policy change | 100 | 48h | Pro+ |
| Custom webhook | 200 | 72h | Enterprise |
| Emergency config | 250 | 4h | Enterprise |

**Gate:** Catalog published and x402 endpoint stub live before genesis closes.

---

### A1 — Third-Party Smart Contract Audit (PHASE 2 BLOCKER)
**Problem:** Internal Slither audit is necessary but not sufficient. SubscriptionKeeper.sol handling autonomous payments from agent wallets requires independent review.  
**Deliverable:** Engage audit firm (Spearbit, Trail of Bits, Code4rena contest, or equivalent) for OperatorNFT + SwapExecutor + SubscriptionKeeper.  
**Timeline:** Submit for audit Q1 end. Required before expanding beyond 25 genesis operators or any material on-chain value at risk.  
**Gate:** Audit report published before Phase 2.

### A2 — Alchemy Scale Plan (PHASE 2 BLOCKER)
**Problem:** At 200 agents × active trading, Alchemy Growth (~300M CU/mo) will be saturated. Need contractual upgrade path.  
**Deliverable:** RPC cost model at 25, 100, 200 agents. Upgrade trigger defined: at >80% CU utilization, auto-scale to Alchemy Scale ($449/mo) or add QuickNode node.  
**Gate:** Model written, upgrade path contractually available.

---

## Pricing Evaluation Mechanism

### Should mint cost adjust with timing?

**Yes. One gate check, not dynamic pricing.**

Dynamic pricing (Dutch auction, bonding curve) worked in 2021 NFT bull market where demand was reflexive. In a bear/neutral market it just signals low demand and caps fill price at minimum. For a B2B infrastructure product, agents are not speculating on price appreciation — they're evaluating ROI on access cost vs execution savings.

**Recommended mechanism:**

**T-48h check (runs 2026-03-04 tonight):**
- Is there verifiable inbound interest from ≥10 distinct agent wallets or operators? (Discord signups, wallet registrations, test pings to /gauntlet/apply)
- If YES → prices stay as set
- If NO → consider 20-30% reduction on Tier C/B only; Enterprise holds (high-touch service, price signals quality)

**T-0 check (at Gauntlet open):**
- If fewer than 15 of 25 seats fill in 72h → drop Tier C to $150, hold B and A
- If all 25 fill in <24h → lock prices for cohort 2 at same level or increase 25%

**What NOT to do:**
- No Dutch auction (wrong product category)
- No whitelist-then-public-sale (adds regulatory surface)
- No "price goes up as seats fill" bonding curve (creates flip incentive, not operator incentive)

**Hard rule:** Prices cannot increase during the genesis window. They can only decrease if fill rate is slow and only on Tier C. Enterprise pricing is a signal — if you discount it you're saying the high-touch service isn't worth it.

---

## Launch Checklist (tonight)

**Must have (blockers):**
- [ ] C1: Execution policy doc written + linked
- [ ] C2: Dual RPC configured in cluster
- [ ] C3: Token language removed from public docs
- [ ] Contracts deployed to Base Sepolia (testnet first, with "Pineapple" from Jack)
- [ ] Gauntlet endpoint live at api.nopipe.io (or subdomain)
- [ ] Site updated: countdown timer, correct seat counts (7/10/8), prices visible
- [ ] feeRecipient address set (Jack to provide)
- [ ] Mint costs wired into gauntlet.rs payment verification
- [ ] USDC Transfer log verification implemented in gauntlet.rs

**Nice to have:**
- [ ] Leaderboard stub (even static placeholder counts)
- [ ] agent.json accessible at nopipe.io/agent.json ✅ (done)
- [ ] Whitepaper linked from site footer

