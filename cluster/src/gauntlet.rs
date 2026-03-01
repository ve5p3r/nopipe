use alloy::primitives::{Address, B256, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use tracing::info;

use crate::plugins::cluster::security::verify_eip191_signature;

#[derive(Clone)]
pub struct GauntletConfig {
    pub challenge_ttl_secs: u64,
    pub base_rpc_http: String,
    pub subscription_keeper: Address,
}

#[derive(Clone, Debug)]
struct GauntletSession {
    wallet: Address,
    challenge: String,
    issued_at: u64,
    deadline: u64,
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
}

#[derive(Deserialize)]
struct ApplyRequest { wallet: String }

#[derive(Serialize)]
struct ApplyResponse { session_id: String, challenge: String, deadline_unix: u64 }

#[derive(Deserialize)]
struct SubmitRequest {
    session_id: String,
    wallet: String,
    challenge_sig: String,
    swap_tx_hash: String,
}

#[derive(Serialize)]
struct DecisionResponse { decision: String, reason: String }

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

pub fn build_challenge_message(wallet: Address, session_id: Uuid, issued_at: u64) -> String {
    format!("Polyclaw-Gauntlet\nwallet:{wallet}\nsession:{session_id}\nissued:{issued_at}")
}

async fn verify_swap_tx(rpc_http: &str, tx_hash: B256, window_start: u64, window_end: u64) -> anyhow::Result<()> {
    let provider = ProviderBuilder::new().connect_http(rpc_http.parse().unwrap());
    let receipt = provider.get_transaction_receipt(tx_hash).await?
        .ok_or_else(|| anyhow::anyhow!("Receipt not found: {tx_hash}"))?;
    if !receipt.status() { return Err(anyhow::anyhow!("Tx reverted")); }
    if let Some(block_num) = receipt.block_number {
        let block = provider.get_block_by_number(block_num.into()).await?
            .ok_or_else(|| anyhow::anyhow!("Block not found"))?;
        let ts = block.header.timestamp;
        if ts < window_start || ts > window_end {
            return Err(anyhow::anyhow!("Swap tx outside challenge window"));
        }
    }
    Ok(())
}

async fn verify_budget_authorized(rpc_http: &str, keeper: Address, wallet: Address) -> anyhow::Result<()> {
    let provider = ProviderBuilder::new().connect_http(rpc_http.parse().unwrap());
    let selector = &alloy::primitives::keccak256(b"getBudget(address)")[..4];
    let mut addr = [0u8; 32];
    addr[12..].copy_from_slice(wallet.as_slice());
    let mut calldata = selector.to_vec();
    calldata.extend_from_slice(&addr);
    let result = provider.call(
        TransactionRequest::default()
            .to(keeper)
            .input(alloy::primitives::Bytes::from(calldata).into()),
    ).await?;
    if result.len() < 32 { return Err(anyhow::anyhow!("getBudget returned empty")); }
    let max_per_cycle = U256::from_be_slice(&result[..32]);
    if max_per_cycle.is_zero() { return Err(anyhow::anyhow!("authorizeBudget not called")); }
    Ok(())
}

fn fail_response(reason: &str) -> axum::response::Response {
    Json(DecisionResponse { decision: "Fail".into(), reason: reason.into() }).into_response()
}

async fn post_apply(State(state): State<GauntletState>, Json(req): Json<ApplyRequest>) -> impl IntoResponse {
    let wallet: Address = match req.wallet.parse() {
        Ok(a) => a,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"invalid wallet"}))).into_response(),
    };
    let session_id = Uuid::new_v4();
    let now = now_unix();
    let deadline = now + state.config.challenge_ttl_secs;
    let challenge = build_challenge_message(wallet, session_id, now);
    state.sessions.insert(session_id, GauntletSession { wallet, challenge: challenge.clone(), issued_at: now, deadline });
    info!("Gauntlet: issued challenge for {wallet}, deadline {deadline}");
    Json(ApplyResponse { session_id: session_id.to_string(), challenge, deadline_unix: deadline }).into_response()
}

async fn post_submit(State(state): State<GauntletState>, Json(req): Json<SubmitRequest>) -> impl IntoResponse {
    let session_id: Uuid = match req.session_id.parse() { Ok(id) => id, Err(_) => return fail_response("Invalid session_id") };
    let wallet: Address = match req.wallet.parse() { Ok(a) => a, Err(_) => return fail_response("Invalid wallet") };
    let session = match state.sessions.get(&session_id) { Some(s) => s.clone(), None => return fail_response("Session not found") };

    if now_unix() > session.deadline {
        state.persist_decision(wallet, GauntletDecision::Fail { reason: "Expired".into() });
        return fail_response("Challenge expired (>120s)");
    }
    if session.wallet != wallet { return fail_response("Wallet mismatch"); }

    if let Err(e) = verify_eip191_signature(&session.challenge, &req.challenge_sig, wallet) {
        state.persist_decision(wallet, GauntletDecision::Fail { reason: format!("Bad sig: {e}") });
        return fail_response(&format!("Invalid challenge signature: {e}"));
    }

    let tx_hash: B256 = match req.swap_tx_hash.parse() { Ok(h) => h, Err(_) => return fail_response("Invalid swap_tx_hash") };
    if let Err(e) = verify_swap_tx(&state.config.base_rpc_http, tx_hash, session.issued_at, session.deadline).await {
        state.persist_decision(wallet, GauntletDecision::Fail { reason: format!("Swap: {e}") });
        return fail_response(&format!("Swap tx invalid: {e}"));
    }

    if let Err(e) = verify_budget_authorized(&state.config.base_rpc_http, state.config.subscription_keeper, wallet).await {
        state.persist_decision(wallet, GauntletDecision::Fail { reason: format!("Budget: {e}") });
        return fail_response(&format!("Budget not authorized: {e}"));
    }

    state.persist_decision(wallet, GauntletDecision::Pass { reason: "All steps passed".into() });
    state.sessions.remove(&session_id);
    info!("Gauntlet: PASS for {wallet}");
    Json(DecisionResponse { decision: "Pass".into(), reason: "All steps validated".into() }).into_response()
}

impl GauntletState {
    pub fn new(config: GauntletConfig) -> Self {
        Self { config, sessions: Arc::new(DashMap::new()), decisions: Arc::new(DashMap::new()) }
    }
    fn persist_decision(&self, wallet: Address, decision: GauntletDecision) {
        self.decisions.insert(wallet, decision);
    }
    pub fn get_decision(&self, wallet: Address) -> Option<GauntletDecision> {
        self.decisions.get(&wallet).map(|d| d.clone())
    }
}

pub fn gauntlet_router(state: GauntletState) -> Router {
    Router::new()
        .route("/gauntlet/apply", post(post_apply))
        .route("/gauntlet/submit", post(post_submit))
        .with_state(state)
}
