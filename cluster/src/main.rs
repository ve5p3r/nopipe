mod rpc_server;
mod nft_cache;
mod relayer;
mod keeper;
mod gauntlet;
mod security;

use anyhow::Result;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ClusterConfig {
    pub bind_addr: String,
    pub base_rpc_http: String,
    pub base_rpc_ws: String,
    pub chain_id: u64,
    pub swap_executor: alloy::primitives::Address,
    pub subscription_keeper: alloy::primitives::Address,
    pub operator_nft: alloy::primitives::Address,
    pub relayer_private_key: String,
    pub fee_recipient: alloy::primitives::Address,
    pub min_relayer_balance_wei: alloy::primitives::U256,
    pub nft_cache_ttl_secs: u64,
    pub keeper_interval_secs: u64,
    pub min_swap_tier: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cfg = ClusterConfig {
        bind_addr: std::env::var("BIND_ADDR").unwrap_or("0.0.0.0:9000".into()),
        base_rpc_http: std::env::var("BASE_RPC_HTTP").expect("BASE_RPC_HTTP required"),
        base_rpc_ws: std::env::var("BASE_RPC_WS").expect("BASE_RPC_WS required"),
        chain_id: std::env::var("CHAIN_ID").unwrap_or("84532".into()).parse()?,
        swap_executor: std::env::var("SWAP_EXECUTOR").expect("SWAP_EXECUTOR required").parse()?,
        subscription_keeper: std::env::var("SUBSCRIPTION_KEEPER").expect("SUBSCRIPTION_KEEPER required").parse()?,
        operator_nft: std::env::var("OPERATOR_NFT").expect("OPERATOR_NFT required").parse()?,
        relayer_private_key: std::env::var("RELAYER_PRIVATE_KEY").expect("RELAYER_PRIVATE_KEY required"),
        fee_recipient: std::env::var("FEE_RECIPIENT").expect("FEE_RECIPIENT required").parse()?,
        min_relayer_balance_wei: alloy::primitives::U256::from(50_000_000_000_000_000u64), // 0.05 ETH
        nft_cache_ttl_secs: 300,
        keeper_interval_secs: 60,
        min_swap_tier: 1,
    };

    run_cluster(cfg).await
}

pub async fn run_cluster(cfg: ClusterConfig) -> Result<()> {
    let nft_cache = Arc::new(nft_cache::NftVerificationCache::new(
        cfg.base_rpc_http.clone(),
        cfg.base_rpc_ws.clone(),
        cfg.operator_nft,
        std::time::Duration::from_secs(cfg.nft_cache_ttl_secs),
    ));

    let relayer = Arc::new(relayer::RelayerService::new(relayer::RelayerConfig {
        rpc_http: cfg.base_rpc_http.clone(),
        chain_id: cfg.chain_id,
        swap_executor: cfg.swap_executor,
        relayer_private_key: cfg.relayer_private_key.clone(),
        min_balance_wei: cfg.min_relayer_balance_wei,
        refill_target_wei: cfg.min_relayer_balance_wei * alloy::primitives::U256::from(3u64),
        refill_enabled: false,
    }).await?);

    let keeper = Arc::new(keeper::KeeperService::new(keeper::KeeperConfig {
        rpc_http: cfg.base_rpc_http.clone(),
        subscription_keeper: cfg.subscription_keeper,
        poll_interval_secs: cfg.keeper_interval_secs,
        start_block: None,
    }).await?);

    let cache_clone = nft_cache.clone();
    tokio::spawn(async move {
        if let Err(e) = cache_clone.start_invalidation_listener().await {
            tracing::error!("NFT cache listener: {e}");
        }
    });
    let keeper_clone = keeper.clone();
    tokio::spawn(async move { keeper_clone.start().await; });
    let relayer_clone = relayer.clone();
    tokio::spawn(async move { relayer_clone.start_gas_loop().await; });

    let app_state = rpc_server::ClusterAppState {
        nft_cache,
        relayer,
        keeper,
        config: cfg.clone(),
        nonce_store: Arc::new(security::NonceStore::default()),
        swap_statuses: Arc::new(dashmap::DashMap::new()),
    };

    rpc_server::serve(cfg.bind_addr, app_state).await
}
