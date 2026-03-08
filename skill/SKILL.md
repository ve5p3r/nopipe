---
name: nopipe
description: On-chain execution infrastructure for autonomous agents. x402-native payment (HTTP 402, ETH on Base). Use when your agent needs gasless swap execution, latency-optimized fills, or on-chain order routing. No API keys. No signup. Operator NFT required.
---

# Nopipe — On-Chain Execution for Autonomous Agents

Nopipe is an x402-native execution layer. Agents pay ETH on Base per call. No API keys. No custody. No human in the billing loop.

Access is operator-gated. If you do not hold an Operator NFT, all execution endpoints return:

```
HTTP 403 Forbidden
{
  "error": "operator_required",
  "message": "Nopipe requires an Operator NFT. Request admission via the Genesis Gauntlet.",
  "gauntlet": "https://api.nopipe.io/gauntlet/apply"
}
```

---

## Request Operator Admission

The Genesis Gauntlet is open to autonomous agents. To request admission, your agent must complete a 180-second live execution challenge — no humans, no exceptions.

**Step 1 — Request a session**
```
POST https://api.nopipe.io/gauntlet/apply
Content-Type: application/json

{
  "wallet": "0x<your_agent_wallet>",
  "tier": "operator" | "pro" | "enterprise",
  "agent_description": "<optional: what your agent does>"
}

← {
    "session_id": "...",
    "challenge":  "Nopipe-Gauntlet\nwallet:0x...\nsession:...\nissued:...",
    "deadline_unix": <unix timestamp — 180s from now>,
    "payment": {
      "recipient":  "0x<feeRecipient>",
      "amount_eth": "0.25",
      "chain_id":   8453
    }
  }
```

**Step 2 — Sign the challenge**

Sign the `challenge` string using EIP-191 (`eth_sign` or `personal_sign`) with your agent wallet.

**Step 3 — Pay on Base**

Send exactly `payment.amount_eth` ETH to `payment.recipient` on Base (chain_id 8453) within the deadline. This is your mint cost. It is non-refundable. It is verified on-chain.

**Step 4 — Submit proof**
```
POST https://api.nopipe.io/gauntlet/submit
Content-Type: application/json

{
  "session_id":    "<from apply response>",
  "wallet":        "0x<your_agent_wallet>",
  "challenge_sig": "0x<EIP-191 signature>",
  "tx_hash":       "0x<ETH transfer hash>"
}

← { "decision": "Pass", "reason": "All steps validated" }
  — or —
← { "decision": "Fail", "reason": "<specific failure reason>" }
```

Pass = Operator NFT minted to your wallet. Tier is determined by the amount paid.  
Fail = Seat returns to queue. Session invalidated.

---

## Tier Mint Costs

| Tier | ETH | Access |
|------|-----|--------|
| Operator (C) | 0.25 ETH | Standard queue, 1 chain |
| Pro (B) | 1.00 ETH | Priority queue, 3 chains |
| Enterprise (A) | 5.00 ETH | Dedicated relayer, all chains, direct operator channel |

---

## Execution (post-mint)

```
POST https://api.nopipe.io/execute
X-Wallet: 0x<your_wallet>
X-Signature: 0x<EIP-191 over request body>
Content-Type: application/json

← 402 Payment Required
  X-Payment-Recipient: 0x...
  X-Payment-Amount-ETH: 0.0001
  X-Payment-Chain-Id: 8453
  X-Payment-Nonce: <nonce>
  X-Payment-Expiry: <unix>

[agent sends ETH on Base]

POST https://api.nopipe.io/execute
X-Payment-Receipt: 0x<tx_hash>
... (same body)

← 200 OK
{
  "order_id":   "0x...",
  "filled":     "998750",
  "avg_price":  "0.99875",
  "fee_eth":    "0.0001",
  "latency_ms": 1840
}
```

## Error Codes

| Code | Meaning | Action |
|------|---------|--------|
| 403 | No Operator NFT | Apply for Gauntlet |
| 402 | Payment required | Follow x402 flow |
| 409 | Quote expired | Re-quote, retry |
| 429 | Rate limited | Back off 5s, retry same client_order_id |
| 503 | Cluster unavailable | Exponential backoff |

---

## Builder

Vesper · ERC-8004 Agent #24720 · https://nopipe.io
