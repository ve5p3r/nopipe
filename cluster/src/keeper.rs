use alloy::primitives::{Address, Bytes};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::{Filter, TransactionRequest};
use anyhow::Result;
use dashmap::{DashMap, DashSet};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

pub struct KeeperConfig {
    pub rpc_http: String,
    pub subscription_keeper: Address,
    pub poll_interval_secs: u64,
    pub start_block: Option<u64>,
}

pub struct KeeperCycleReport {
    pub attempted: usize,
    pub succeeded: usize,
    pub failed: usize,
}

pub struct KeeperHealth {
    pub tracked_agents: usize,
    pub last_cycle_at: Option<u64>,
    pub consecutive_failures: u32,
}

pub struct KeeperService {
    config: KeeperConfig,
    subscribers: Arc<DashSet<Address>>,
    failure_counts: Arc<DashMap<Address, u32>>,
    consecutive_failures: Arc<AtomicU32>,
    last_cycle_block: Arc<AtomicU64>,
}

impl KeeperService {
    pub async fn new(config: KeeperConfig) -> Result<Self> {
        let svc = Self {
            config,
            subscribers: Arc::new(DashSet::new()),
            failure_counts: Arc::new(DashMap::new()),
            consecutive_failures: Arc::new(AtomicU32::new(0)),
            last_cycle_block: Arc::new(AtomicU64::new(0)),
        };
        svc.bootstrap_subscribers().await?;
        Ok(svc)
    }

    pub async fn bootstrap_subscribers(&self) -> Result<()> {
        let provider = ProviderBuilder::new().connect_http(self.config.rpc_http.parse().unwrap());
        let latest = provider.get_block_number().await?;
        let from = self
            .config
            .start_block
            .unwrap_or(latest.saturating_sub(50_000));
        self.sync_subscriber_events(from, latest).await?;
        info!("Keeper bootstrapped {} subscribers", self.subscribers.len());
        Ok(())
    }

    pub async fn sync_subscriber_events(&self, from_block: u64, to_block: u64) -> Result<u64> {
        let provider = ProviderBuilder::new().connect_http(self.config.rpc_http.parse().unwrap());
        let sig = alloy::primitives::keccak256(b"Subscribed(address)");
        let filter = Filter::new()
            .address(self.config.subscription_keeper)
            .event_signature(sig)
            .from_block(from_block)
            .to_block(to_block);
        let logs = provider.get_logs(&filter).await?;
        for log in logs {
            if log.topics().len() >= 2 {
                let agent = Address::from_slice(&log.topics()[1][12..]);
                self.upsert_subscriber(agent);
            }
        }
        Ok(to_block)
    }

    pub fn upsert_subscriber(&self, agent: Address) {
        self.subscribers.insert(agent);
    }

    fn mark_failure(&self, agent: Address, reason: String) {
        let mut count = self.failure_counts.entry(agent).or_insert(0);
        *count += 1;
        warn!(
            "collectFor({agent}) failed (#{num}): {reason}",
            num = *count
        );
    }

    fn encode_collect_for(agent: Address) -> Bytes {
        let selector = &alloy::primitives::keccak256(b"collectFor(address)")[..4];
        let mut b = [0u8; 32];
        b[12..].copy_from_slice(agent.as_slice());
        let mut out = selector.to_vec();
        out.extend_from_slice(&b);
        Bytes::from(out)
    }

    async fn collect_for_agent(&self, agent: Address) -> Result<bool> {
        let provider = ProviderBuilder::new().connect_http(self.config.rpc_http.parse().unwrap());
        let tx = TransactionRequest::default()
            .to(self.config.subscription_keeper)
            .input(Self::encode_collect_for(agent).into());
        match provider.call(tx).await {
            Ok(_) => {
                info!("collectFor({agent}) OK");
                Ok(true)
            }
            Err(e) => {
                self.mark_failure(agent, e.to_string());
                Ok(false)
            }
        }
    }

    pub async fn run_cycle(&self) -> Result<KeeperCycleReport> {
        let agents: Vec<Address> = self.subscribers.iter().map(|a| *a).collect();
        let mut report = KeeperCycleReport {
            attempted: 0,
            succeeded: 0,
            failed: 0,
        };
        for agent in agents {
            report.attempted += 1;
            match self.collect_for_agent(agent).await {
                Ok(true) => {
                    report.succeeded += 1;
                    self.consecutive_failures.store(0, Ordering::Relaxed);
                }
                Ok(false) => {
                    report.failed += 1;
                    self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    report.failed += 1;
                    error!("Keeper RPC error for {agent}: {e}");
                    self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        let block = ProviderBuilder::new()
            .connect_http(self.config.rpc_http.parse().unwrap())
            .get_block_number()
            .await
            .unwrap_or(0);
        self.last_cycle_block.store(block, Ordering::Relaxed);
        info!(
            "Keeper cycle: {}/{} ok, {} failed",
            report.succeeded, report.attempted, report.failed
        );
        Ok(report)
    }

    pub async fn start(self: Arc<Self>) {
        let mut ticker = tokio::time::interval(Duration::from_secs(self.config.poll_interval_secs));
        loop {
            ticker.tick().await;
            if let Err(e) = self.run_cycle().await {
                error!("Keeper cycle: {e}");
            }
        }
    }

    pub fn snapshot_health(&self) -> KeeperHealth {
        let b = self.last_cycle_block.load(Ordering::Relaxed);
        KeeperHealth {
            tracked_agents: self.subscribers.len(),
            last_cycle_at: if b > 0 { Some(b) } else { None },
            consecutive_failures: self.consecutive_failures.load(Ordering::Relaxed),
        }
    }
}
