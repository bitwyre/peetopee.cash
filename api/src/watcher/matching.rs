use std::collections::HashSet;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct IncomingTransfer {
    pub txid: String,
    pub to: String,
    pub amount: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// First transfer to `courier_address` of at least `min_amount`, at/after
/// `requested_at`, whose txid hasn't already settled another order.
pub fn find_matching_transfer<'a>(
    transfers: &'a [IncomingTransfer],
    courier_address: &str,
    min_amount: Decimal,
    requested_at: DateTime<Utc>,
    used_txids: &HashSet<String>,
) -> Option<&'a IncomingTransfer> {
    transfers.iter().find(|t| {
        t.to.eq_ignore_ascii_case(courier_address)
            && t.amount >= min_amount
            && t.timestamp >= requested_at
            && !used_txids.contains(&t.txid)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn t(txid: &str, to: &str, amount: &str, secs: i64) -> IncomingTransfer {
        IncomingTransfer {
            txid: txid.into(),
            to: to.into(),
            amount: Decimal::from_str(amount).unwrap(),
            timestamp: DateTime::from_timestamp(secs, 0).unwrap(),
        }
    }

    const ADDR: &str = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";

    #[test]
    fn matches_exact_amount_after_request() {
        let transfers = [t("tx1", ADDR, "92.5", 1000)];
        let hit = find_matching_transfer(&transfers, ADDR, Decimal::from_str("92.5").unwrap(),
            DateTime::from_timestamp(900, 0).unwrap(), &HashSet::new());
        assert_eq!(hit.unwrap().txid, "tx1");
    }

    #[test]
    fn rejects_low_amount_early_timestamp_wrong_address_and_used_txid() {
        let requested = DateTime::from_timestamp(900, 0).unwrap();
        let min = Decimal::from_str("92.5").unwrap();
        assert!(find_matching_transfer(&[t("a", ADDR, "92.4", 1000)], ADDR, min, requested, &HashSet::new()).is_none());
        assert!(find_matching_transfer(&[t("b", ADDR, "92.5", 800)], ADDR, min, requested, &HashSet::new()).is_none());
        assert!(find_matching_transfer(&[t("c", "TOtherAddr", "92.5", 1000)], ADDR, min, requested, &HashSet::new()).is_none());
        let used: HashSet<String> = ["d".to_string()].into();
        assert!(find_matching_transfer(&[t("d", ADDR, "92.5", 1000)], ADDR, min, requested, &used).is_none());
    }

    #[test]
    fn double_settlement_guard_second_identical_order_gets_no_match() {
        // Two orders (same courier address, same amount, same request time) racing for a
        // single on-chain transfer within one poll tick. This mirrors the fix in
        // watcher::mod::check_order: once a txid settles an order, it must be inserted into
        // the shared `used` set immediately so any later order in the same tick can't match
        // the same transfer again.
        let requested = DateTime::from_timestamp(900, 0).unwrap();
        let min = Decimal::from_str("92.5").unwrap();
        let transfers = [t("shared-tx", ADDR, "92.5", 1000)];
        let mut used: HashSet<String> = HashSet::new();

        // First order matches the transfer.
        let first = find_matching_transfer(&transfers, ADDR, min, requested, &used);
        assert_eq!(first.unwrap().txid, "shared-tx");

        // Simulate the successful CAS settle recording the txid as used.
        used.insert(first.unwrap().txid.clone());

        // Second, identical order must not be able to reuse the same txid.
        let second = find_matching_transfer(&transfers, ADDR, min, requested, &used);
        assert!(second.is_none());
    }

    #[test]
    fn evm_address_match_is_case_insensitive() {
        let transfers = [t("e", "0xdac17f958d2ee523a2206206994597c13d831ec7", "50", 1000)];
        let hit = find_matching_transfer(&transfers, "0xdAC17F958D2ee523a2206206994597C13D831ec7",
            Decimal::from_str("50").unwrap(), DateTime::from_timestamp(900, 0).unwrap(), &HashSet::new());
        assert!(hit.is_some());
    }
}
