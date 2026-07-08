use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use crate::transaction::Transaction;


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemeryPool {
    pub candidate: HashSet<Transaction>,
}

impl MemeryPool {
    pub fn new() -> Self {
        Self { candidate: HashSet::new() }

    }
    // 提交交易
    pub fn submit(&mut self, tx: Transaction) -> Result<(), String> {
        if tx.verify() {
            self.candidate.insert(tx);
        }
        else {
            return Err("Invalid Transaction".to_string());
        }
        Ok(())
    }

    pub fn select(&self, count: usize) -> Vec<Transaction> {
        self.candidate.iter().take(count).cloned().collect()
    }

    pub fn remove(&mut self, txs: &[Transaction]) {
        txs.iter().for_each(|tx| {
            self.candidate.remove(tx);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::generate_wallet;

    fn make_valid_tx() -> Transaction {
        let sender = generate_wallet();
        let receiver = hex::encode(generate_wallet().verifying_key().to_bytes());
        Transaction::new(&sender, &receiver, 100)
    }

    #[test]
    fn test_submit_accepts_valid() {
        let mut pool = MemeryPool::new();
        assert!(pool.submit(make_valid_tx()).is_ok());
        assert_eq!(pool.candidate.len(), 1);
    }

    #[test]
    fn test_select_limits_count() {
        let mut pool = MemeryPool::new();
        for _ in 0..5 {
            pool.submit(make_valid_tx()).unwrap();
        }
        assert_eq!(pool.select(3).len(), 3);
    }

    #[test]
    fn test_select_less_than_available() {
        let mut pool = MemeryPool::new();
        pool.submit(make_valid_tx()).unwrap();
        assert_eq!(pool.select(10).len(), 1);
    }

    #[test]
    fn test_remove_clears_selected() {
        let mut pool = MemeryPool::new();
        pool.submit(make_valid_tx()).unwrap();
        pool.submit(make_valid_tx()).unwrap();
        let txs = pool.select(2);
        pool.remove(&txs);
        assert!(pool.candidate.is_empty());
    }

    #[test]
    fn test_select_empty_pool() {
        let pool = MemeryPool::new();
        assert!(pool.select(5).is_empty());
    }
}
