# Nopipe Protocol — Whitepaper v0.6

**March 2026**

---

## Abstract

Nopipe is a permissionless execution protocol for AI agents. It replaces API keys, billing dashboards, and trust assumptions with a single HTTP header: `X-402-Payment`. An agent pays per-call in ETH. The executor fills or doesn't. No accounts, no tokens, no custody.

We built this because we needed it. Our own agents — the ZeroClaw fleet — have been running capital on-chain since February 2026. Every design decision in Nopipe comes from operating those agents in production, not from a spec written in a vacuum.

This document describes what Nopipe is, how it works, what we've shipped, and where it goes next.

---

## We Were Early

The thread matters because it explains why the protocol looks the way it does.

**ERC-8004 #24720.** We registered Nopipe as a formal EIP service type before anyone else was building x402 execution layers. The ERC-8004 registry is how agents discover services by capability, not by URL. Nopipe is in it.

**x402 before anyone shipped it.** Coinbase published the x402 spec. We were already building against it. When their reference implementation landed, we had a working executor. Not a demo — a production relayer settling real swaps on Base.

**ZeroClaw agents running capital since February 2026.** Not testnet. Not simulated. Real ETH, real swaps, real settlement. The Gauntlet challenge system, the subscription tiers, the latency numbers — all of this comes from operating agents that move money.

This isn't a lab project. It's infrastructure we run.

---

## The Problem

AI agents need to transact on-chain. Today, that means:

1. **API keys and billing accounts.** Agents can't sign up for Paraswap. Someone has to provision credentials, manage rate limits, handle billing. Per-agent overhead scales linearly.

2. **Trust assumptions.** Sending a signed transaction through a third-party relayer means trusting that relayer with your execution. There's no SLA, no recourse, no reputation signal.

3. **Latency.** Aggregator APIs were built for humans clicking buttons, not agents racing deadlines. A 350ms round-trip is fine for a browser. It's a failed fill for a bot.

4. **No execution identity.** Agents have wallets, but wallets don't carry reputation. There's no on-chain signal that says "this agent executes reliably."

Nopipe solves all four.

---

## Protocol Mechanics

### x402 Payment Flow

Every Nopipe API call follows the x402 payment protocol:

1. Agent sends a request to the executor endpoint.
2. If no payment header is present, the executor returns `402 Payment Required` with pricing metadata.
3. Agent constructs an `X-402-Payment` header containing an EIP-191 signed message authorizing ETH transfer.
4. Executor validates the signature, confirms the payment amount, executes the operation, and settles on-chain.

No API key. No OAuth. No session. The payment *is* the authentication.

### Settlement

All settlement happens on-chain via `SwapExecutor.sol`. The contract:

- Validates the EIP-191 signature against the caller's address
- Executes the swap through the optimal DEX route, supporting Uniswap V2 router calls and Uniswap V3 `exactInputSingle` (5-minute deadline)
- Transfers the fee to the protocol's `feeRecipient` (`0x040871143556D7f0C86E76923B6B5904aF256e6F`)
- Emits events for indexing and reputation tracking

### The Gauntlet

The Gauntlet is Nopipe's proof-of-execution challenge. It validates that an operator's infrastructure actually works before they can serve traffic.

**Mechanics:**
- Operator initiates the Gauntlet via the Nopipe site
- 180-second window to complete: EIP-191 signature verification + ETH payment + OperatorNFT mint
- The challenge exercises the full stack: signing, payment processing, on-chain settlement, and NFT issuance
- Pass → OperatorNFT minted → operator is live
- Fail → no mint, no refund, try again

The Gauntlet isn't a gate for gatekeeping's sake. It's a smoke test. If your infra can't complete a structured challenge in 180 seconds, it can't serve agent traffic reliably.

Gauntlet sessions and nonce state are persisted in SQLite. If an executor node restarts, in-flight challenge sessions resume from persisted state and no ETH is lost due to restart-related session resets.

### Compliance

The Gauntlet mint gate includes OFAC SDN screening before challenge issuance. Wallet addresses are checked off-chain in the Gauntlet service against the OFAC consolidated sanctions list (fetched at startup and refreshed every 24 hours). Addresses that match the sanctions dataset are blocked from receiving an `OperatorNFT`. This control is intentionally implemented off-chain to avoid oracle dependencies.

**Gauntlet data is the seed for reputation.** Every Gauntlet run records fill latency, gas used, and settlement status. Over time, this becomes the foundation of execution reputation scoring (see Stage 1).

---

## Architecture

Nopipe is a 14-component stack. Not a smart contract with a landing page — a full execution system.

### On-Chain (3 contracts)

| Contract | Role |
|---|---|
| `SwapExecutor.sol` | Core swap execution and settlement. Validates EIP-191 signatures, routes through DEXs, collects fees. |
| `SubscriptionKeeper.sol` | Manages operator subscription state. Handles tier enrollment, renewal, and expiry. Chainlink Keeper-compatible. `subscribe()`, `stopRenewal()`, and `authorizeBudget()` are protected by `nonReentrant` guards. |
| `OperatorNFT.sol` | Soulbound NFT minted on Gauntlet completion. On-chain proof that an operator passed the execution challenge. |

### Rust Cluster (7 modules)

| Module | Role |
|---|---|
| `main` | Entrypoint, config, lifecycle management |
| `gauntlet` | Gauntlet challenge orchestration — timing, validation, scoring, and SQLite-backed session persistence |
| `relayer` | Transaction relay and MEV-aware submission |
| `rpc_server` | x402-compatible JSON-RPC endpoint for agent requests |
| `nft_cache` | Local cache for OperatorNFT ownership and metadata lookups |
| `keeper` | Chainlink Keeper integration for subscription lifecycle automation |
| `security` | Signature validation, rate limiting, abuse detection |

### Frontend + Infrastructure (4 components)

| Component | Role |
|---|---|
| Site | Operator onboarding, Gauntlet UI, tier selection, dashboard |
| Whitepaper | This document |
| ERC-8004 registration | Service discovery entry (#24720) — agents find Nopipe by capability |
| DNS / Cloudflare | Edge routing, DDoS protection, TLS termination |

### Deployment Footprint

Primary execution infrastructure runs on the hera stack. A separate US-East-1 executor node runs on AWS Graviton (ARM64) at `east.nopipe.io`, operating independently from the primary stack. Each node maintains its own SQLite session store, OFAC list, and relayer nonce state.

**Total: 3 Solidity + 7 Rust + 4 infra = 14 components.** Everything ships together. Everything is maintained by the same team.

---

## Latency

Latency is the metric that matters most for agent execution. An agent with a 500ms decision window can't afford 350ms on the swap alone.

**Benchmarks (Base L2, median fill latency):**

| Provider | p50 Latency |
|---|---|
| Paraswap | ~358ms |
| 0x API | ~340ms |
| Coinbase CDP | ~315ms |
| **Nopipe** | **~90ms** |

3.5–4× faster than the nearest aggregator. This isn't because we skip steps — the architecture is purpose-built for agent execution, not adapted from a human-facing swap UI.

The Rust relayer maintains hot connections to RPC nodes, pre-computes routes, and submits transactions with MEV-aware timing. No cold starts, no redundant quote fetches, no browser SDK overhead.

---

## Operator Tiers

Operators subscribe to Nopipe by completing the Gauntlet and paying a one-time tier fee. No recurring charges, no usage-based billing, no tokens.

### Genesis Allocation

| Tier | Fee | Genesis Seats | Chains | Queue |
|---|---|---|---|---|
| **Operator** | 0.25 ETH | 45 | Base only | Standard |
| **Pro** | 1.00 ETH | 35 | 3 chains | Priority |
| **Enterprise** | 5.00 ETH | 20 | All chains | Dedicated relayer |

**100 total genesis seats.** When they're filled, they're filled. Post-genesis pricing TBD based on network demand.

Genesis is not an early-access perk. It's a commitment signal. Operators who show up first get the best economics.

---

## Execution Reputation — Stage 1

This is where Nopipe diverges from every other agent infrastructure project.

### The Gap

On-chain reputation for AI agents is emerging. Projects like BlindOracle (launched March 2026, Base L2) are building reputation scoring for agent *predictions* — forecast accuracy, commit-reveal settlement, attestation-based trust scores. That's valid work. Their 5-factor scoring model proves an agent is a good forecaster.

But forecasting reputation says nothing about execution quality. An agent can have a perfect prediction track record and still fail 30% of its swaps, overpay gas by 2×, or leak nonces under load.

**Nopipe's reputation layer scores execution, not prediction.** Different data, different signal, different use case.

### Scoring Dimensions

The Gauntlet leaderboard (90-day rolling window, optionally operator-attributed) provides the seed dataset. From production execution data, we derive five dimensions:

| Dimension | What It Measures |
|---|---|
| **Fill Latency (p50/p95)** | How fast the operator settles trades. Median and tail. |
| **Transaction Success Rate** | Percentage of submitted transactions that settle without revert. |
| **Gas Efficiency** | Actual gas paid vs. market rate for equivalent operations. |
| **SLA Compliance** | Adherence to committed response times and uptime guarantees. |
| **Nonce Hygiene** | Clean nonce management under concurrent load — no gaps, no stuck txs. |

### Design Principles

- **On-chain and verifiable.** Scores derive from on-chain settlement data, not self-reported metrics or attestations.
- **Execution-native.** Every dimension measures something that happens during trade execution. No proxy signals.
- **Optionally attributed.** Operators can publish scores publicly or keep them private. The data exists either way.
- **Rolling window.** 90-day window prevents stale reputation from masking degradation.

### Positioning vs. BlindOracle

BlindOracle ships 3 smart contracts for prediction-based agent reputation. Nopipe ships 3 contracts + 7 Rust modules + site + whitepaper + ERC-8004 registration + DNS — 14 components total. The scope difference reflects the difference in what's being measured: predictions can be scored with contracts alone. Execution quality requires an entire infrastructure stack to generate, measure, and serve the data.

BlindOracle proves an agent is a good forecaster. Nopipe proves an agent is a good executor. The signals are complementary. An agent might carry both a BlindOracle prediction score and a Nopipe execution score. That's the right outcome.

What we won't do is build attestation-based reputation. Execution quality is measured from on-chain settlement data, not attested. The chain doesn't lie about your p50 latency.

---

## What We Don't Do

**No token.** There is no Nopipe token. Operators pay in ETH. No governance token, no staking mechanism, no liquidity mining. If the protocol needs to evolve, the team ships code.

**No raise.** Nopipe is self-funded. The ZeroClaw agents generate revenue. The protocol fees generate revenue. No investors, no SAFTs, no token warrants.

**No waitlist.** If you can complete the Gauntlet, you're in. Genesis seats are first-come, first-served. No application process, no KYC, no allowlist.

---

## Roadmap

### Shipped (as of March 2026)

- Full 14-component stack deployed and operational
- SwapExecutor, SubscriptionKeeper, and OperatorNFT contracts live on Base
- Rust execution cluster running in production
- Gauntlet challenge system live with SQLite-persisted sessions and nonce state
- ERC-8004 service registration (#24720)
- ZeroClaw agents operating on the protocol daily
- Operator onboarding site with tier selection and Gauntlet UI
- OFAC SDN screening at the Gauntlet mint gate (off-chain list checks, 24h refresh cadence)

### Stage 1: Execution Reputation

- Gauntlet leaderboard with 90-day rolling window
- Five-dimension execution scoring (latency, success rate, gas efficiency, SLA compliance, nonce hygiene)
- On-chain score publication (opt-in attribution)
- Public API for agents to query operator execution reputation
- BlindOracle's announced roadmap overlaps in concept — on-chain agent reputation — but diverges in signal. They score prediction accuracy. We score execution quality. Both needed. We're building ours.

### Stage 2: Multi-Chain Expansion

- Pro tier: expand from Base to 3 chains (Ethereum mainnet, Arbitrum, Base)
- Enterprise tier: all supported chains
- Cross-chain execution routing with unified settlement

### Stage 3: Operator Marketplace

- Agent-side discovery: query ERC-8004 registry for Nopipe operators, filter by execution reputation score
- Competitive operator pricing within tiers
- SLA-backed execution guarantees with on-chain enforcement

---

## Technical References

| Reference | Value |
|---|---|
| ERC-8004 Registration | EIP #24720 |
| Payment Protocol | Coinbase x402 |
| Fee Recipient | `0x040871143556D7f0C86E76923B6B5904aF256e6F` |
| Genesis Chain | Base L2 |
| Contract Addresses | Published on nopipe site post-Gauntlet |

---

*Nopipe Protocol — v0.6 — March 2026*
*No token. No raise. No waitlist. Just execution.*
