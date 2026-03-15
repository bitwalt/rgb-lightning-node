use bitcoin::blockdata::transaction::Transaction;
use bitcoin::{BlockHash, ScriptBuf, Txid};
use electrum_client::{Client as ElectrumClient, ElectrumApi, GetHistoryRes};
use lightning::chain::chaininterface::{BroadcasterInterface, ConfirmationTarget, FeeEstimator};
use lightning::chain::{Filter, WatchedOutput};
use lightning::log_warn;
use lightning::util::logger::Logger;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::disk::FilesystemLogger;
#[cfg(test)]
use crate::test::mock_fee;

/// The minimum feerate we are allowed to send, as specified by LDK.
const MIN_FEERATE: u32 = 253;

pub(crate) struct IndexerClient {
    pub(crate) electrum: Arc<ElectrumClient>,
    fees: Arc<HashMap<ConfirmationTarget, AtomicU32>>,
    handle: tokio::runtime::Handle,
    logger: Arc<FilesystemLogger>,
    // Watched scripts registered by LDK's chain::Filter
    watched_txids: Arc<Mutex<HashMap<Txid, ScriptBuf>>>,
    watched_outputs: Arc<Mutex<Vec<ScriptBuf>>>,
}

impl IndexerClient {
    pub(crate) fn new(
        electrum_url: &str,
        handle: tokio::runtime::Handle,
        logger: Arc<FilesystemLogger>,
    ) -> std::io::Result<Self> {
        let electrum = ElectrumClient::new(electrum_url).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                format!("Failed to connect to electrum indexer: {e}"),
            )
        })?;
        // Validate connection
        electrum.ping().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                format!("Failed to ping electrum indexer: {e}"),
            )
        })?;

        let mut fees: HashMap<ConfirmationTarget, AtomicU32> = HashMap::new();
        fees.insert(
            ConfirmationTarget::MaximumFeeEstimate,
            AtomicU32::new(50000),
        );
        fees.insert(ConfirmationTarget::UrgentOnChainSweep, AtomicU32::new(5000));
        fees.insert(
            ConfirmationTarget::MinAllowedAnchorChannelRemoteFee,
            AtomicU32::new(MIN_FEERATE),
        );
        fees.insert(
            ConfirmationTarget::MinAllowedNonAnchorChannelRemoteFee,
            AtomicU32::new(MIN_FEERATE),
        );
        fees.insert(
            ConfirmationTarget::AnchorChannelFee,
            AtomicU32::new(MIN_FEERATE),
        );
        fees.insert(
            ConfirmationTarget::NonAnchorChannelFee,
            AtomicU32::new(2000),
        );
        fees.insert(
            ConfirmationTarget::ChannelCloseMinimum,
            AtomicU32::new(MIN_FEERATE),
        );
        fees.insert(
            ConfirmationTarget::OutputSpendingFee,
            AtomicU32::new(MIN_FEERATE),
        );

        let client = Self {
            electrum: Arc::new(electrum),
            fees: Arc::new(fees),
            handle: handle.clone(),
            logger,
            watched_txids: Arc::new(Mutex::new(HashMap::new())),
            watched_outputs: Arc::new(Mutex::new(Vec::<ScriptBuf>::new())),
        };

        IndexerClient::poll_for_fee_estimates(
            client.fees.clone(),
            client.electrum.clone(),
            client.logger.clone(),
            handle,
        );

        Ok(client)
    }

    fn poll_for_fee_estimates(
        fees: Arc<HashMap<ConfirmationTarget, AtomicU32>>,
        electrum: Arc<ElectrumClient>,
        logger: Arc<FilesystemLogger>,
        handle: tokio::runtime::Handle,
    ) {
        handle.spawn(async move {
            loop {
                let electrum_clone = Arc::clone(&electrum);
                let logger_clone = Arc::clone(&logger);

                let result = tokio::task::spawn_blocking(move || {
                    // electrum estimate_fee returns BTC/kB
                    // LDK uses sat/1000_weight: BTC/kB * 100_000_000 / 4 = sat/kw
                    let btc_per_kb_to_sat_per_kw =
                        |f: f64| -> u32 { (f * 100_000_000.0 / 4.0).round() as u32 };

                    let background = electrum_clone
                        .estimate_fee(144)
                        .ok()
                        .map(btc_per_kb_to_sat_per_kw);
                    let normal = electrum_clone
                        .estimate_fee(18)
                        .ok()
                        .map(btc_per_kb_to_sat_per_kw);
                    let high_prio = electrum_clone
                        .estimate_fee(6)
                        .ok()
                        .map(btc_per_kb_to_sat_per_kw);
                    let very_high_prio = electrum_clone
                        .estimate_fee(2)
                        .ok()
                        .map(btc_per_kb_to_sat_per_kw);

                    (background, normal, high_prio, very_high_prio)
                })
                .await;

                match result {
                    Ok((background, normal, high_prio, very_high_prio)) => {
                        let background = background.unwrap_or(MIN_FEERATE).max(MIN_FEERATE);
                        let normal = normal.unwrap_or(2000).max(MIN_FEERATE);
                        let high_prio = high_prio.unwrap_or(5000).max(MIN_FEERATE);
                        let very_high_prio = very_high_prio.unwrap_or(50000).max(MIN_FEERATE);

                        fees.get(&ConfirmationTarget::MaximumFeeEstimate)
                            .unwrap()
                            .store(very_high_prio, Ordering::Release);
                        fees.get(&ConfirmationTarget::UrgentOnChainSweep)
                            .unwrap()
                            .store(high_prio, Ordering::Release);
                        fees.get(&ConfirmationTarget::MinAllowedAnchorChannelRemoteFee)
                            .unwrap()
                            .store(background.saturating_sub(250), Ordering::Release);
                        fees.get(&ConfirmationTarget::MinAllowedNonAnchorChannelRemoteFee)
                            .unwrap()
                            .store(background.saturating_sub(250), Ordering::Release);
                        fees.get(&ConfirmationTarget::AnchorChannelFee)
                            .unwrap()
                            .store(background, Ordering::Release);
                        fees.get(&ConfirmationTarget::NonAnchorChannelFee)
                            .unwrap()
                            .store(normal, Ordering::Release);
                        fees.get(&ConfirmationTarget::ChannelCloseMinimum)
                            .unwrap()
                            .store(background, Ordering::Release);
                        fees.get(&ConfirmationTarget::OutputSpendingFee)
                            .unwrap()
                            .store(background, Ordering::Release);
                    }
                    Err(e) => {
                        log_warn!(logger_clone, "Error updating fee estimates from indexer: {}", e);
                    }
                }

                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
    }

    /// Get the current best block (height, hash) from the electrum indexer.
    pub(crate) fn get_best_block_sync(&self) -> std::io::Result<(u32, BlockHash)> {
        let notification = self.electrum.block_headers_subscribe().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to get best block from indexer: {e}"),
            )
        })?;
        let height = notification.height as u32;
        let hash = notification.header.block_hash();
        Ok((height, hash))
    }

    /// Get a block header at a specific height.
    pub(crate) fn get_header_at_height(
        &self,
        height: u32,
    ) -> std::io::Result<bitcoin::blockdata::block::Header> {
        self.electrum
            .block_header(height as usize)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    }

    /// Get transaction history for a script (for chain::Confirm monitoring).
    pub(crate) fn get_script_history(
        &self,
        script: &bitcoin::Script,
    ) -> std::io::Result<Vec<GetHistoryRes>> {
        self.electrum
            .script_get_history(script)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    }

    /// Get a transaction by txid.
    pub(crate) fn get_tx(&self, txid: &Txid) -> std::io::Result<Transaction> {
        self.electrum
            .transaction_get(txid)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    }

    /// Get all unique scripts currently being watched (from registered txids and outputs).
    pub(crate) fn get_watched_scripts(&self) -> Vec<ScriptBuf> {
        let txids = self.watched_txids.lock().unwrap();
        let outputs = self.watched_outputs.lock().unwrap();
        let mut scripts: std::collections::HashSet<ScriptBuf> = std::collections::HashSet::new();
        for s in txids.values() {
            scripts.insert(s.clone());
        }
        for s in outputs.iter() {
            scripts.insert(s.clone());
        }
        scripts.into_iter().collect()
    }
}

impl FeeEstimator for IndexerClient {
    fn get_est_sat_per_1000_weight(&self, confirmation_target: ConfirmationTarget) -> u32 {
        let fee = self
            .fees
            .get(&confirmation_target)
            .unwrap()
            .load(Ordering::Acquire);
        #[cfg(test)]
        let fee = mock_fee(fee);
        fee
    }
}

impl BroadcasterInterface for IndexerClient {
    fn broadcast_transactions(&self, txs: &[&Transaction]) {
        let electrum = Arc::clone(&self.electrum);
        let logger = Arc::clone(&self.logger);
        let txs_owned: Vec<Transaction> = txs.iter().map(|tx| (*tx).clone()).collect();
        self.handle.spawn(async move {
            tokio::task::spawn_blocking(move || {
                for tx in &txs_owned {
                    // This may error if the transaction was already broadcast, which is safe to ignore.
                    match electrum.transaction_broadcast(tx) {
                        Ok(_) => {}
                        Err(e) => {
                            let err_str = e.to_string();
                            log_warn!(
                                logger,
                                "Warning, failed to broadcast a transaction, this is likely okay \
                                 but may indicate an error: {}\nTransaction: {:?}",
                                err_str,
                                tx
                            );
                        }
                    }
                }
            })
            .await
            .ok();
        });
    }
}

impl Filter for IndexerClient {
    fn register_tx(&self, txid: &Txid, script_pubkey: &bitcoin::Script) {
        self.watched_txids
            .lock()
            .unwrap()
            .insert(*txid, script_pubkey.to_owned());
    }

    fn register_output(&self, output: WatchedOutput) {
        self.watched_outputs
            .lock()
            .unwrap()
            .push(output.script_pubkey);
    }
}
