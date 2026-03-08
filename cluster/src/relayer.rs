use alloy::network::EthereumWallet;
use alloy::primitives::{Address, Bytes, B256, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::{TransactionReceipt, TransactionRequest};
use alloy::signers::local::PrivateKeySigner;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info, warn};

// Pre-computed gas limit for tradeFor() — skips eth_estimateGas on every call.
// Measured on Base mainnet; 20% headroom over typical ~200k gas.
const TRADE_FOR_GAS_LIMIT: u64 = 250_000;

pub struct RelayerConfig {
    pub rpc_http: String,
    pub chain_id: u64,
    pub swap_executor: Address,
    pub relayer_private_key: String,
    pub db_path: String,
    pub min_balance_wei: U256,
    pub refill_target_wei: U256,
    pub refill_enabled: bool,
}

pub struct TradeForRequest {
    pub amount_in: U256,
    pub recipient: Address,
    pub router: Address,
    pub path: Vec<Address>,
    pub slippage_bps: u32,
}

pub struct SubmittedTx {
    pub tx_hash: B256,
    pub nonce: u64,
    pub submitted_at: Instant,
}

pub struct RelayerHealth {
    pub balance_wei: U256,
    pub threshold_wei: U256,
    pub last_refill_tx: Option<B256>,
    pub last_error: Option<String>,
}

pub struct RelayerService {
    config: RelayerConfig,
    signer: PrivateKeySigner,
    signer_wallet: Address,
    health: Arc<RwLock<RelayerHealth>>,
    // In-memory nonce — avoids eth_getTransactionCount per tx
    nonce: Arc<Mutex<u64>>,
}

impl RelayerService {
    pub async fn new(config: RelayerConfig) -> Result<Self> {
        let signer: PrivateKeySigner = config
            .relayer_private_key
            .parse()
            .map_err(|e| anyhow!("Invalid relayer private key: {e}"))?;

        let provider = ProviderBuilder::new().connect_http(config.rpc_http.parse().unwrap());

        let balance = provider.get_balance(signer.address()).await?;
        info!("Relayer {} balance: {} wei", signer.address(), balance);

        let signer_wallet = signer.address();
        let on_chain_nonce = provider.get_transaction_count(signer_wallet).await?;
        let stored_nonce = Self::load_persisted_nonce(&config.db_path, signer_wallet)?;
        let nonce = std::cmp::max(on_chain_nonce, stored_nonce.unwrap_or(0));
        Self::persist_nonce_at_path(&config.db_path, signer_wallet, nonce)?;
        info!("Relayer nonce at boot: {nonce}");

        let health = Arc::new(RwLock::new(RelayerHealth {
            balance_wei: balance,
            threshold_wei: config.min_balance_wei,
            last_refill_tx: None,
            last_error: None,
        }));

        Ok(Self {
            nonce: Arc::new(Mutex::new(nonce)),
            config,
            signer,
            signer_wallet,
            health,
        })
    }

    fn encode_trade_for_call(&self, req: &TradeForRequest) -> Result<Bytes> {
        // tradeFor(uint256,address,address,address[],uint32)
        let selector =
            &alloy::primitives::keccak256(b"tradeFor(uint256,address,address,address[],uint32)")
                [..4];
        let mut out = selector.to_vec();

        let b = req.amount_in.to_be_bytes::<32>();
        out.extend_from_slice(&b);

        let mut b = [0u8; 32];
        b[12..].copy_from_slice(req.recipient.as_slice());
        out.extend_from_slice(&b);

        let mut b = [0u8; 32];
        b[12..].copy_from_slice(req.router.as_slice());
        out.extend_from_slice(&b);

        // path[] offset: 5 * 32 = 160
        let mut b = [0u8; 32];
        b[31] = 160u8;
        out.extend_from_slice(&b);

        let mut b = [0u8; 32];
        b[28..].copy_from_slice(&req.slippage_bps.to_be_bytes());
        out.extend_from_slice(&b);

        let mut b = [0u8; 32];
        b[31] = req.path.len() as u8;
        out.extend_from_slice(&b);

        for addr in &req.path {
            let mut b = [0u8; 32];
            b[12..].copy_from_slice(addr.as_slice());
            out.extend_from_slice(&b);
        }

        Ok(Bytes::from(out))
    }

    pub async fn submit_trade_for(&self, req: TradeForRequest) -> Result<SubmittedTx> {
        if req.recipient == Address::ZERO {
            return Err(anyhow!("recipient cannot be zero"));
        }
        if req.path.len() < 2 {
            return Err(anyhow!("path must have >= 2 tokens"));
        }
        if req.amount_in.is_zero() {
            return Err(anyhow!("amountIn cannot be zero"));
        }

        let calldata = self.encode_trade_for_call(&req)?;

        // Grab nonce from in-memory store — no RPC round-trip
        let nonce = {
            let mut n = self.nonce.lock().await;
            let current = *n;
            *n += 1;
            self.persist_nonce(*n)?;
            current
        };

        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect_http(self.config.rpc_http.parse().unwrap());

        let tx = TransactionRequest::default()
            .to(self.config.swap_executor)
            .input(calldata.into())
            .nonce(nonce)
            .gas_limit(TRADE_FOR_GAS_LIMIT); // Pre-set — skips eth_estimateGas

        let t0 = Instant::now();
        let pending = provider
            .send_transaction(tx)
            .await
            .map_err(|e| anyhow!("Submit failed: {e}"))?;

        let tx_hash = *pending.tx_hash();
        info!(
            "submit_trade_for: {tx_hash} nonce={nonce} elapsed={}ms",
            t0.elapsed().as_millis()
        );

        Ok(SubmittedTx {
            tx_hash,
            nonce,
            submitted_at: Instant::now(),
        })
    }

    /// Resync nonce from chain — call on nonce collision errors to recover
    pub async fn resync_nonce(&self) -> Result<()> {
        let provider = ProviderBuilder::new().connect_http(self.config.rpc_http.parse().unwrap());
        let on_chain = provider
            .get_transaction_count(self.signer.address())
            .await?;
        let mut n = self.nonce.lock().await;
        *n = on_chain;
        self.persist_nonce(*n)?;
        info!("Nonce resynced from chain: {on_chain}");
        Ok(())
    }

    pub async fn wait_for_receipt(
        &self,
        tx_hash: B256,
        timeout: Duration,
    ) -> Result<TransactionReceipt> {
        let provider = ProviderBuilder::new().connect_http(self.config.rpc_http.parse().unwrap());
        let deadline = Instant::now() + timeout;
        loop {
            if Instant::now() > deadline {
                return Err(anyhow!("Timeout waiting for receipt: {tx_hash}"));
            }
            if let Ok(Some(receipt)) = provider.get_transaction_receipt(tx_hash).await {
                if !receipt.status() {
                    return Err(anyhow!("Tx reverted: {tx_hash}"));
                }
                return Ok(receipt);
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    async fn check_relayer_balance(&self) -> Result<U256> {
        let provider = ProviderBuilder::new().connect_http(self.config.rpc_http.parse().unwrap());
        let balance = provider.get_balance(self.signer.address()).await?;
        self.health.write().await.balance_wei = balance;
        Ok(balance)
    }

    async fn emit_low_balance_alert(&self, balance: U256, threshold: U256) {
        warn!(
            "Relayer {} low balance: {balance} wei (threshold: {threshold} wei)",
            self.signer.address()
        );
        self.health.write().await.last_error =
            Some(format!("Low balance: {balance} wei below {threshold} wei"));
    }

    async fn maybe_refill(&self, balance: U256) -> Result<Option<B256>> {
        if balance < self.config.min_balance_wei {
            self.emit_low_balance_alert(balance, self.config.min_balance_wei)
                .await;
            if self.config.refill_enabled {
                let amount = self.config.refill_target_wei - balance;
                return Err(anyhow!(
                    "Auto-refill not implemented — fund {amount} wei to relayer manually"
                ));
            }
        }
        Ok(None)
    }

    pub async fn start_gas_loop(self: Arc<Self>) {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            match self.check_relayer_balance().await {
                Ok(bal) => {
                    if let Err(e) = self.maybe_refill(bal).await {
                        error!("Refill: {e}");
                    }
                }
                Err(e) => error!("Balance check failed: {e}"),
            }
        }
    }

    pub async fn health_snapshot(&self) -> RelayerHealth {
        let h = self.health.read().await;
        RelayerHealth {
            balance_wei: h.balance_wei,
            threshold_wei: h.threshold_wei,
            last_refill_tx: h.last_refill_tx,
            last_error: h.last_error.clone(),
        }
    }

    fn init_db(path: &str) -> Result<()> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS pending_nonces (
                wallet TEXT PRIMARY KEY,
                nonce INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );",
        )?;
        Ok(())
    }

    fn load_persisted_nonce(path: &str, wallet: Address) -> Result<Option<u64>> {
        Self::init_db(path)?;
        let conn = Connection::open(path)?;
        let mut stmt = conn.prepare("SELECT nonce FROM pending_nonces WHERE wallet = ?1")?;
        let mut rows = stmt.query(params![wallet.to_string()])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let nonce_i64: i64 = row.get(0)?;
        let nonce = u64::try_from(nonce_i64).map_err(|_| anyhow!("Stored nonce is negative"))?;
        Ok(Some(nonce))
    }

    fn persist_nonce_at_path(path: &str, wallet: Address, nonce: u64) -> Result<()> {
        Self::init_db(path)?;
        let conn = Connection::open(path)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        conn.execute(
            "INSERT INTO pending_nonces (wallet, nonce, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(wallet) DO UPDATE SET
                nonce = excluded.nonce,
                updated_at = excluded.updated_at",
            params![
                wallet.to_string(),
                i64::try_from(nonce)?,
                i64::try_from(now)?
            ],
        )?;
        Ok(())
    }

    fn persist_nonce(&self, nonce: u64) -> Result<()> {
        Self::persist_nonce_at_path(&self.config.db_path, self.signer_wallet, nonce)
    }
}
