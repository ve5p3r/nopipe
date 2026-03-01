---
## PRD: JSON-RPC 2.0 Cluster Server
### Gate 1 — Problem & Goal
Polyclaw needs a single machine-readable execution endpoint so agents can swap, quote, register, and check status without custom integrations or manual gas workflows. The solution is an axum-based JSON-RPC 2.0 server under the ZeroClaw cluster plugin that performs auth (EIP-191), access control (NFT tier), and execution (relayer -> `tradeFor()`) in one request path. Done looks like `swap_execute` returning a valid onchain `tx_hash` and all four methods (`swap_execute`, `swap_quote`, `agent_register`, `swap_status`) passing integration tests.

### Gate 2 — Architecture
**Components**
- HTTP layer: axum 0.7 router (forking `src/gateway/mod.rs` patterns for state, timeout, body limits, JSON handling).
- JSON-RPC core: request/response envelope parser, method dispatcher, and JSON-RPC error mapper.
- Auth layer: EIP-191 signature verification + nonce/replay guard.
- Access layer: NFT tier lookup via `NftVerificationCache`.
- Execution layer: relayer client that submits `SwapExecutor.tradeFor(...)` and records status by request id.
- Status storage: in-memory `DashMap<Uuid, SwapExecutionStatus>` with optional persistence hook.

**Stack**
- Rust + tokio + axum + serde/serde_json + anyhow/thiserror.
- `alloy` for signature recovery and EVM typing.
- `dashmap` for request status map.

**NON-GOALS**
- No websocket RPC transport in v1.
- No JSON-RPC batch requests in v1.
- No long-term DB persistence for swap status in v1 (memory + optional replay from chain only).

### Gate 3 — Implementation Plan
1) **Create cluster plugin scaffold and boot entrypoints**
- File(s):
  - `/home/jack/code/zeroclaw/src/plugins/mod.rs`
  - `/home/jack/code/zeroclaw/src/plugins/cluster/mod.rs`
- Implement:
  - `pub mod rpc_server;`
  - `pub mod nft_cache;`
  - `pub mod relayer;`
  - `pub mod keeper;`
  - `pub mod gauntlet;`
  - `pub async fn run_cluster_server(config: ClusterConfig) -> anyhow::Result<()>`
- Acceptance criteria:
  - Project compiles with `plugins::cluster` enabled in module graph.
  - `run_cluster_server` starts listener and logs bind address.

2) **Implement JSON-RPC envelope + dispatcher**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/rpc_server.rs`
- Implement types/functions:
  - `#[derive(Deserialize)] struct JsonRpcRequest { jsonrpc: String, method: String, params: serde_json::Value, id: serde_json::Value }`
  - `#[derive(Serialize)] struct JsonRpcResponse { jsonrpc: &'static str, result: Option<serde_json::Value>, error: Option<RpcErrorObject>, id: serde_json::Value }`
  - `#[derive(Serialize)] struct RpcErrorObject { code: i64, message: String, data: Option<serde_json::Value> }`
  - `#[derive(Clone)] pub struct ClusterAppState { ... }`
  - `pub fn build_cluster_router(state: ClusterAppState) -> axum::Router`
  - `async fn handle_rpc(State(state): State<ClusterAppState>, Json(req): Json<JsonRpcRequest>) -> impl IntoResponse`
  - `async fn dispatch_method(state: &ClusterAppState, req: JsonRpcRequest) -> Result<serde_json::Value, RpcErrorObject>`
- Acceptance criteria:
  - Unknown method returns JSON-RPC error `-32601`.
  - Invalid params returns `-32602`.
  - Valid call returns `{ "jsonrpc": "2.0", "result": ..., "id": ... }`.

3) **Implement method handlers (`swap_execute`, `swap_quote`, `agent_register`, `swap_status`)**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/rpc_server.rs`
- Implement types/functions:
  - `#[derive(Deserialize)] struct SwapExecuteParams { wallet: Address, token_in: Address, token_out: Address, amount_in: U256, router: Address, slippage_bps: u32, nonce: String, sig: String }`
  - `#[derive(Deserialize)] struct SwapQuoteParams { token_in: Address, token_out: Address, amount_in: U256, router: Address }`
  - `#[derive(Deserialize)] struct AgentRegisterParams { wallet: Address, nonce: String, sig: String, metadata: Option<serde_json::Value> }`
  - `#[derive(Deserialize)] struct SwapStatusParams { request_id: String }`
  - `async fn handle_swap_execute(...) -> Result<serde_json::Value, RpcErrorObject>`
  - `async fn handle_swap_quote(...) -> Result<serde_json::Value, RpcErrorObject>`
  - `async fn handle_agent_register(...) -> Result<serde_json::Value, RpcErrorObject>`
  - `async fn handle_swap_status(...) -> Result<serde_json::Value, RpcErrorObject>`
- Acceptance criteria:
  - `swap_execute` successful path returns both `request_id` and `tx_hash`.
  - `swap_quote` calls contract quote path and returns deterministic numeric output.
  - `agent_register` rejects replayed nonce.
  - `swap_status` returns one of: `pending | submitted | confirmed | failed`.

4) **Add EIP-191 verification + replay prevention in execute/register flows**
- File(s):
  - `/home/jack/code/zeroclaw/src/plugins/cluster/rpc_server.rs`
  - `/home/jack/code/zeroclaw/src/plugins/cluster/security.rs`
- Implement functions:
  - `pub fn build_eip191_message(domain: &str, wallet: Address, nonce: &str, payload_hash: B256) -> String`
  - `pub fn verify_eip191_signature(message: &str, sig_hex: &str, expected_wallet: Address) -> anyhow::Result<()>`
  - `pub struct NonceStore { inner: DashMap<Address, HashSet<String>> }`
  - `impl NonceStore { pub fn consume_nonce(&self, wallet: Address, nonce: &str) -> bool }`
- Acceptance criteria:
  - Invalid signature rejected before any onchain call.
  - Reused nonce returns deterministic auth error.
  - Signature verification uses recovered signer address equality.

5) **Wire NFT tier check + relayer trade execution**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/rpc_server.rs`
- Implement flow in `handle_swap_execute`:
  - `let tier = state.nft_cache.get_tier(wallet).await?;`
  - enforce `tier >= state.policy.min_tier_for_swap`.
  - call `state.relayer.submit_trade_for(TradeForRequest { ... })`.
  - store status in `state.swap_statuses: DashMap<String, SwapExecutionStatus>`.
- Acceptance criteria:
  - Wallet below required tier gets access-denied error (no tx submitted).
  - Allowed wallet submits tx and receives non-zero `tx_hash`.
  - Status record is queryable via `swap_status` immediately after submission.

6) **Tests + local integration harness**
- File(s):
  - `/home/jack/code/zeroclaw/src/plugins/cluster/rpc_server.rs` (unit tests module)
  - `/home/jack/code/zeroclaw/tests/cluster_rpc_integration.rs`
- Implement tests:
  - JSON-RPC envelope compliance.
  - Signature success/failure.
  - Tier gating deny/allow.
  - Relayer mocked submit returning expected `tx_hash`.
- Acceptance criteria:
  - `cargo test --test cluster_rpc_integration` passes.
  - All 4 methods have at least one passing success-path test.

### Dependencies
- `SwapExecutor.sol`, `SubscriptionKeeper.sol`, `OperatorNFT.sol` deployed ABI + addresses available.
- Local Base RPC endpoint reachable from ZeroClaw.
- Relayer module (`relayer.rs`) and NFT cache module (`nft_cache.rs`) exposed in `cluster` plugin.
- Config values for contract addresses, min tier policy, and relayer key path/env.

### Estimated effort
- ~900–1200 LOC (including tests).
- 2.5–3.5 days for one strong Rust dev.

---
## PRD: NFT Verification Cache
### Gate 1 — Problem & Goal
Checking NFT access tier on every request via direct chain calls creates avoidable latency and RPC load. The solution is a local `DashMap<Address, CacheEntry>` with 5-minute TTL and event-driven invalidation from `Transfer` logs so hot reads are near-memory speed while stale data is corrected quickly. Done looks like warm cache reads <2ms p95 and cold misses resolving tier by `eth_call` in ~10–20ms against the local Base node.

### Gate 2 — Architecture
**Components**
- In-memory cache keyed by operator wallet address.
- Read path: TTL check -> hit return, miss refresh via `OperatorNFT.highestTier(address)`.
- Invalidation path: websocket subscription to `Transfer(address,address,uint256)` for OperatorNFT contract.
- Metrics: hit/miss counters, refresh latency histogram, invalidation count.

**Stack**
- Rust + `dashmap` + `tokio`.
- `alloy` providers: HTTP (reads) + WS (subscriptions).
- `tracing` for observability.

**NON-GOALS**
- No distributed cache in v1.
- No cross-process coherence guarantees in v1.
- No historical event backfill scanner beyond startup catch-up window.

### Gate 3 — Implementation Plan
1) **Build cache core type with TTL semantics**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/nft_cache.rs`
- Implement:
  - `pub struct CacheEntry { pub tier: u8, pub expires_at: std::time::Instant, pub fetched_at_block: u64 }`
  - `pub struct NftVerificationCache { entries: DashMap<Address, CacheEntry>, ttl: Duration, ... }`
  - `impl NftVerificationCache { pub fn new(...) -> Self }`
  - `fn read_if_fresh(&self, wallet: Address) -> Option<u8>`
- Acceptance criteria:
  - Entry expires exactly at `now + 300s`.
  - Stale entries are treated as misses (never returned as valid tier).

2) **Implement cold-miss fetch path from onchain `highestTier`**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/nft_cache.rs`
- Implement:
  - `async fn fetch_tier_from_chain(&self, wallet: Address) -> anyhow::Result<(u8, u64)>`
  - `pub async fn get_tier(&self, wallet: Address) -> anyhow::Result<u8>`
  - `fn upsert_entry(&self, wallet: Address, tier: u8, block: u64)`
- Acceptance criteria:
  - Missing wallet returns tier `0` or contract-defined value consistently.
  - Cold miss populates cache and subsequent call becomes warm hit.
  - Measured cold miss latency in local test environment averages 10–20ms.

3) **Add websocket invalidation from NFT `Transfer` events**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/nft_cache.rs`
- Implement:
  - `pub async fn start_invalidation_listener(self: Arc<Self>) -> anyhow::Result<tokio::task::JoinHandle<()>>`
  - `async fn handle_transfer_log(&self, log: alloy::rpc::types::Log) -> anyhow::Result<()>`
  - `fn invalidate_wallet(&self, wallet: Address)`
- Invalidation rules:
  - Invalidate `from` when `from != Address::ZERO`.
  - Invalidate `to` when `to != Address::ZERO`.
- Acceptance criteria:
  - Transfer event causes invalidation of affected wallets within one event loop tick.
  - Listener reconnects automatically after WS drop.

4) **Expose cache stats and perf guardrails**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/nft_cache.rs`
- Implement:
  - `pub struct NftCacheStats { pub hits: u64, pub misses: u64, pub invalidations: u64, pub avg_cold_ms: f64 }`
  - `pub fn snapshot_stats(&self) -> NftCacheStats`
- Acceptance criteria:
  - Stats can be read safely during concurrent load.
  - Warm hit benchmark shows <2ms p95 with 10k sequential reads on populated cache.

5) **Add tests (unit + integration)**
- File(s):
  - `/home/jack/code/zeroclaw/src/plugins/cluster/nft_cache.rs` (`#[cfg(test)]`)
  - `/home/jack/code/zeroclaw/tests/nft_cache_integration.rs`
- Implement tests:
  - TTL expiration behavior.
  - Cache warm hit path.
  - Event-driven invalidation behavior.
  - WS reconnect simulation.
- Acceptance criteria:
  - All tests pass in CI.
  - Perf assertions for warm-hit p95 <2ms are reproducible in local benchmark mode.

### Dependencies
- OperatorNFT contract deployed and ABI available with `highestTier(address)` and ERC-721 `Transfer` event.
- Local Base node HTTP + WS endpoints available.
- `alloy` and `dashmap` dependencies added to ZeroClaw `Cargo.toml`.

### Estimated effort
- ~350–500 LOC.
- 1.5–2 days for one strong Rust dev.

---
## PRD: Relayer Wallet + Gas Loop
### Gate 1 — Problem & Goal
Agents should never hold ETH for gas, or execution reliability collapses when wallets run dry. The solution is a dedicated relayer wallet service that signs/submits all `tradeFor()` calls, continuously monitors balance, alerts on low funds, and refills from fee accumulation policy. Done looks like end-to-end swap submission with only relayer gas spend and automatic low-balance remediation before service interruption.

### Gate 2 — Architecture
**Components**
- Relayer signer and provider client (alloy-based).
- Transaction builder for `SwapExecutor.tradeFor(...)` calldata.
- Submit/track engine with nonce management and receipt polling.
- Gas loop (`tokio::interval`) for balance monitor + refill trigger.
- Alert sink (structured log + webhook/Discord adapter).

**Stack**
- Rust + tokio + alloy (provider, signer, primitives, contract bindings).
- `tracing` + optional metrics hooks.

**NON-GOALS**
- No multi-relayer leader election in v1.
- No HSM/KMS signing in v1 (local key env/file only).
- No advanced gas auction strategy beyond sane EIP-1559 defaults.

### Gate 3 — Implementation Plan
1) **Create relayer config + service skeleton**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/relayer.rs`
- Implement:
  - `pub struct RelayerConfig { pub rpc_http: String, pub chain_id: u64, pub swap_executor: Address, pub relayer_private_key: String, pub min_balance_wei: U256, pub refill_target_wei: U256, pub refill_enabled: bool }`
  - `pub struct RelayerService { ... }`
  - `impl RelayerService { pub async fn new(cfg: RelayerConfig) -> anyhow::Result<Self> }`
- Acceptance criteria:
  - Service fails fast on invalid private key/address config.
  - On startup logs relayer address and current balance.

2) **Implement `tradeFor()` submission path using alloy**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/relayer.rs`
- Implement:
  - `pub struct TradeForRequest { pub amount_in: U256, pub recipient: Address, pub router: Address, pub path: Vec<Address>, pub slippage_bps: u32 }`
  - `pub struct SubmittedTx { pub tx_hash: B256, pub nonce: u64, pub submitted_at: std::time::Instant }`
  - `pub async fn submit_trade_for(&self, req: TradeForRequest) -> anyhow::Result<SubmittedTx>`
  - `async fn encode_trade_for_call(&self, req: &TradeForRequest) -> anyhow::Result<Bytes>`
  - `pub async fn wait_for_receipt(&self, tx_hash: B256, timeout: Duration) -> anyhow::Result<TransactionReceipt>`
- Acceptance criteria:
  - Successful submission returns non-zero `tx_hash`.
  - Contract revert surfaces explicit reason in error path.
  - Uses alloy only (no `ethers-rs` imports anywhere in module).

3) **Implement gas monitor loop + refill logic**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/relayer.rs`
- Implement:
  - `pub async fn start_gas_loop(self: Arc<Self>) -> tokio::task::JoinHandle<()>`
  - `async fn check_relayer_balance(&self) -> anyhow::Result<U256>`
  - `async fn maybe_refill(&self, balance: U256) -> anyhow::Result<Option<B256>>`
  - `async fn refill_relayer_wallet(&self, amount: U256) -> anyhow::Result<B256>`
- Acceptance criteria:
  - Loop runs every configured interval (default 30s).
  - Below-threshold balance emits alert and attempts refill when enabled.
  - Refill tx hash recorded and observable in logs.

4) **Add alerting + health status surface**
- File(s):
  - `/home/jack/code/zeroclaw/src/plugins/cluster/relayer.rs`
  - `/home/jack/code/zeroclaw/src/plugins/cluster/mod.rs`
- Implement:
  - `async fn emit_low_balance_alert(&self, balance: U256, threshold: U256)`
  - `pub fn health_snapshot(&self) -> RelayerHealth`
  - `pub struct RelayerHealth { pub balance_wei: U256, pub threshold_wei: U256, pub last_refill_tx: Option<B256>, pub last_error: Option<String> }`
- Acceptance criteria:
  - Health data available for cluster status endpoint.
  - At least one alert path tested (mock sink).

5) **Test coverage (unit + fork/integration optional)**
- File(s):
  - `/home/jack/code/zeroclaw/tests/relayer_submit.rs`
  - `/home/jack/code/zeroclaw/tests/relayer_gas_loop.rs`
- Acceptance criteria:
  - Submit path test validates calldata and tx request construction.
  - Gas-loop test validates threshold detection and refill trigger.
  - Integration test against Base Sepolia optional but documented.

### Dependencies
- Deployed `SwapExecutor` address + ABI.
- Relayer funded wallet private key in secure runtime env.
- RPC endpoint with sendRawTransaction support.
- Fee accumulation/refill source account policy defined.

### Estimated effort
- ~450–650 LOC.
- 2–3 days for one strong Rust dev.

---
## PRD: Keeper Background Task
### Gate 1 — Problem & Goal
Subscriptions cannot require human babysitting; renewals must happen automatically or revenue and access controls fail. The solution is a 60-second tokio keeper loop that maintains active subscriber state from contract events and calls `collectFor(agent)` for due accounts. Done looks like unattended renewal cycles, successful `SubRenewed` progression, and graceful handling/logging of `SubRenewalFailed` without task crashes.

### Gate 2 — Architecture
**Components**
- Keeper runtime task (`tokio::spawn`) with fixed 60s cadence.
- Subscriber registry (`DashSet<Address>`) populated from onchain events.
- Event reader for `Subscribed`, `SubRenewed`, and `SubRenewalFailed`.
- Collection executor calling `SubscriptionKeeper.collectFor(agent)`.
- Failure handling with categorized reasons + retry policy.

**Stack**
- Rust + tokio + alloy provider/event filters.
- `dashmap`/`dashset` for in-memory subscriber state.

**NON-GOALS**
- No distributed multi-node keeper coordination in v1.
- No offchain billing logic changes (keeper only triggers contract logic).
- No auto-removal of subscribers solely from one failed renewal event.

### Gate 3 — Implementation Plan
1) **Create keeper service + state model**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/keeper.rs`
- Implement:
  - `pub struct KeeperConfig { pub rpc_http: String, pub subscription_keeper: Address, pub poll_interval_secs: u64, pub start_block: Option<u64> }`
  - `pub struct KeeperService { ... }`
  - `pub struct KeeperCycleReport { pub attempted: usize, pub succeeded: usize, pub failed: usize }`
  - `impl KeeperService { pub async fn new(cfg: KeeperConfig) -> anyhow::Result<Self> }`
- Acceptance criteria:
  - Service starts with default 60s loop.
  - Empty subscriber set handled without errors.

2) **Hydrate and maintain active subscriber list from events**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/keeper.rs`
- Implement:
  - `pub async fn bootstrap_subscribers(&self) -> anyhow::Result<()>`
  - `pub async fn sync_subscriber_events(&self, from_block: u64, to_block: u64) -> anyhow::Result<u64>`
  - `fn upsert_subscriber(&self, agent: Address)`
- Event handling requirements:
  - Add agent on `Subscribed`.
  - Refresh liveness on `SubRenewed`.
  - Record failure reason/count on `SubRenewalFailed`.
- Acceptance criteria:
  - After bootstrap, known subscribed wallets are in memory set.
  - Event sync is idempotent when same block range replayed.

3) **Implement 60-second collection loop**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/keeper.rs`
- Implement:
  - `pub async fn start(self: Arc<Self>) -> tokio::task::JoinHandle<()>`
  - `async fn run_cycle(&self) -> anyhow::Result<KeeperCycleReport>`
  - `async fn collect_for_agent(&self, agent: Address) -> anyhow::Result<bool>`
- Collection behavior:
  - Iterate active set.
  - Call `collectFor(agent)` tx path.
  - Record per-agent result + tx hash when sent.
- Acceptance criteria:
  - Loop ticks every 60s (+/- scheduler jitter).
  - At least one cycle report emitted per tick.
  - Failed collect for one agent does not stop other agents in same cycle.

4) **Graceful `SubRenewalFailed` handling + observability**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/keeper.rs`
- Implement:
  - `fn classify_failure_reason(reason: &str) -> KeeperFailureKind`
  - `fn mark_failure(&self, agent: Address, reason: String)`
  - `pub fn snapshot_health(&self) -> KeeperHealth`
  - `pub struct KeeperHealth { pub tracked_agents: usize, pub last_cycle_at: Option<u64>, pub consecutive_failures: u32 }`
- Acceptance criteria:
  - No panic on repeated `SubRenewalFailed` events.
  - Failure counts visible via health snapshot.

5) **Tests**
- File(s):
  - `/home/jack/code/zeroclaw/tests/keeper_events.rs`
  - `/home/jack/code/zeroclaw/tests/keeper_cycle.rs`
- Acceptance criteria:
  - Event ingestion test validates subscriber registry updates.
  - Cycle test validates continued processing after one agent failure.
  - Health snapshot reflects failure increments correctly.

### Dependencies
- Deployed `SubscriptionKeeper` contract + ABI and event signatures.
- Keeper signer wallet authorized to submit `collectFor` txs (permissionless but funded).
- Relayer/tx submission utility available (or direct keeper wallet path defined).

### Estimated effort
- ~400–600 LOC.
- 2–2.5 days for one strong Rust dev.

---
## PRD: Binary Gauntlet
### Gate 1 — Problem & Goal
Genesis access must be gated to operators who can actually run infrastructure, not just fill forms. The solution is a strict 120-second challenge flow: signed EIP-191 proof, live micro-swap, proof bundle submission, and `authorizeBudget()` verification. Done looks like deterministic pass/fail decisions with auditable reasons and a durable eligibility record for minting.

### Gate 2 — Architecture
**Components**
- HTTP endpoints for apply + submit.
- Challenge/session store with TTL (`DashMap<Uuid, GauntletSession>`).
- EIP-191 verifier.
- Onchain tx verifier for micro swap hash.
- Subscription budget verifier (`authorizeBudget` success evidence via event/view).
- Eligibility registry output (wallet -> pass/fail + reason).

**Stack**
- Rust + axum + tokio + serde + alloy.
- In-memory sessions for v1; optional file/db persistence for completed decisions.

**NON-GOALS**
- No frontend UI in v1.
- No complex scoring/ranking (binary pass/fail only).
- No automatic NFT mint in v1 (only eligibility output).

### Gate 3 — Implementation Plan
1) **Create gauntlet route module and state**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/gauntlet.rs`
- Implement:
  - `pub struct GauntletConfig { pub challenge_ttl_secs: u64, pub micro_swap_min_usd: f64 }`
  - `pub struct GauntletState { sessions: DashMap<uuid::Uuid, GauntletSession>, ... }`
  - `pub fn gauntlet_router(state: GauntletState) -> axum::Router`
  - Route handlers:
    - `async fn post_apply(...) -> impl IntoResponse`
    - `async fn post_submit(...) -> impl IntoResponse`
- Acceptance criteria:
  - `POST /gauntlet/apply` returns challenge + deadline.
  - `POST /gauntlet/submit` validates same challenge session by id.

2) **Implement challenge generation + signature verification**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/gauntlet.rs`
- Implement:
  - `fn build_challenge_message(wallet: Address, session_id: Uuid, issued_at_unix: u64) -> String`
  - `fn verify_challenge_sig(wallet: Address, challenge: &str, sig_hex: &str) -> anyhow::Result<()>`
  - `fn is_session_expired(session: &GauntletSession, now_unix: u64) -> bool`
- Acceptance criteria:
  - Expired sessions (>120s) are always rejected.
  - Valid signature from expected wallet passes.
  - Signature from different wallet fails.

3) **Verify micro swap execution via cluster/onchain evidence**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/gauntlet.rs`
- Implement:
  - `async fn verify_micro_swap_tx(&self, wallet: Address, tx_hash: B256) -> anyhow::Result<SwapProof>`
  - `fn validate_swap_proof_bundle(bundle: &ProofBundle) -> anyhow::Result<()>`
  - `struct ProofBundle { tx_hash: B256, challenge_sig: String, attestation_sig: String, ... }`
- Validation requirements:
  - Tx exists and succeeded.
  - Tx maps to expected wallet-recipient relation.
  - Tx happened inside challenge window.
- Acceptance criteria:
  - Random tx hash or failed tx is rejected with explicit reason.
  - Successful micro swap inside window is accepted as swap proof.

4) **Verify `authorizeBudget()` and produce final decision**
- File: `/home/jack/code/zeroclaw/src/plugins/cluster/gauntlet.rs`
- Implement:
  - `async fn verify_budget_authorized(&self, wallet: Address) -> anyhow::Result<BudgetProof>`
  - `fn evaluate_session(session: &GauntletSession, swap: &SwapProof, budget: &BudgetProof) -> GauntletDecision`
  - `pub enum GauntletDecision { Pass { reason: String }, Fail { reason: String } }`
  - `fn persist_decision(&self, wallet: Address, decision: &GauntletDecision) -> anyhow::Result<()>`
- Acceptance criteria:
  - Pass only when all 4 required steps are true.
  - Failure reason explicitly indicates missing/invalid step.
  - Eligibility record retrievable for mint allowlist pipeline.

5) **Integration tests for full challenge lifecycle**
- File(s):
  - `/home/jack/code/zeroclaw/tests/gauntlet_flow.rs`
- Acceptance criteria:
  - Happy-path test: apply -> sign -> micro swap -> budget auth -> pass.
  - Timeout-path test: submit after 120s -> fail.
  - Tampered proof bundle test -> fail.

### Dependencies
- JSON-RPC cluster server live (for micro swap execution).
- Relayer + NFT cache modules operational.
- `SubscriptionKeeper` contract readable for budget proof.
- Contract events/receipts accessible from Base node.

### Estimated effort
- ~500–750 LOC.
- 2.5–3.5 days for one strong Rust dev.

---
## PRD: Base Sepolia Deploy + Smoke Test
### Gate 1 — Problem & Goal
Without a reproducible deployment and live smoke test, the stack is still theoretical and risky to integrate. The solution is scripted Base Sepolia deployment, relayer funding, and an end-to-end `tradeFor()` execution proving gas sponsorship + fee extraction + token delivery from a 0-ETH agent wallet. Done looks like fresh addresses written to `deployments/sepolia.json` and one verifiable successful smoke transaction on Basescan.

### Gate 2 — Architecture
**Components**
- Hardhat deployment script for all 3 contracts.
- Deployment artifact writer (`deployments/sepolia.json`).
- Relayer funding script.
- Smoke test runner script invoking cluster endpoint + chain verification.
- CI/manual checklist output for Basescan links.

**Stack**
- TypeScript + Hardhat + ethers (contracts repo side) + dotenv.
- Rust cluster server consuming emitted deployment file.

**NON-GOALS**
- No Base mainnet deploy in this PRD.
- No frontend dashboard.
- No production observability stack rollout beyond smoke logging.

### Gate 3 — Implementation Plan
1) **Add deterministic Base Sepolia deploy script**
- File(s):
  - `/home/jack/code/polyclaw-contracts/scripts/deploy-sepolia.ts`
  - `/home/jack/code/polyclaw-contracts/package.json`
- Implement:
  - `async function deploySwapExecutor(...)`
  - `async function deploySubscriptionKeeper(...)`
  - `async function deployOperatorNFT(...)`
  - `async function main()`
  - npm script: `"deploy:sepolia": "hardhat run scripts/deploy-sepolia.ts --network baseSepolia"`
- Acceptance criteria:
  - Script deploys all three contracts successfully on Base Sepolia.
  - Contract constructors use expected config values (fee recipients, stable token).

2) **Write deployment output file for downstream services**
- File: `/home/jack/code/polyclaw-contracts/deployments/sepolia.json`
- Implement in `deploy-sepolia.ts`:
  - `async function writeDeploymentJson(payload: DeploymentOutput): Promise<void>`
  - Include fields: `chainId`, `deployedAt`, `swapExecutor`, `subscriptionKeeper`, `operatorNft`, `deployer`, `txHashes`.
- Acceptance criteria:
  - File exists after deploy with non-empty checksummed addresses.
  - JSON parseable and consumable by ZeroClaw config loader.

3) **Fund test relayer wallet script**
- File: `/home/jack/code/polyclaw-contracts/scripts/fund-relayer-sepolia.ts`
- Implement:
  - `async function fundRelayer(relayer: string, amountEth: string)`
  - `async function main()`
- Acceptance criteria:
  - Relayer wallet balance after script >= configured minimum.
  - Funding tx hash printed with Basescan URL.

4) **Create end-to-end smoke test script (`tradeFor`)**
- File(s):
  - `/home/jack/code/polyclaw-contracts/scripts/smoke-tradefor-sepolia.ts`
  - `/home/jack/code/polyclaw-contracts/package.json`
- Implement:
  - `async function ensureAgentHasZeroEth(agent: string): Promise<void>`
  - `async function executeGaslessSwapViaCluster(params: SmokeSwapParams): Promise<string>`
  - `async function assertTokenArrival(agent: string, tokenOut: string, minDelta: bigint): Promise<void>`
  - `async function assertFeeExtracted(feeRecipient: string, tokenIn: string, expectedMinFee: bigint): Promise<void>`
  - npm script: `"smoke:sepolia": "hardhat run scripts/smoke-tradefor-sepolia.ts --network baseSepolia"`
- Acceptance criteria:
  - Agent wallet starts with 0 ETH and still completes swap.
  - Output tokens increase in agent wallet.
  - Fee recipient token balance increases by expected fee bound.
  - Swap tx hash resolves on Basescan as success.

5) **Document runbook and verification checklist**
- File: `/home/jack/.openclaw/workspace/projects/polyclaw/SEPOLIA-SMOKE-RUNBOOK.md`
- Include:
  - exact command order,
  - required env vars,
  - expected pass outputs,
  - rollback/retry guidance.
- Acceptance criteria:
  - Junior dev can execute runbook from clean shell and reproduce smoke pass.

### Dependencies
- Base Sepolia RPC URL + funded deployer private key.
- Token/router addresses on Base Sepolia for micro swap path.
- Cluster JSON-RPC server running against deployed addresses.
- Working relayer module in ZeroClaw.

### Estimated effort
- ~300–450 LOC scripts/docs.
- 1.5–2.5 days for one strong dev.
