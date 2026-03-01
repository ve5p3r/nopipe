use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Filter;
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use anyhow::Result;
use tracing::{info, warn};

#[derive(Clone, Debug)]
pub struct CacheEntry {
    pub tier: u8,
    pub expires_at: Instant,
    pub fetched_at_block: u64,
}

pub struct NftCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub invalidations: u64,
    pub avg_cold_ms: f64,
}

pub struct NftVerificationCache {
    entries: DashMap<Address, CacheEntry>,
    ttl: Duration,
    rpc_http: String,
    rpc_ws: String,
    nft_contract: Address,
    hits: AtomicU64,
    misses: AtomicU64,
    invalidations: AtomicU64,
    cold_total_ms: AtomicU64,
    cold_count: AtomicU32,
}

impl NftVerificationCache {
    pub fn new(rpc_http: String, rpc_ws: String, nft_contract: Address, ttl: Duration) -> Self {
        Self {
            entries: DashMap::new(),
            ttl,
            rpc_http,
            rpc_ws,
            nft_contract,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            invalidations: AtomicU64::new(0),
            cold_total_ms: AtomicU64::new(0),
            cold_count: AtomicU32::new(0),
        }
    }

    fn read_if_fresh(&self, wallet: Address) -> Option<u8> {
        self.entries.get(&wallet).and_then(|e| {
            if Instant::now() < e.expires_at { Some(e.tier) } else { None }
        })
    }

    async fn fetch_tier_from_chain(&self, wallet: Address) -> Result<(u8, u64)> {
        let provider = ProviderBuilder::new().connect_http(self.rpc_http.parse().unwrap());
        let block = provider.get_block_number().await?;

        // highestTier(address) selector
        let selector = &alloy::primitives::keccak256(b"highestTier(address)")[..4];
        let mut addr_param = [0u8; 32];
        addr_param[12..].copy_from_slice(wallet.as_slice());
        let mut calldata = selector.to_vec();
        calldata.extend_from_slice(&addr_param);

        let result = provider.call(
            alloy::rpc::types::TransactionRequest::default()
                .to(self.nft_contract)
                .input(alloy::primitives::Bytes::from(calldata).into()),
        ).await?;

        let tier = if result.len() >= 32 { result[31] } else { 0u8 };
        Ok((tier, block))
    }

    fn upsert_entry(&self, wallet: Address, tier: u8, block: u64) {
        self.entries.insert(wallet, CacheEntry {
            tier,
            expires_at: Instant::now() + self.ttl,
            fetched_at_block: block,
        });
    }

    pub async fn get_tier(&self, wallet: Address) -> Result<u8> {
        if let Some(tier) = self.read_if_fresh(wallet) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Ok(tier);
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        let start = Instant::now();
        let (tier, block) = self.fetch_tier_from_chain(wallet).await?;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        self.cold_total_ms.fetch_add(elapsed_ms, Ordering::Relaxed);
        self.cold_count.fetch_add(1, Ordering::Relaxed);
        self.upsert_entry(wallet, tier, block);
        Ok(tier)
    }

    pub fn invalidate_wallet(&self, wallet: Address) {
        self.entries.remove(&wallet);
        self.invalidations.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot_stats(&self) -> NftCacheStats {
        let count = self.cold_count.load(Ordering::Relaxed);
        let total_ms = self.cold_total_ms.load(Ordering::Relaxed);
        NftCacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            invalidations: self.invalidations.load(Ordering::Relaxed),
            avg_cold_ms: if count > 0 { total_ms as f64 / count as f64 } else { 0.0 },
        }
    }

    pub async fn start_invalidation_listener(self: Arc<Self>) -> Result<()> {
        loop {
            match self.run_listener().await {
                Ok(()) => info!("NFT cache WS listener exited, reconnecting"),
                Err(e) => warn!("NFT cache WS error: {e}, reconnecting in 5s"),
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn run_listener(&self) -> Result<()> {
        let ws = WsConnect::new(self.rpc_ws.clone());
        let provider = ProviderBuilder::new().connect_ws(ws).await?;
        let filter = Filter::new()
            .address(self.nft_contract)
            .event("Transfer(address,address,uint256)");
        let sub = provider.subscribe_logs(&filter).await?;
        let mut stream = sub.into_stream();
        info!("NFT cache invalidation listener connected");
        use futures_util::StreamExt;
        while let Some(log) = stream.next().await {
            if log.topics().len() >= 3 {
                let from = Address::from_slice(&log.topics()[1][12..]);
                let to   = Address::from_slice(&log.topics()[2][12..]);
                if from != Address::ZERO { self.invalidate_wallet(from); }
                if to   != Address::ZERO { self.invalidate_wallet(to); }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cache() -> NftVerificationCache {
        NftVerificationCache::new(
            "http://localhost:8545".into(),
            "ws://localhost:8546".into(),
            Address::ZERO,
            Duration::from_secs(5),
        )
    }

    #[test]
    fn ttl_expiry_returns_none() {
        let cache = make_cache();
        let wallet = Address::repeat_byte(1);
        cache.entries.insert(wallet, CacheEntry {
            tier: 2,
            expires_at: Instant::now() - Duration::from_secs(1),
            fetched_at_block: 1,
        });
        assert!(cache.read_if_fresh(wallet).is_none());
    }

    #[test]
    fn fresh_entry_returns_tier() {
        let cache = make_cache();
        let wallet = Address::repeat_byte(2);
        cache.upsert_entry(wallet, 2, 100);
        assert_eq!(cache.read_if_fresh(wallet), Some(2));
    }

    #[test]
    fn invalidate_removes_entry() {
        let cache = make_cache();
        let wallet = Address::repeat_byte(3);
        cache.upsert_entry(wallet, 1, 50);
        cache.invalidate_wallet(wallet);
        assert!(cache.read_if_fresh(wallet).is_none());
        assert_eq!(cache.snapshot_stats().invalidations, 1);
    }
}
