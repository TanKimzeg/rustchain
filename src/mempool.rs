use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::transaction::generate_wallet;
use crate::{
    COINBASE_ADDR,
    block::{Block, Blockchain},
    transaction::Transaction,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemeryPool {
    pub candidate: HashSet<Transaction>,
}

impl MemeryPool {
    pub fn new() -> Self {
        Self {
            candidate: HashSet::new(),
        }
    }
    // 提交交易
    pub fn submit(&mut self, tx: Transaction) -> Result<(), String> {
        if tx.verify() {
            self.candidate.insert(tx);
        } else {
            return Err("Invalid Transaction".to_string());
        }
        Ok(())
    }

    pub fn select(&self, count: usize) -> Vec<Transaction> {
        let mut txs: Vec<_> = self.candidate.iter().cloned().collect();
        txs.sort_by(|a, b| b.fee.cmp(&a.fee));
        txs.into_iter().take(count).collect()
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
    use crate::REWARD;
    use crate::transaction::generate_wallet;
    use ed25519_dalek::SigningKey;

    fn make_valid_tx() -> Transaction {
        let sender = generate_wallet();
        let receiver = hex::encode(generate_wallet().verifying_key().to_bytes());
        Transaction::new(&sender, &receiver, 100, 1, 0)
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

    // ── 测试辅助 ──────────────────────────

    fn wallet() -> (SigningKey, String) {
        let w = generate_wallet();
        let addr = hex::encode(w.verifying_key().to_bytes());
        (w, addr)
    }

    /// 从交易列表创建已挖好的区块（简化版，不依赖 API 或循环）
    fn make_block(chain: &Blockchain, txs: Vec<Transaction>, miner_addr: &str) -> Block {
        let prev = chain.latest_block();
        let fees: u64 = txs.iter().map(|t| t.fee).sum();
        let coinbase = Transaction::new_coinbase(miner_addr, fees);
        let mut all_txs = vec![coinbase];
        all_txs.extend(txs);
        let mut block = Block::new(prev.index + 1, all_txs, prev.hash.clone());
        block.mine_block(chain.difficulty);
        block
    }

    // ── Miner 测试 ──────────────────────────

    #[test]
    fn test_miner_assemble_block_has_coinbase() {
        let mut chain = Blockchain::new(2);
        let (_alice, alice_addr) = wallet();
        // 初始块给 Alice 资金
        let _ = chain.add_block(make_block(&chain, vec![], &alice_addr));

        let pool = Arc::new(Mutex::new(MemeryPool::new()));
        let chain_arc = Arc::new(Mutex::new(chain));
        let miner = Miner {
            address: "test_miner".to_string(),
            pool: pool.clone(),
            chain: chain_arc.clone(),
            tx_count: Arc::new(Mutex::new(HashMap::new())),
        };

        let block = miner.assemble_block();

        assert_eq!(block.index, 2);
        assert!(!block.transactions.is_empty());
        assert_eq!(block.transactions[0].sender, COINBASE_ADDR);
        assert_eq!(block.transactions[0].receiver, "test_miner");
        assert_eq!(block.transactions[0].amount, REWARD);
        assert!(block.hash.starts_with("00"));
    }

    #[test]
    fn test_miner_assemble_block_includes_paid_txs() {
        let mut chain = Blockchain::new(2);
        let (alice, alice_addr) = wallet();
        let (_, bob_addr) = wallet();
        let _ = chain.add_block(make_block(&chain, vec![], &alice_addr));

        let pool = Arc::new(Mutex::new(MemeryPool::new()));
        let chain_arc = Arc::new(Mutex::new(chain));

        // Alice 提交交易
        let tx = Transaction::new(&alice, &bob_addr, 10, 1, 0);
        pool.lock().unwrap().submit(tx.clone()).unwrap();

        let miner = Miner {
            address: "miner".to_string(),
            pool: pool.clone(),
            chain: chain_arc.clone(),
            tx_count: Arc::new(Mutex::new(HashMap::new())),
        };

        let block = miner.assemble_block();

        // coinbase + Alice 的交易
        assert_eq!(block.transactions.len(), 2);
        assert_eq!(block.transactions[1], tx);
        // coinbase 金额 = REWARD + fee
        assert_eq!(block.transactions[0].amount, REWARD + 1);
    }

    #[test]
    fn test_miner_assemble_block_can_be_added_to_chain() {
        let mut chain = Blockchain::new(2);
        let (_alice, alice_addr) = wallet();
        let _ = chain.add_block(make_block(&chain, vec![], &alice_addr));

        let chain_arc = Arc::new(Mutex::new(chain));
        let miner = Miner::start_new(chain_arc.clone());

        let block;
        {
            block = miner.assemble_block();
        }
        let mut c = chain_arc.lock().unwrap();
        assert!(c.add_block(block).is_ok());
        assert_eq!(c.chain.len(), 3); // genesis + fund + mined
        assert!(c.is_valid().is_ok());
    }

    #[test]
    fn test_miner_start_new_creates_miner_with_pool() {
        let chain = Arc::new(Mutex::new(Blockchain::new(2)));
        let miner = Miner::start_new(chain.clone());
        assert!(!miner.address.is_empty());
        // 验证矿工有自己的交易池
        let pool = miner.pool.lock().unwrap();
        assert!(pool.candidate.is_empty());
    }
}

#[derive(Clone)]
pub struct Miner {
    pub address: String,
    pub pool: Arc<Mutex<MemeryPool>>,
    pub chain: Arc<Mutex<Blockchain>>,
    pub tx_count: Arc<Mutex<HashMap<String, u64>>>,
}

impl Miner {
    /// 组装一个新区块（coinbase + 有效交易）
    pub fn assemble_block(&self) -> Block {
        let txs = self.pool.lock().unwrap().select(10);

        let fees = txs.iter().map(|t| t.fee).sum::<u64>();
        let coinbase = Transaction::new_coinbase(&self.address, fees);

        let mut all_txs = vec![coinbase];
        all_txs.extend(txs);
        let (valid_txs, invalid_txs) = self.filter_valid_txs(all_txs);
        self.pool.lock().unwrap().remove(&invalid_txs);

        let chain = self.chain.lock().unwrap();
        let prev_block = chain.latest_block();
        let mut block = Block::new(prev_block.index + 1, valid_txs.clone(), prev_block.hash.clone());
        block.mine_block(chain.difficulty); // PoW 循环在这里
        self.pool.lock().unwrap().remove(&valid_txs);
        block
    }

    /// 后台循环挖矿
    pub fn start_mining_loop(&self) {
        let miner = self.clone();
        std::thread::spawn(move || {
            loop {
                let block = miner.assemble_block();
                miner.chain.lock().unwrap().add_block(block).ok();

                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        });
    }

    pub fn start_new(chain: Arc<Mutex<Blockchain>>) -> Self {
        let miner = Self {
            address: hex::encode(generate_wallet().verifying_key().to_bytes()),
            pool: Arc::new(Mutex::new(MemeryPool::new())),
            tx_count: Arc::new(Mutex::new(HashMap::new())),
            chain,
        };
        miner.start_mining_loop();
        miner
    }

    /// 从交易列表中过滤出合法/非法的交易
    pub fn filter_valid_txs(&self, txs: Vec<Transaction>) -> (Vec<Transaction>, Vec<Transaction>) {
        let mut valid = txs.clone();
        let mut invalid = Vec::new();
        // 从已上链的区块推导当前余额，然后逐笔模拟本批交易
        let mut sim_balances = self.chain.lock().unwrap().compute_balances().unwrap();
        let tx_count = self.tx_count.lock().unwrap();
        for (i, tx) in txs.iter().enumerate() {
            if tx.sender == COINBASE_ADDR {
                // coinbase: 凭空创造货币，加到接收方余额
                *sim_balances.entry(tx.receiver.clone()).or_insert(0) += tx.amount;
                // valid.push(tx.clone());
                continue;
            }
            if tx.verify()
                && sim_balances.get(&tx.sender).copied().unwrap_or(0) >= tx.amount + tx.fee
            {
                // 普通交易：发送方扣钱，接收方加钱
                *sim_balances.get_mut(&tx.sender).unwrap() -= tx.amount + tx.fee;
                *sim_balances.entry(tx.receiver.clone()).or_insert(0) += tx.amount;
                // valid.push(tx.clone());
            } else {
                invalid.push(tx.clone());
                valid.remove(i);
                continue;
            }
            // 检查交易nonce
            if tx_count.get(&tx.sender).unwrap_or(&0u64) != &tx.nonce {
                invalid.push(tx.clone());
                valid.remove(i);
                continue;
            }
        }
        (valid, invalid)
    }
}
