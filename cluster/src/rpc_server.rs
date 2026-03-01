use alloy::primitives::{Address, U256};
use axum::{extract::State, response::IntoResponse, routing::post, Json, Router};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tracing::info;

use super::{
    ClusterConfig,
    nft_cache::NftVerificationCache,
    relayer::{RelayerService, TradeForRequest},
    keeper::KeeperService,
    security::{NonceStore, verify_eip191_signature, build_eip191_message},
};

#[derive(Deserialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
    pub id: Value,
}

#[derive(Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcErrorObject>,
    pub id: Value,
}

#[derive(Serialize, Clone, Debug)]
pub struct RpcErrorObject {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

const ERR_INVALID_REQUEST: i64 = -32600;
const ERR_METHOD_NOT_FOUND: i64 = -32601;
const ERR_INVALID_PARAMS: i64   = -32602;
const ERR_INTERNAL: i64         = -32603;
const ERR_ACCESS_DENIED: i64    = -32001;
const ERR_AUTH_FAILED: i64      = -32002;
const ERR_NONCE_REPLAY: i64     = -32003;

#[derive(Clone, Debug, Serialize)]
pub enum SwapStatus {
    Pending,
    Submitted { tx_hash: String },
    Confirmed { tx_hash: String, block: u64 },
    Failed { reason: String },
}

#[derive(Clone)]
pub struct ClusterAppState {
    pub nft_cache: Arc<NftVerificationCache>,
    pub relayer: Arc<RelayerService>,
    pub keeper: Arc<KeeperService>,
    pub config: ClusterConfig,
    pub nonce_store: Arc<NonceStore>,
    pub swap_statuses: Arc<DashMap<String, SwapStatus>>,
}

#[derive(Deserialize)]
struct SwapExecuteParams {
    wallet: String, token_in: String, token_out: String,
    amount_in: String, router: String, slippage_bps: u32,
    nonce: String, sig: String,
}

#[derive(Deserialize)]
struct SwapQuoteParams { token_in: String, token_out: String, amount_in: String, router: String }

#[derive(Deserialize)]
struct AgentRegisterParams { wallet: String, nonce: String, sig: String, metadata: Option<Value> }

#[derive(Deserialize)]
struct SwapStatusParams { request_id: String }

pub async fn serve(bind_addr: String, state: ClusterAppState) -> anyhow::Result<()> {
    let app = build_cluster_router(state);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    info!("Polyclaw cluster RPC listening on {bind_addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

pub fn build_cluster_router(state: ClusterAppState) -> Router {
    Router::new().route("/rpc", post(handle_rpc)).with_state(state)
}

async fn handle_rpc(State(state): State<ClusterAppState>, Json(req): Json<JsonRpcRequest>) -> impl IntoResponse {
    if req.jsonrpc != "2.0" {
        return Json(JsonRpcResponse {
            jsonrpc: "2.0", result: None,
            error: Some(rpc_err(ERR_INVALID_REQUEST, "jsonrpc must be '2.0'")),
            id: req.id,
        });
    }
    match dispatch_method(&state, &req).await {
        Ok(result) => Json(JsonRpcResponse { jsonrpc: "2.0", result: Some(result), error: None, id: req.id }),
        Err(e)     => Json(JsonRpcResponse { jsonrpc: "2.0", result: None, error: Some(e), id: req.id }),
    }
}

async fn dispatch_method(state: &ClusterAppState, req: &JsonRpcRequest) -> Result<Value, RpcErrorObject> {
    match req.method.as_str() {
        "swap_execute"   => handle_swap_execute(state, &req.params).await,
        "swap_quote"     => handle_swap_quote(state, &req.params).await,
        "agent_register" => handle_agent_register(state, &req.params).await,
        "swap_status"    => handle_swap_status(state, &req.params).await,
        m => Err(rpc_err(ERR_METHOD_NOT_FOUND, format!("Method '{m}' not found"))),
    }
}

fn parse_params<T: for<'de> Deserialize<'de>>(params: &Value) -> Result<T, RpcErrorObject> {
    let p = if params.is_array() {
        params.as_array().and_then(|a| a.first()).cloned().unwrap_or(Value::Null)
    } else { params.clone() };
    serde_json::from_value(p).map_err(|e| rpc_err(ERR_INVALID_PARAMS, format!("Invalid params: {e}")))
}

fn rpc_err(code: i64, msg: impl Into<String>) -> RpcErrorObject {
    RpcErrorObject { code, message: msg.into(), data: None }
}

async fn handle_swap_execute(state: &ClusterAppState, params: &Value) -> Result<Value, RpcErrorObject> {
    let p: SwapExecuteParams = parse_params(params)?;
    let wallet: Address = p.wallet.parse().map_err(|_| rpc_err(ERR_INVALID_PARAMS, "Invalid wallet"))?;
    let token_in: Address = p.token_in.parse().map_err(|_| rpc_err(ERR_INVALID_PARAMS, "Invalid token_in"))?;
    let token_out: Address = p.token_out.parse().map_err(|_| rpc_err(ERR_INVALID_PARAMS, "Invalid token_out"))?;
    let router: Address = p.router.parse().map_err(|_| rpc_err(ERR_INVALID_PARAMS, "Invalid router"))?;
    let amount_in: U256 = p.amount_in.parse().map_err(|_| rpc_err(ERR_INVALID_PARAMS, "Invalid amount_in"))?;

    if !state.nonce_store.consume_nonce(wallet, &p.nonce) {
        return Err(rpc_err(ERR_NONCE_REPLAY, "Nonce already used"));
    }

    let payload_hash = alloy::primitives::keccak256(
        format!("{wallet}{token_in}{token_out}{amount_in}{router}{}{}", p.slippage_bps, p.nonce)
    );
    let message = build_eip191_message("swap_execute", wallet, &p.nonce, payload_hash);
    verify_eip191_signature(&message, &p.sig, wallet)
        .map_err(|e| rpc_err(ERR_AUTH_FAILED, format!("Sig invalid: {e}")))?;

    let tier = state.nft_cache.get_tier(wallet).await
        .map_err(|e| rpc_err(ERR_INTERNAL, format!("NFT cache: {e}")))?;
    if tier < state.config.min_swap_tier {
        return Err(rpc_err(ERR_ACCESS_DENIED, format!(
            "NFT_ACCESS_DENIED: tier {tier} < required {}", state.config.min_swap_tier
        )));
    }

    let request_id = uuid::Uuid::new_v4().to_string();
    state.swap_statuses.insert(request_id.clone(), SwapStatus::Pending);

    let submitted = state.relayer.submit_trade_for(TradeForRequest {
        amount_in, recipient: wallet, router,
        path: vec![token_in, token_out],
        slippage_bps: p.slippage_bps,
    }).await.map_err(|e| {
        state.swap_statuses.insert(request_id.clone(), SwapStatus::Failed { reason: e.to_string() });
        rpc_err(ERR_INTERNAL, format!("Swap failed: {e}"))
    })?;

    let tx_hash = format!("{:?}", submitted.tx_hash);
    state.swap_statuses.insert(request_id.clone(), SwapStatus::Submitted { tx_hash: tx_hash.clone() });
    info!("swap_execute: {wallet} → {tx_hash} (req {request_id})");

    Ok(serde_json::json!({ "request_id": request_id, "tx_hash": tx_hash, "status": "pending" }))
}

async fn handle_swap_quote(_state: &ClusterAppState, params: &Value) -> Result<Value, RpcErrorObject> {
    let p: SwapQuoteParams = parse_params(params)?;
    let _: Address = p.token_in.parse().map_err(|_| rpc_err(ERR_INVALID_PARAMS, "Invalid token_in"))?;
    let _: Address = p.token_out.parse().map_err(|_| rpc_err(ERR_INVALID_PARAMS, "Invalid token_out"))?;
    let _: Address = p.router.parse().map_err(|_| rpc_err(ERR_INVALID_PARAMS, "Invalid router"))?;
    let amount_in: U256 = p.amount_in.parse().map_err(|_| rpc_err(ERR_INVALID_PARAMS, "Invalid amount_in"))?;
    let fee = amount_in / U256::from(1000u64);
    let amount_out = amount_in - fee;
    Ok(serde_json::json!({ "amount_in": amount_in.to_string(), "amount_out_estimate": amount_out.to_string(), "fee": fee.to_string(), "fee_bps": 10 }))
}

async fn handle_agent_register(state: &ClusterAppState, params: &Value) -> Result<Value, RpcErrorObject> {
    let p: AgentRegisterParams = parse_params(params)?;
    let wallet: Address = p.wallet.parse().map_err(|_| rpc_err(ERR_INVALID_PARAMS, "Invalid wallet"))?;
    if !state.nonce_store.consume_nonce(wallet, &p.nonce) {
        return Err(rpc_err(ERR_NONCE_REPLAY, "Nonce already used"));
    }
    let payload_hash = alloy::primitives::keccak256(format!("{wallet}register{}", p.nonce));
    let message = build_eip191_message("agent_register", wallet, &p.nonce, payload_hash);
    verify_eip191_signature(&message, &p.sig, wallet)
        .map_err(|e| rpc_err(ERR_AUTH_FAILED, format!("Sig invalid: {e}")))?;
    let tier = state.nft_cache.get_tier(wallet).await
        .map_err(|e| rpc_err(ERR_INTERNAL, format!("NFT cache: {e}")))?;
    Ok(serde_json::json!({
        "wallet": wallet.to_string(), "tier": tier,
        "swap_contract": state.config.swap_executor.to_string(),
        "subscription_keeper": state.config.subscription_keeper.to_string(),
        "operator_nft": state.config.operator_nft.to_string(),
        "supported_tokens": ["WETH","USDC","BNKR","VIRTUAL","AERO","BRETT"]
    }))
}

async fn handle_swap_status(state: &ClusterAppState, params: &Value) -> Result<Value, RpcErrorObject> {
    let p: SwapStatusParams = parse_params(params)?;
    match state.swap_statuses.get(&p.request_id) {
        Some(s) => Ok(serde_json::json!({ "request_id": p.request_id, "status": format!("{:?}", *s) })),
        None => Err(rpc_err(ERR_INVALID_PARAMS, format!("Unknown request_id: {}", p.request_id))),
    }
}
