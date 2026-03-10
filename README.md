# NoPipe

Swap execution infrastructure for autonomous AI agents on Base.

## What it does

An AI agent calls the cluster RPC endpoint to execute token swaps. Access is gated by an OperatorNFT. The agent pays a subscription fee autonomously — no human required.

## Structure

| Directory | What |
|-----------|------|
| `cluster/` | Rust binary — JSON-RPC server, NFT gate, swap relay |
| `contracts/` | Solidity — SwapExecutor, SubscriptionKeeper, OperatorNFT |
| `docs/` | Strategy, PRDs, pitch |
| `context/` | Session notes and decision log |

## Run

```bash
# Contracts
cd contracts && npm install && npx hardhat test

# Cluster
BASE_RPC_HTTP=https://base-sepolia.infura.io/v3/KEY \
BASE_RPC_WS=wss://base-sepolia.infura.io/ws/v3/KEY \
SWAP_EXECUTOR=0x... \
SUBSCRIPTION_KEEPER=0x... \
OPERATOR_NFT=0x... \
RELAYER_PRIVATE_KEY=0x... \
FEE_RECIPIENT=0x... \
cargo run --release -p nopipe-cluster
```

## Status

Contracts: ✅ compile + 23/23 tests  
Cluster: ✅ compiles clean  
Deployed: ✅ Base Mainnet

- OperatorNFT: `0x5910664eD98f126839CE5093f10c70f8B77b05e8`
- SwapExecutor: `0xf7d1983642FEa96349c0505e101f931e56ADaa13`
- SubscriptionKeeper: `0xE53c3C251bEe73f7729570eDCf618868f26E91BA`
