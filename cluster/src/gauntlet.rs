use alloy::primitives::{Address, B256, U256};
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use dashmap::DashMap;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

use crate::relayer::rpc_with_fallback;
use crate::security::verify_eip191_signature;

/// Mint costs in wei per tier (Operator=1, Pro=2, Enterprise=3)
pub const TIER_MINT_COST: [(u8, u128); 3] = [
    (1, 250_000_000_000_000_000),   // 0.25 ETH — Operator
    (2, 1_000_000_000_000_000_000), // 1.00 ETH — Pro
    (3, 5_000_000_000_000_000_000), // 5.00 ETH — Enterprise
];

#[derive(Clone)]
pub struct GauntletConfig {
    pub challenge_ttl_secs: u64,
    pub base_rpc_http: String,
    pub base_rpc_http_fallback: Option<String>,
    pub subscription_keeper: Address,
    pub fee_recipient: Address,
    pub chain_id: u64,
    pub db_path: String,
    pub telegram_bot_token: Option<String>,
    pub telegram_ops_chat_id: Option<i64>,
    pub genesis_mode: bool,
}

#[derive(Clone, Debug)]
struct GauntletSession {
    session_id: Uuid,
    wallet: Address,
    challenge: String,
    issued_at: u64,
    deadline: u64,
    tier: u8,
    mint_cost_wei: u128,
    entry_price_wei: u128,
    position_size_wei: u128,
    timestamp: u64,
}

#[derive(Debug, Clone, Serialize)]
pub enum GauntletDecision {
    Pass { reason: String },
    Fail { reason: String },
}

#[derive(Clone)]
pub struct GauntletState {
    config: GauntletConfig,
    sessions: Arc<DashMap<Uuid, GauntletSession>>,
    decisions: Arc<DashMap<Address, GauntletDecision>>,
    sanctioned_evm_addresses: Arc<RwLock<HashSet<Address>>>,
}

#[derive(Deserialize)]
struct ApplyRequest {
    wallet: String,
    #[serde(default = "default_tier")]
    tier: u8,
}

fn default_tier() -> u8 {
    1
}

#[derive(Serialize)]
struct PaymentDetails {
    recipient: String,
    amount_eth: String,
    amount_wei: String,
    chain_id: u64,
}

#[derive(Serialize)]
struct ApplyResponse {
    session_id: String,
    challenge: String,
    deadline_unix: u64,
    tier: u8,
    payment: PaymentDetails,
}

#[derive(Deserialize)]
struct SubmitRequest {
    session_id: String,
    wallet: String,
    challenge_sig: String,
    #[serde(alias = "swap_tx_hash")]
    tx_hash: String,
}

#[derive(Serialize)]
struct DecisionResponse {
    decision: String,
    reason: String,
    next_steps: String,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn build_challenge_message(wallet: Address, session_id: Uuid, issued_at: u64) -> String {
    format!("Nopipe-Gauntlet\nwallet:{wallet}\nsession:{session_id}\nissued:{issued_at}")
}

async fn verify_eth_payment(
    rpc_http: &str,
    fallback_rpc: Option<String>,
    tx_hash: B256,
    expected_to: Address,
    min_value_wei: u128,
    window_start: u64,
    window_end: u64,
) -> anyhow::Result<()> {
    let tx_hex = format!("{tx_hash:#x}");

    // eth_getTransactionByHash
    let resp = rpc_with_fallback(
        rpc_http,
        fallback_rpc.as_deref(),
        serde_json::json!({
            "jsonrpc": "2.0", "id": 1,
            "method": "eth_getTransactionByHash",
            "params": [tx_hex]
        }),
    )
    .await?;

    let tx = resp["result"]
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Transaction not found: {tx_hash}"))?;

    // Verify recipient
    let to_str = tx
        .get("to")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No 'to' field in transaction"))?;
    let to: Address = to_str
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid 'to' address: {to_str}"))?;
    if to.to_checksum(None).to_lowercase() != expected_to.to_checksum(None).to_lowercase() {
        return Err(anyhow::anyhow!(
            "Payment to wrong address: got {to}, expected {expected_to}"
        ));
    }

    // Verify value
    let value_hex = tx.get("value").and_then(|v| v.as_str()).unwrap_or("0x0");
    let value = u128::from_str_radix(value_hex.trim_start_matches("0x"), 16).unwrap_or(0);
    if value < min_value_wei {
        return Err(anyhow::anyhow!(
            "Insufficient payment: got {value} wei, need {min_value_wei} wei"
        ));
    }

    // eth_getTransactionReceipt — check status + block timestamp
    let rcpt_resp = rpc_with_fallback(
        rpc_http,
        fallback_rpc.as_deref(),
        serde_json::json!({
            "jsonrpc": "2.0", "id": 2,
            "method": "eth_getTransactionReceipt",
            "params": [tx_hex]
        }),
    )
    .await?;

    let rcpt = rcpt_resp["result"]
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Receipt not found"))?;

    let status_hex = rcpt.get("status").and_then(|v| v.as_str()).unwrap_or("0x0");
    if status_hex == "0x0" {
        return Err(anyhow::anyhow!("Transaction reverted"));
    }

    let block_hex = rcpt
        .get("blockNumber")
        .and_then(|v| v.as_str())
        .unwrap_or("0x0");
    let block_num = u64::from_str_radix(block_hex.trim_start_matches("0x"), 16).unwrap_or(0);

    // eth_getBlockByNumber for timestamp
    let blk_resp = rpc_with_fallback(
        rpc_http,
        fallback_rpc.as_deref(),
        serde_json::json!({
            "jsonrpc": "2.0", "id": 3,
            "method": "eth_getBlockByNumber",
            "params": [format!("0x{block_num:x}"), false]
        }),
    )
    .await?;

    let blk = blk_resp["result"]
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Block not found"))?;
    let ts_hex = blk
        .get("timestamp")
        .and_then(|v| v.as_str())
        .unwrap_or("0x0");
    let ts = u64::from_str_radix(ts_hex.trim_start_matches("0x"), 16).unwrap_or(0);

    if ts < window_start || ts > window_end {
        return Err(anyhow::anyhow!(
            "Payment outside challenge window (block ts={ts}, window={window_start}-{window_end})"
        ));
    }

    Ok(())
}

async fn verify_budget_authorized(
    rpc_http: &str,
    fallback_rpc: Option<String>,
    keeper: Address,
    wallet: Address,
) -> anyhow::Result<()> {
    let selector = &alloy::primitives::keccak256(b"getBudget(address)")[..4];
    let mut addr = [0u8; 32];
    addr[12..].copy_from_slice(wallet.as_slice());
    let mut calldata = selector.to_vec();
    calldata.extend_from_slice(&addr);
    let call_result = rpc_with_fallback(
        rpc_http,
        fallback_rpc.as_deref(),
        serde_json::json!({
            "jsonrpc": "2.0", "id": 4,
            "method": "eth_call",
            "params": [
                {
                    "to": keeper.to_string(),
                    "data": format!("0x{}", hex::encode(&calldata))
                },
                "latest"
            ]
        }),
    )
    .await?;
    let result_hex = call_result["result"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("eth_call missing result"))?;
    let result_hex = result_hex.trim_start_matches("0x");
    let result = hex::decode(result_hex).map_err(|e| anyhow::anyhow!("Invalid eth_call hex: {e}"))?;
    if result.len() < 32 {
        return Err(anyhow::anyhow!("getBudget returned empty"));
    }
    let max_per_cycle = U256::from_be_slice(&result[..32]);
    if max_per_cycle.is_zero() {
        return Err(anyhow::anyhow!("authorizeBudget not called"));
    }
    Ok(())
}

fn fail_response(reason: &str) -> axum::response::Response {
    Json(DecisionResponse {
        decision: "Fail".into(),
        reason: reason.into(),
        next_steps: "Resolve the issue above and retry the Gauntlet before your session expires."
            .into(),
    })
    .into_response()
}


fn tier_name(tier: u8) -> &'static str {
    match tier {
        3 => "Enterprise",
        2 => "Pro",
        _ => "Operator",
    }
}

async fn post_apply(
    State(state): State<GauntletState>,
    Json(req): Json<ApplyRequest>,
) -> impl IntoResponse {
    let wallet: Address = match req.wallet.parse() {
        Ok(a) => a,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error":"invalid wallet"})),
            )
                .into_response()
        }
    };

    if state
        .sanctioned_evm_addresses
        .read()
        .await
        .contains(&wallet)
    {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "sanctioned_wallet",
                "message": "This wallet is blocked by sanctions screening."
            })),
        )
            .into_response();
    }

    // Rate limit: 1 active session per wallet
    let now = now_unix();
    let has_active = state
        .sessions
        .iter()
        .any(|e| e.wallet == wallet && e.deadline > now);
    if has_active {
        return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({
            "error": "rate_limited",
            "message": "Active session already exists for this wallet. Wait for it to expire or submit."
        }))).into_response();
    }

    let tier = if req.tier >= 1 && req.tier <= 3 {
        req.tier
    } else {
        1
    };
    let mint_cost_wei = GauntletState::tier_cost_wei(tier);
    let amount_eth = format!("{:.2}", mint_cost_wei as f64 / 1e18);

    let session_id = Uuid::new_v4();
    let now = now_unix();
    let deadline = now + state.config.challenge_ttl_secs;
    let challenge = build_challenge_message(wallet, session_id, now);
    state.sessions.insert(
        session_id,
        GauntletSession {
            session_id,
            wallet,
            challenge: challenge.clone(),
            issued_at: now,
            deadline,
            tier,
            mint_cost_wei,
            entry_price_wei: mint_cost_wei,
            position_size_wei: 1,
            timestamp: now,
        },
    );
    if let Err(e) = state.persist_session(session_id) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "session_persist_failed",
                "message": format!("{e}")
            })),
        )
            .into_response();
    }
    info!("Gauntlet: issued challenge for {wallet} tier={tier} cost={amount_eth}ETH deadline={deadline}");
    if let (Some(token), Some(chat_id)) = (
        &state.config.telegram_bot_token,
        state.config.telegram_ops_chat_id,
    ) {
        let msg = format!(
            "🎯 <b>Gauntlet APPLY</b>\n\nWallet: <code>{wallet}</code>\nTier: {} ({amount_eth} ETH)\nSession: <code>{session_id}</code>\nDeadline: +{}s\nTime: {}",
            tier_name(tier),
            state.config.challenge_ttl_secs,
            chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
        );
        crate::telegram::notify_ops(token, chat_id, &msg).await;
    }
    Json(ApplyResponse {
        session_id: session_id.to_string(),
        challenge,
        deadline_unix: deadline,
        tier,
        payment: PaymentDetails {
            recipient: state.config.fee_recipient.to_string(),
            amount_eth,
            amount_wei: mint_cost_wei.to_string(),
            chain_id: state.config.chain_id,
        },
    })
    .into_response()
}

async fn post_submit(
    State(state): State<GauntletState>,
    Json(req): Json<SubmitRequest>,
) -> impl IntoResponse {
    let session_id: Uuid = match req.session_id.parse() {
        Ok(id) => id,
        Err(_) => return fail_response("Invalid session_id"),
    };
    let wallet: Address = match req.wallet.parse() {
        Ok(a) => a,
        Err(_) => return fail_response("Invalid wallet"),
    };
    let session = match state.sessions.get(&session_id) {
        Some(s) => s.clone(),
        None => return fail_response("Session not found"),
    };

    if now_unix() > session.deadline {
        state.persist_decision(
            wallet,
            GauntletDecision::Fail {
                reason: "Expired".into(),
            },
        );
        state.remove_session(&session_id);
        if let (Some(token), Some(chat_id)) = (
            &state.config.telegram_bot_token,
            state.config.telegram_ops_chat_id,
        ) {
            let msg = format!(
                "⏱️ <b>Gauntlet TIMEOUT</b>\n\nWallet: <code>{wallet}</code>\nTier: {}\nTime: {}",
                tier_name(session.tier),
                chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
            );
            crate::telegram::notify_ops(token, chat_id, &msg).await;
        }
        return fail_response("Challenge expired (>120s)");
    }
    if session.wallet != wallet {
        return fail_response("Wallet mismatch");
    }

    if let Err(e) = verify_eip191_signature(&session.challenge, &req.challenge_sig, wallet) {
        state.persist_decision(
            wallet,
            GauntletDecision::Fail {
                reason: format!("Bad sig: {e}"),
            },
        );
        if let (Some(token), Some(chat_id)) = (
            &state.config.telegram_bot_token,
            state.config.telegram_ops_chat_id,
        ) {
            let msg = format!(
                "❌ <b>Gauntlet FAIL</b> — bad sig\n\nWallet: <code>{wallet}</code>\nTier: {}\nReason: {e}\nTime: {}",
                tier_name(session.tier),
                chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
            );
            crate::telegram::notify_ops(token, chat_id, &msg).await;
        }
        return fail_response(&format!("Invalid challenge signature: {e}"));
    }

    let tx_hash: B256 = match req.tx_hash.parse() {
        Ok(h) => h,
        Err(_) => return fail_response("Invalid tx_hash"),
    };
    if let Err(e) = verify_eth_payment(
        &state.config.base_rpc_http,
        state.config.base_rpc_http_fallback.clone(),
        tx_hash,
        state.config.fee_recipient,
        session.mint_cost_wei,
        session.issued_at,
        session.deadline,
    )
    .await
    {
        state.persist_decision(
            wallet,
            GauntletDecision::Fail {
                reason: format!("Payment: {e}"),
            },
        );
        if let (Some(token), Some(chat_id)) = (
            &state.config.telegram_bot_token,
            state.config.telegram_ops_chat_id,
        ) {
            let msg = format!(
                "❌ <b>Gauntlet FAIL</b> — payment\n\nWallet: <code>{wallet}</code>\nTier: {}\nReason: {e}\nTime: {}",
                tier_name(session.tier),
                chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
            );
            crate::telegram::notify_ops(token, chat_id, &msg).await;
        }
        return fail_response(&format!("Payment invalid: {e}"));
    }

    // In genesis mode, skip budget authorization (contracts not yet deployed)
    if !state.config.genesis_mode {
        if let Err(e) = verify_budget_authorized(
            &state.config.base_rpc_http,
            state.config.base_rpc_http_fallback.clone(),
            state.config.subscription_keeper,
            wallet,
        )
        .await
        {
            state.persist_decision(
                wallet,
                GauntletDecision::Fail {
                    reason: format!("Budget: {e}"),
                },
            );
            if let (Some(token), Some(chat_id)) = (
                &state.config.telegram_bot_token,
                state.config.telegram_ops_chat_id,
            ) {
                let msg = format!(
                    "❌ <b>Gauntlet FAIL</b> — budget\n\nWallet: <code>{wallet}</code>\nTier: {}\nReason: {e}\nTime: {}",
                    tier_name(session.tier),
                    chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
                );
                crate::telegram::notify_ops(token, chat_id, &msg).await;
            }
            return fail_response(&format!("Budget not authorized: {e}"));
        }
    } else {
        tracing::info!("Genesis mode: skipping budget check for {wallet}");
    }

    state.persist_decision(
        wallet,
        GauntletDecision::Pass {
            reason: "All steps passed".into(),
        },
    );
    if let (Some(token), Some(chat_id)) = (
        &state.config.telegram_bot_token,
        state.config.telegram_ops_chat_id,
    ) {
        let tier_label = tier_name(session.tier);
        let msg = format!(
            "✅ <b>Gauntlet PASS</b>\n\nWallet: <code>{wallet}</code>\nTier: {tier_label} ({})\nTx: <code>{tx_hash}</code>\nTime: {}\n\n<b>Mint action:</b>\n<code>/mint {wallet} {}</code>",
            format!("{:.2} ETH", session.mint_cost_wei as f64 / 1e18),
            chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"),
            session.tier,
        );
        crate::telegram::notify_ops(token, chat_id, &msg).await;
    }
    state.remove_session(&session_id);
    info!("Gauntlet: PASS for {wallet}");
    Json(DecisionResponse {
        decision: "Pass".into(),
        reason: "All steps validated".into(),
        next_steps: "Gauntlet passed. Your wallet has been recorded. The Nopipe team will mint your OperatorNFT and reach out via @NoPipeBot on Telegram within 24 hours.".into(),
    })
    .into_response()
}

impl GauntletState {
    pub fn new(
        config: GauntletConfig,
        sanctioned_evm_addresses: Arc<RwLock<HashSet<Address>>>,
    ) -> Self {
        let state = Self {
            config,
            sessions: Arc::new(DashMap::new()),
            decisions: Arc::new(DashMap::new()),
            sanctioned_evm_addresses,
        };
        if let Err(e) = state.init_db() {
            tracing::error!("Gauntlet DB init failed: {e}");
        }
        if let Err(e) = state.load_sessions_from_db() {
            tracing::error!("Gauntlet DB restore failed: {e}");
        }
        state
    }

    pub fn tier_cost_wei(tier: u8) -> u128 {
        TIER_MINT_COST
            .iter()
            .find(|(t, _)| *t == tier)
            .map(|(_, cost)| *cost)
            .unwrap_or(TIER_MINT_COST[0].1)
    }
    fn persist_decision(&self, wallet: Address, decision: GauntletDecision) {
        self.decisions.insert(wallet, decision);
    }
    pub fn get_decision(&self, wallet: Address) -> Option<GauntletDecision> {
        self.decisions.get(&wallet).map(|d| d.clone())
    }

    fn init_db(&self) -> anyhow::Result<()> {
        let conn = Connection::open(&self.config.db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS gauntlet_sessions (
                session_id TEXT PRIMARY KEY,
                wallet TEXT NOT NULL,
                challenge TEXT NOT NULL,
                issued_at INTEGER NOT NULL,
                deadline INTEGER NOT NULL,
                tier INTEGER NOT NULL,
                mint_cost_wei TEXT NOT NULL,
                entry_price_wei TEXT NOT NULL,
                position_size_wei TEXT NOT NULL,
                timestamp INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS pending_nonces (
                wallet TEXT PRIMARY KEY,
                nonce INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );",
        )?;
        Ok(())
    }

    fn load_sessions_from_db(&self) -> anyhow::Result<()> {
        let conn = Connection::open(&self.config.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT session_id, wallet, challenge, issued_at, deadline, tier, mint_cost_wei, entry_price_wei, position_size_wei, timestamp
             FROM gauntlet_sessions"
        )?;
        let rows = stmt.query_map([], |row| {
            let session_id_str: String = row.get(0)?;
            let wallet_str: String = row.get(1)?;
            let tier_i64: i64 = row.get(5)?;
            Ok((
                session_id_str,
                wallet_str,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                tier_i64,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, i64>(9)?,
            ))
        })?;

        let now = now_unix();
        for row in rows {
            let (
                session_id_str,
                wallet_str,
                challenge,
                issued_at_i64,
                deadline_i64,
                tier_i64,
                mint_cost_wei_str,
                entry_price_wei_str,
                position_size_wei_str,
                timestamp_i64,
            ) = row?;

            let session_id: Uuid = match session_id_str.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let wallet: Address = match wallet_str.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let issued_at = match u64::try_from(issued_at_i64) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let deadline = match u64::try_from(deadline_i64) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let tier = match u8::try_from(tier_i64) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let mint_cost_wei: u128 = match mint_cost_wei_str.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let entry_price_wei: u128 = match entry_price_wei_str.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let position_size_wei: u128 = match position_size_wei_str.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let timestamp = match u64::try_from(timestamp_i64) {
                Ok(v) => v,
                Err(_) => continue,
            };

            if deadline <= now {
                let _ = conn.execute(
                    "DELETE FROM gauntlet_sessions WHERE session_id = ?1",
                    params![session_id.to_string()],
                );
                continue;
            }

            self.sessions.insert(
                session_id,
                GauntletSession {
                    session_id,
                    wallet,
                    challenge,
                    issued_at,
                    deadline,
                    tier,
                    mint_cost_wei,
                    entry_price_wei,
                    position_size_wei,
                    timestamp,
                },
            );
        }
        Ok(())
    }

    fn persist_session(&self, session_id: Uuid) -> anyhow::Result<()> {
        let Some(session) = self.sessions.get(&session_id) else {
            return Ok(());
        };
        let conn = Connection::open(&self.config.db_path)?;
        conn.execute(
            "INSERT INTO gauntlet_sessions
             (session_id, wallet, challenge, issued_at, deadline, tier, mint_cost_wei, entry_price_wei, position_size_wei, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(session_id) DO UPDATE SET
                wallet = excluded.wallet,
                challenge = excluded.challenge,
                issued_at = excluded.issued_at,
                deadline = excluded.deadline,
                tier = excluded.tier,
                mint_cost_wei = excluded.mint_cost_wei,
                entry_price_wei = excluded.entry_price_wei,
                position_size_wei = excluded.position_size_wei,
                timestamp = excluded.timestamp",
            params![
                session.session_id.to_string(),
                session.wallet.to_string(),
                session.challenge.clone(),
                i64::try_from(session.issued_at)?,
                i64::try_from(session.deadline)?,
                i64::from(session.tier),
                session.mint_cost_wei.to_string(),
                session.entry_price_wei.to_string(),
                session.position_size_wei.to_string(),
                i64::try_from(session.timestamp)?,
            ],
        )?;
        Ok(())
    }

    fn remove_session(&self, session_id: &Uuid) {
        self.sessions.remove(session_id);
        if let Ok(conn) = Connection::open(&self.config.db_path) {
            let _ = conn.execute(
                "DELETE FROM gauntlet_sessions WHERE session_id = ?1",
                params![session_id.to_string()],
            );
        }
    }
}

pub fn gauntlet_router(state: GauntletState) -> Router {
    Router::new()
        .route("/gauntlet/apply", post(post_apply))
        .route("/gauntlet/submit", post(post_submit))
        .with_state(state)
}
