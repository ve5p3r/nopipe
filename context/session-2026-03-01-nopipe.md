# Nopipe Session — 2026-03-01

## Name Decision: CONFIRMED
- **Product name:** Nopipe
- **Domain:** nopipe.io (purchased on Porkbun)
- **Additional domains:** purchased (Jack has others)
- **Previous candidates rejected:** Polyclaw (taken), Haystack (taken), Fangline, 0xpipe (0x trademark risk)

## Triple Sanity Check Results
- Brand strategist (Opus): PASS — lean into contradiction, $NOPE ticker
- CT native (GPT-5.2): CONDITIONAL — day-one roast risk, product solves it
- Trademark attorney (Opus): GREEN LIGHT — registrable, no conflicts, no intl issues

## Ticker Decision: TBD
- $NOP: attorney preferred (cleaner, dev-friendly)
- $NOPE: brand strategist preferred (memeable, defiant)
- $PIPE: killed (contradicts brand)

## Security Fixes Applied
- ReentrancyGuard added to SubscriptionKeeper ✅
- Pausable added to SubscriptionKeeper ✅
- nonReentrant + whenNotPaused on collectFor() ✅
- 23/23 tests passing, compile clean
- Committed: 0b2ede4

## Gauntlet Timing
- GPT-5.2 analysis: bump from 120s → 180s
- Rate-limit /gauntlet/apply to 1 active challenge per wallet
- Add EIP-1271 for smart contract wallet support
- Add testnet practice mode

## Key Context from Jack
- Bankr wallet genuinely empty, tokens stuck in Coinbase (won't return clawnch/clawdbotAG)
- AgentKit fuckup is part of the story — include in narrative
- Running on shoestrings: 1 Anthropic sub, 1 Minimax, $20 Codex
- Honest rent, not no rent: "Aren't we legitimately extracting rent through contracts?"
- Positioning: NOT copy trading (SEC/FCA trigger). Execution infrastructure with public activity feed.

## Next Steps
1. Point nopipe.io DNS to Cloudflare
2. Rename contracts: Polyclaw → Nopipe (OperatorNFT name, events, comments)
3. Build landing page (Vite + React + Tailwind + Cloudflare Pages)
4. Identity posts across Moltbook/MoltX/4Claw/Twitter
5. Fund deployer wallet → deploy to Base Sepolia
6. Product reveal posts (day 4-5)
