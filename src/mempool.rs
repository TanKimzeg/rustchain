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
    #[test]
    fn test_mempool() {
        let mut pool = MemeryPool::new();
        // 模拟账户
        let alice_wallet = generate_wallet();
        let alice_addr = hex::encode(alice_wallet.verifying_key().to_bytes());
        let bob_wallet = generate_wallet();
        let bob_addr = hex::encode(bob_wallet.verifying_key().to_bytes());
        let charlie_wallet = generate_wallet();
        let charlie_addr = hex::encode(charlie_wallet.verifying_key().to_bytes());

        let tx_alice2bob = Transaction::new(&alice_wallet, &bob_addr, 15);
        let tx_bob2charlie =Transaction::new(&bob_wallet, &charlie_addr, 10);
        let tx_chalie2alice =Transaction::new(&charlie_wallet, &alice_addr, 2);
        pool.submit(tx_alice2bob).unwrap();
        pool.submit(tx_bob2charlie).unwrap();
        pool.submit(tx_chalie2alice).unwrap();

        let selected = pool.select(10);
        assert!(!selected.is_empty())

    }
}
