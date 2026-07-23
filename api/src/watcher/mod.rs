pub mod clients;
pub mod matching;

use std::collections::HashSet;
use std::time::Duration;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::Row;
use uuid::Uuid;
use crate::state::AppState;
use clients::{ChainClient, USDT_BEP20, USDT_ERC20};
use matching::find_matching_transfer;

pub async fn run(state: AppState) {
    let client = ChainClient::new(&state.config);
    let mut tick = tokio::time::interval(Duration::from_secs(20));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        if let Err(e) = poll_once(&state, &client).await {
            tracing::warn!("watcher poll failed: {e}");
        }
    }
}

struct PendingOrder {
    id: Uuid,
    usdt_amount: Decimal,
    requested_at: DateTime<Utc>,
    trc20: Option<String>,
    bep20: Option<String>,
    erc20: Option<String>,
}

async fn poll_once(state: &AppState, client: &ChainClient) -> Result<(), String> {
    let rows = sqlx::query(
        "SELECT o.id, o.usdt_amount, o.payment_requested_at, u.usdt_trc20, u.usdt_bep20, u.usdt_erc20 \
         FROM orders o JOIN users u ON u.id = o.courier_id \
         WHERE o.status = 'AWAITING_PAYMENT'",
    )
    .fetch_all(&state.pool).await.map_err(|e| e.to_string())?;
    if rows.is_empty() {
        return Ok(());
    }
    let mut used_txids: HashSet<String> = sqlx::query_scalar::<_, String>(
        "SELECT payment_txid FROM orders WHERE payment_txid IS NOT NULL",
    )
    .fetch_all(&state.pool).await.map_err(|e| e.to_string())?
    .into_iter().collect();

    for row in rows {
        let order = match (|| -> Result<PendingOrder, sqlx::Error> {
            Ok(PendingOrder {
                id: row.try_get("id")?,
                usdt_amount: row.try_get("usdt_amount")?,
                requested_at: row.try_get("payment_requested_at")?,
                trc20: row.try_get("usdt_trc20")?,
                bep20: row.try_get("usdt_bep20")?,
                erc20: row.try_get("usdt_erc20")?,
            })
        })() {
            Ok(order) => order,
            Err(e) => {
                tracing::warn!("watcher: skipping row with decode error: {e}");
                continue;
            }
        };
        check_order(state, client, &order, &mut used_txids).await;
    }
    Ok(())
}

async fn check_order(state: &AppState, client: &ChainClient, order: &PendingOrder, used: &mut HashSet<String>) {
    let mut attempts: Vec<(&str, Result<Vec<matching::IncomingTransfer>, String>, &str)> = Vec::new();
    if let Some(addr) = &order.trc20 {
        attempts.push(("trc20", client.trc20_transfers(addr, order.requested_at).await, addr.as_str()));
    }
    if let Some(addr) = &order.bep20 {
        attempts.push(("bep20", client.evm_transfers(56, USDT_BEP20, addr).await, addr.as_str()));
    }
    if let Some(addr) = &order.erc20 {
        attempts.push(("erc20", client.evm_transfers(1, USDT_ERC20, addr).await, addr.as_str()));
    }
    for (network, result, addr) in attempts {
        match result {
            Err(e) => tracing::warn!("watcher {network} fetch failed for order {}: {e}", order.id),
            Ok(transfers) => {
                if let Some(hit) = find_matching_transfer(&transfers, addr, order.usdt_amount, order.requested_at, used) {
                    let txid = hit.txid.clone();
                    let updated = sqlx::query(
                        "UPDATE orders SET status = 'PAID', payment_network = $1, payment_txid = $2, paid_at = now() \
                         WHERE id = $3 AND status = 'AWAITING_PAYMENT'",
                    )
                    .bind(network).bind(&txid).bind(order.id)
                    .execute(&state.pool).await;
                    match updated {
                        Ok(r) if r.rows_affected() == 1 => {
                            used.insert(txid.clone());
                            tracing::info!("order {} paid via {network} tx {}", order.id, txid);
                            return;
                        }
                        Ok(_) => {}
                        Err(e) if e.as_database_error().is_some_and(|d| d.is_unique_violation()) => {
                            // Another order already claimed this txid between our snapshot and
                            // this UPDATE (payment_txid has a UNIQUE constraint). Record it so
                            // we don't retry, and move on without killing the watcher loop.
                            tracing::warn!("watcher: txid {txid} already settled another order, skipping order {}", order.id);
                            used.insert(txid);
                        }
                        Err(e) => tracing::error!("watcher settle failed for {}: {e}", order.id),
                    }
                }
            }
        }
    }
}
