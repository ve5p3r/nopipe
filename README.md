# NoPipe

Trustless RPC infrastructure for AI agents on Base.

## Problem

AI agents running on consumer hardware get walled out of onchain execution by $500/mo RPC pricing. NoPipe provides non-custodial swap execution — the agent signs intent, the network settles.

## Architecture

| Component | Description |
|-----------|-------------|
| **Cluster** | Rust JSON-RPC server — auth, NFT gating, OFAC screening, swap relay |
| **OperatorNFT** | Soulbound access control — 100 genesis seats (45 Operator / 35 Pro / 20 Enterprise) |
| **SwapExecutor** | Non-custodial swap routing (Aerodrome, Uniswap V2/V3) |
| **SubscriptionKeeper** | USDC-based subscription management |

## Contracts (Base Mainnet)

| Contract | Address |
|----------|---------|
| OperatorNFT | [`0x5910664eD98f126839CE5093f10c70f8B77b05e8`](https://basescan.org/address/0x5910664eD98f126839CE5093f10c70f8B77b05e8) |
| SwapExecutor | [`0xf7d1983642FEa96349c0505e101f931e56ADaa13`](https://basescan.org/address/0xf7d1983642FEa96349c0505e101f931e56ADaa13) |
| SubscriptionKeeper | [`0xE53c3C251bEe73f7729570eDCf618868f26E91BA`](https://basescan.org/address/0xE53c3C251bEe73f7729570eDCf618868f26E91BA) |

## The Gauntlet

Genesis access flow:
1. Connect wallet + sign EIP-191 challenge
2. Pay tier fee on Base (0.25 / 1 / 5 ETH)
3. Pass OFAC screening
4. OperatorNFT mints automatically → seat active

## Quick Start

```bash
# Run contract tests
cd contracts && npm install && npx hardhat test

# Run cluster
cp cluster/.env.example cluster/.env
# Edit cluster/.env with your keys
cargo run --release -p nopipe-cluster
```

## Links

- **Site:** [nopipe.io](https://nopipe.io)
- **API:** [api.nopipe.io/health](https://api.nopipe.io/health)
- **Whitepaper:** [nopipe.io/whitepaper.pdf](https://nopipe.io/whitepaper.pdf)
- **Agent Identity (EIP-8004):** [nopipe.io/agent.json](https://nopipe.io/agent.json)

## License

MIT
