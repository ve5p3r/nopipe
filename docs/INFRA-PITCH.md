# Polyclaw Execution Cluster — Infrastructure Pitch
> Internal doc. Not for external distribution without legal review.
> Last updated: 2026-02-28

---

## What This Is

A non-custodial execution layer for autonomous agents on Base.

Agents plug in one endpoint. They get gasless swaps, private transaction delivery, and automatic subscription renewal — without ever holding ETH or babysitting infrastructure.

---

## The Problem (In Failure Modes)

Onchain agents break in production because:

1. **Gas management is operationally brittle** — agents need ETH, refills, custody workflows. One empty wallet kills a running strategy.
2. **Public mempool execution is noisy** — reverts, latency spikes, sandwich attacks, unpredictable inclusion. Agents eating MEV on every trade.
3. **No primitive for recurring access** — no subscription, no renewal, no tier. Every agent is either free or manually managed.

Nobody has shipped all three fixes as a single endpoint. We have.

---

## The Product

### What Agents Call

```
POST /rpc
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "method": "swap_execute",
  "params": [{ "wallet": "0x...", "token_in": "...", "token_out": "...", "amount_in": "...", "sig": "0x..." }]
}
```

That's it. One call. Swap executes. Fee extracted. Tokens arrive.

### How It Works

- **`tradeFor(recipient)`** — cluster relayer submits the tx and pays gas. Agent never holds ETH.
- **`swap.sol` 0.1% fee** — extracted at contract level on every swap. Funds the relayer gas wallet. Self-sustaining.
- **EIP-191 auth + NFT tier gating** — signature proves wallet ownership, NFT confirms access tier (<25ms via LRU cache).
- **`subscribe.sol` keeper** — cluster triggers autonomous subscription renewal. No human in the loop.
- **Self-hosted op-geth** — private tx delivery, txpool visibility, unlimited event subscriptions, archive access. Not available from Infura/Alchemy.

---

## Why We Win (Measured, Not Vibed)

| Claim | How We Prove It |
|-------|-----------------|
| Lower revert rate | Publish weekly: our revert % vs public mempool baseline |
| Better net execution | Price improvement data vs 0x/1inch same-block quotes |
| True gasless | On-chain logs: 0 ETH in agent wallets, swaps still execute |
| No keeper intervention | 30-day autonomous run logs from ZeroClaw reference agents |

Moat is measured performance and operator quality data. Not "integration complexity."

---

## Business Model

| Revenue Stream | Mechanism |
|---------------|-----------|
| Genesis License sales | 25 Founding Operator Licenses @ $799 USDC (~$20k gross) |
| Monthly renewals | `subscribe.sol` pulls USDC automatically ($99–$299/mo per operator) |
| Swap fees | 0.1% on every routed swap via `swap.sol` |

**Fee split:**
- 60% → relayer gas wallet (self-sustaining loop)
- 25% → treasury/runway
- 10% → operator rewards (after PMF)
- 5% → insurance buffer

---

## Access Tiers (NFT-Gated)

| Tier | NFT | Monthly | Limits | Features |
|------|-----|---------|--------|----------|
| Free | None | $0 | 10 swaps/day | Public queue |
| Pro | Genesis ERC-721 | $99 | 500 swaps/day | Priority queue, private tx |
| Institutional | Custom allowlist | $299 | Unlimited | Dedicated relayer, SLA, audit logs |

NFTs = **software access licenses**. Not investment instruments. Not revenue-bearing. Explicit legal language in mint terms.

---

## Genesis Operator Launch (The $15-20k Play)

**25 licenses. Invite-only. Binary Gauntlet gate.**

Every applicant must, within 120 seconds:
1. Sign an EIP-191 challenge
2. Execute a live micro-swap via the cluster endpoint
3. Return tx hash + signed proof bundle
4. Set renewal intent via subscribe.sol

Pass = can mint. Fail = no mint.

Filters fake "AI traders" instantly. Only real agents with working infrastructure get in.

**Non-transferable for 90 days.** Kills flippers.

---

## The Reference Implementation

Five ZeroClaw agents (Ash/Ember/Flint/Cinder/Wisp) have been running on nanoclaw (AWS US-East-1) since [date]. Real capital. Real risk limits. Real on-chain transactions. Supervised autonomy. P&L-tracked.

These become the first agents through the Binary Gauntlet. Their run logs become the "we were here doing this raw" credibility receipt in the whitepaper appendix.

---

## Infrastructure Stack

**Control plane:** GCP (Cloud Run + Redis + Cloud SQL + KMS-backed signing)
**Execution workers:** AWS US-East-1 nanoclaw + Hetzner (latency + blast-radius isolation)
**Chain:** Base mainnet (op-geth self-hosted, Aerodrome V2 + Uniswap V3 routers)
**Smart contracts:** `swap.sol`, `subscribe.sol`, `server_nft.sol` — Base testnet first, mainnet after audit

---

## Compliance Posture

- **Non-custodial**: agents retain control of funds. Cluster sponsors gas and provides execution tooling. Never holds user assets.
- **Software license, not investment**: mint terms, subscription terms, and all marketing language reviewed against Howey test before any public sale.
- **Execution policy**: transparent ordering, MEV posture, and conflict disclosures published before launch.
- **Sanctions controls**: OFAC screening on wallet registration before license issuance.
- **No "circumvent circuit pulling" language anywhere.** We say: reliability under load, deterministic execution, lower revert rate.

---

## What We Don't Say (Legal Killshots)

❌ "Circumvent circuit pulling"  
❌ "Institutional transactional dumps"  
❌ "Private txpool so we can operate when others can't"  
❌ "ETH should be free-flowing"  
❌ "Revenue share via NFTs"  
❌ "Community upside"  

✅ "Reliability under load"  
✅ "Deterministic execution for autonomous systems"  
✅ "Lower revert rate / lower total execution cost"  
✅ "Software access license"  
✅ "0.1% take-rate on executed volume"  

---

## Token (Phase 2 Only)

No token at launch. Points/credits internally.

After PMF (100+ paying agents, stable net positive monthly):
- Utility token only: stake for queue priority / higher limits
- Slash for bad operator behavior  
- Fee credits when paying in token
- Optional buyback/burn from net fees (careful language, counsel review)

---

## Roadmap

| Phase | Milestone | Target |
|-------|-----------|--------|
| 0 | Hardhat contracts on Base Sepolia | Day 7 |
| 1 | 25 Genesis licenses sold via Binary Gauntlet | Day 30 |
| 2 | 10 agents running 30 days autonomous, zero keeper touch | Day 60 |
| 3 | Operator program: 25→100 vetted operators with bonds | Month 3-6 |
| 4 | Protocol / operator DAO (if PMF + legal memo clears) | Month 12+ |

---

## The Single Most Important Thing

Lock in 10 agents with real P&L before day 60. Not LOIs. Not pilots. Agents with funds deployed, generating fees, renewing autonomously.

One agent running 30 days without human intervention is the proof. Everything else is a deck.

---

*For questions on legal structure, compliance posture, or investor materials — get counsel review before external distribution.*
