use alloy::primitives::{Address, B256};
use alloy::signers::Signature;
use anyhow::{anyhow, Result};
use dashmap::DashMap;
use std::collections::HashSet;

pub fn build_eip191_message(
    domain: &str,
    wallet: Address,
    nonce: &str,
    payload_hash: B256,
) -> String {
    format!("Nopipe-{domain}\nwallet:{wallet}\nnonce:{nonce}\npayload:{payload_hash}")
}

pub fn verify_eip191_signature(
    message: &str,
    sig_hex: &str,
    expected_wallet: Address,
) -> Result<()> {
    let sig_bytes = hex::decode(sig_hex.trim_start_matches("0x"))
        .map_err(|e| anyhow!("Invalid sig hex: {e}"))?;
    if sig_bytes.len() != 65 {
        return Err(anyhow!("Signature must be 65 bytes"));
    }
    let sig = Signature::try_from(sig_bytes.as_slice())
        .map_err(|e| anyhow!("Failed to parse signature: {e}"))?;
    let msg_hash = alloy::primitives::keccak256(format!(
        "\x19Ethereum Signed Message:\n{}{}",
        message.len(),
        message
    ));
    let recovered = sig
        .recover_address_from_prehash(&msg_hash)
        .map_err(|e| anyhow!("Failed to recover address: {e}"))?;
    if recovered != expected_wallet {
        return Err(anyhow!(
            "Signature mismatch: expected {expected_wallet}, got {recovered}"
        ));
    }
    Ok(())
}

#[derive(Default)]
pub struct NonceStore {
    inner: DashMap<Address, HashSet<String>>,
}

impl NonceStore {
    pub fn consume_nonce(&self, wallet: Address, nonce: &str) -> bool {
        let mut entry = self.inner.entry(wallet).or_default();
        if entry.contains(nonce) {
            false
        } else {
            entry.insert(nonce.to_string());
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nonce_consumed_once() {
        let store = NonceStore::default();
        let wallet = Address::ZERO;
        assert!(store.consume_nonce(wallet, "abc123"));
        assert!(!store.consume_nonce(wallet, "abc123"));
        assert!(store.consume_nonce(wallet, "abc124"));
    }
}
