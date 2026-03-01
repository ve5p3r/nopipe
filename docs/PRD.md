# Polyclaw PRD

**Status:** Gate 3 approved 2026-02-23
**Goal:** Extend the Terrarium (nanoclaw EC2) with a Polymarket execution layer. Start with weather markets. Operate as maker, not taker.

---

## Architecture

### Agent Mapping (existing ZeroClaw, additive only — do NOT modify existing SOUL.md or skills)
| Agent | Current Role | Polyclaw Layer |
|-------|-------------|----------------|
| Ash (zc-0) | Risk Manager | Layer 3 — Portfolio & Risk oversight |
| Ember (zc-1) | MiniMax M2.5 | Layer 1 — Forecasting (base rates, ensemble) |
| Flint (zc-2) | MiniMax M2.5 | Layer 1 — Resolution Monitor |
| Cinder (zc-3) | Research Analyst | Layer 1 — Calibration + signal synthesis |
| Wisp (zc-4) | Reporter (@Ve5p3rbot) | Layer 5 — P&L reporting, Telegram alerts |

### Coordination Layer
`orchestrator.db` (SQLite) at `/opt/polyclaw/data/` — ZeroClaw agents write signals here, Python executor reads from it.

### Data Ingestion
- Polymarket CLOB WebSocket (`wss://ws-subscriptions-clob.polymarket.com/ws/market`)
- Scrapling (`pip install "scrapling[ai]"`) — weather.gov, news sites, GDELT
- Redis pub/sub for internal routing

### Stack
- Python + py-clob-client + asyncio + Redis + SQLite
- Deployed as systemd services on nanoclaw (98.89.6.40, EC2 US-East-1)

---

## File Structure
```
/opt/polyclaw/
├── config/
│   ├── settings.py        # API keys, thresholds, PAPER_MODE=True
│   └── markets.py         # Target weather market list
├── data/
│   └── orchestrator.db    # SQLite coordination DB
├── skills/                # New ZeroClaw skills (additive only)
│   ├── polyclaw_signal.md
│   └── polyclaw_report.md
├── ingest/
│   ├── clob_ws.py
│   └── scraper.py
├── signal/
│   ├── alpha.py
│   ├── validator.py
│   └── devils_advocate.py
├── execution/
│   ├── order_manager.py
│   └── fee_signer.py
├── risk/
│   ├── portfolio.py
│   └── circuit_breaker.py
├── monitoring/
│   └── reporter.py
└── main.py
```

---

## Tasks

### Task 0 — Nanoclaw Security Hardening (PREREQUISITE)
- Install fail2ban, configure SSH jail (max 5 retries, 10min ban)
- Verify firewall: only ports 22, 80, 443 open
- Audit systemd services, confirm no API keys exposed in env files
- Acceptance: `fail2ban-client status sshd` shows jail active

### Task 1 — Scaffold, Config & DB
- Create all directories and empty module files
- requirements.txt: `py-clob-client`, `scrapling[ai]`, `redis`, `aiohttp`
- settings.py with all constants
- orchestrator.db schema: signals, tasks, pnl, events tables
- Acceptance: imports work, DB tables created

### Task 2 — CLOB WebSocket
- ClobWsClient.connect() → WebSocket stream
- subscribe_orderbook(market_id) → writes to events table
- Acceptance: 10 orderbook events received and stored from 1 live weather market

### Task 3 — Scrapling Ingest
- WeatherScraper.get_forecast(location) → dict (weather.gov)
- NewsScraper.get_headlines(query) → list (GDELT)
- Acceptance: both functions return valid data

### Task 4 — ZeroClaw Skills (additive only)
- polyclaw_signal.md: instructs agent to write probability estimate to signals table
- polyclaw_report.md: instructs agent to read P&L and summarize
- NO changes to existing SOUL.md or skills
- Acceptance: Cinder can invoke skill, row appears in DB

### Task 5 — Signal Generator
- calc_edge(), kelly_size(), validate_signal() → TradeDecision(TRADE/SKIP)
- Acceptance: p_model=0.65, p_market=0.55 → TRADE; p_model=0.56, p_market=0.55 → SKIP

### Task 6 — Execution (PAPER_MODE=True)
- place_maker_order(), cancel_replace(), fee-aware signing
- PAPER_MODE=True by default — writes to DB, does NOT call Polymarket API
- Acceptance: paper orders placed and logged

### Task 7 — Circuit Breaker
- alert at -10% drawdown (Discord + Telegram)
- halt at -20% (cancel all orders, set TRADING_HALTED flag)
- Acceptance: mock injections trigger correct responses

### Task 8 — Monitoring & Reporting
- Hourly P&L summary to #🦎-terrarium (1475268408957468793) and @Ve5p3rbot
- Deploy as cron
- Acceptance: message appears in both channels

---

## Go-Live Gate
Jack manually sets `PAPER_MODE=False` and approves starting capital.

## NON-GOALS
- No Rust, no BTC/ETH markets, no Kafka, no multi-exchange
- No autonomous capital deployment
- NO interference with existing Terrarium agents, skills, or trading
- NO changes to existing SOUL.md files
