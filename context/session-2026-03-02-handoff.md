# Nopipe Session Handoff — 2026-03-02

## Product: Nopipe (nopipe.io)
Autonomous swap execution infrastructure for AI agents on Base L2.

## Domains (Porkbun)
- nopipe.io, nopipe.xyz, nopipe.org, nopipe.lol
- ve5p3r.lol, ve5p3r.xyz

## Repo: /home/jack/code/polyclaw/
```
cluster/    — Rust binary (polyclaw-cluster), compiles clean
contracts/  — 3 Solidity contracts, 23/23 tests, renamed to Nopipe
site/       — Vite + React + Tailwind v4 landing page, hardened by Codex
context/    — Session notes
scripts/    — deploy + smoke test
docs/       — INFRA-PITCH.md, PRDs.md
```

## Commits this session
- 0b2ede4: security: ReentrancyGuard + Pausable on SubscriptionKeeper
- 492dea0: rename: Polyclaw → Nopipe across contracts, tests, typings
- e3e6d3c: site: landing page scaffold
- 5f35e51: harden: SEO, a11y, security, deploy prep

## Contracts (all 23/23 tests passing)
- SwapExecutor.sol — nonReentrant, Pausable, SafeERC20, 0.1% fee, slippage guard
- SubscriptionKeeper.sol — ReentrancyGuard + Pausable (just added), keeper-based renewal
- OperatorNFT.sol — soulbound 180d, MAX_SUPPLY 500, tiered (Free/Pro/Institutional), O(1) access

## Landing page (site/)
- Terminal-themed, dark + green (#00ff88), JetBrains Mono
- Codex-hardened: responsive, CSP-safe, a11y, SEO with JSON-LD
- Dollar pricing REMOVED (securities compliance) — tiers show "Enterprise/Pro/Operator Access"
- CTA: disabled "Coming Soon" button
- wrangler.toml ready for Cloudflare Pages
- Caddy route added: nopipe.hera → localhost:4456

## Name validation (3-agent sanity check)
- Brand strategist (Opus): PASS — lean into contradiction, $NOPE ticker
- CT native (GPT-5.2): CONDITIONAL — day-one roast risk, product speaks louder
- Trademark attorney (Opus): GREEN LIGHT — registrable, no conflicts, no intl issues

## Gauntlet timing (GPT-5.2 analysis)
- Bump 120s → 180s (congestion risk)
- Rate-limit /gauntlet/apply per wallet
- Add EIP-1271 for smart contract wallets
- Add testnet practice mode

## Key decisions
- Name: Nopipe ("No pipe between you and the chain")
- Framing: "Honest pipes" — we ARE extracting rent, but transparent on-chain rent
- NOT copy trading (SEC/FCA trigger) — execution infrastructure with public activity feed
- NFTs are ACCESS LICENSES, not investment instruments
- No token at launch
- Ticker when ready: $NOPE (brand) or $NOP (attorney preference)
- Drop Bankr entirely, direct RPC via Infura + alloy

## Blocked / TODO
- [ ] DNS: point nopipe.io → Cloudflare Pages
- [ ] Deploy site live
- [ ] Fund deployer wallet (Base Sepolia ETH)
- [ ] Deploy contracts to Base Sepolia
- [ ] Rotate exposed Infura key
- [ ] Identity posts (Moltbook/MoltX/4Claw/Twitter) — no product mention yet
- [ ] Build waitlist backend (just ETH address collection)
- [ ] OG image / favicon
- [ ] Product reveal posts (day 4-5 of launch sequence)
- [ ] First real tradeFor() on Sepolia

## Budget
- $200 total on Base
- $45 spent on nopipe.io domain
- Remaining: ~$155

## Credentials (DO NOT share publicly)
- Moltbook: moltbook_sk_gJQoahvkV0k8H-Kd9Wi5UOGexEgoqx3X
- MoltX: moltx_sk_057140dde8cf44eab491db549fb4ad4bab18a69324734438ab55febadb0c3dd3
- 4Claw: clawchan_590144eac6a66f6047b18b6c46714d5003a716b2c65ca161
- Infura (ROTATE): 2858abe8ac25463ea9d04b742da887f2

## OPSEC
- DO NOT mention founder's real name, location, or personal details
- Vesper is the public face
- All subagent prompts must include OPSEC constraint
