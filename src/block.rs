use std::collections::HashMap;

use hex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use chrono::Utc;

use crate::{
    COINBASE_ADDR, INIT_ADJ_INTERVAL, INIT_TARGET_TIME, REWARD, merkle, transaction::Transaction,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Block {
    pub index: u32,
    pub timestamp: u64,
    pub hash: String,
    pub prev_hash: String,
    pub nonce: u64,
    pub mined_difficulty: usize,
    pub merkle_root: String,
    pub transactions: Vec<Transaction>,
}

impl Block {
    /// 创建创世区块
    pub fn genesis() -> Self {
        let timestamp = Utc::now().timestamp() as u64;
        let transactions = Vec::new(); // 应该为 CoinJoin 地址
        let prev_hash = "0".repeat(64);
        let nonce = 0;
        let mut block = Self {
            index: 0,
            timestamp,
            transactions,
            merkle_root: String::new(),
            hash: String::new(),
            prev_hash,
            nonce,
            mined_difficulty: 0,
        };
        block.hash = block.calculate_hash();
        block
    }
    /// 计算区块哈希
    pub fn calculate_hash(&self) -> String {
        let mut hasher = Sha256::new();
        let input = format!(
            "{}{}{}{}{}",
            self.index, self.timestamp, self.merkle_root, self.prev_hash, self.nonce
        );
        hasher.update(input);
        hex::encode(hasher.finalize())
    }

    pub fn new(index: u32, transactions: Vec<Transaction>, prev_hash: String) -> Self {
        let timestamp = Utc::now().timestamp() as u64;
        let merkle_root = merkle::compute_merkle_root(&transactions);
        let mut block = Block {
            index,
            timestamp,
            transactions,
            merkle_root,
            nonce: 0,
            mined_difficulty: 0,
            hash: String::new(),
            prev_hash,
        };
        block.hash = block.calculate_hash();
        block
    }
    /// 挖矿：找到符合条件的nonce，使得哈希满足难度要求
    /// 难度用哈希前导0的数量表示，比如难度4就是哈希前4位是0
    pub fn mine_block(&mut self, difficulty: usize) {
        let prefix = "0".repeat(difficulty);
        println!("开始挖矿，难度: {}（前导0数量）", difficulty);
        // 循环修改nonce，直到哈希满足前导0要求
        while &self.calculate_hash()[..difficulty] != prefix {
            self.nonce += 1;
        }
        // 挖矿成功，更新最终哈希
        self.hash = self.calculate_hash();
        // 存入难度
        self.mined_difficulty = difficulty;
        println!("挖矿成功！nonce: {}, 哈希: {}", self.nonce, self.hash);
    }
}
#[derive(Debug, Clone)]
pub struct Blockchain {
    pub chain: Vec<Block>,             // 存储所有区块
    pub difficulty: usize,             // 统一挖矿难度
    pub balance: HashMap<String, u64>, // 余额
    pub target_block_time: u64,        // 目标出块时间（秒）
    pub adjustment_interval: u32,      // 每几个块调整一次难度
}
impl Blockchain {
    /// 初始化区块链：自动创建创世块
    pub fn new(difficulty: usize) -> Self {
        let genesis = Block::genesis();
        Blockchain {
            chain: vec![genesis],
            difficulty,
            balance: HashMap::new(),
            target_block_time: INIT_TARGET_TIME,
            adjustment_interval: INIT_ADJ_INTERVAL,
        }
    }
    /// 获取链上最新的区块
    pub fn latest_block(&self) -> &Block {
        self.chain.last().unwrap()
    }

    /// 调整难度
    fn adjust_difficulty(&mut self) {
        // 还没到调整点，跳过
        if self.latest_block().index as u32 % self.adjustment_interval != 0 {
            return;
        }

        // 取最近 interval 个块的实际时间
        let start = self.chain.len() - self.adjustment_interval as usize;
        let actual_time = self.chain.last().unwrap().timestamp - self.chain[start].timestamp;

        let target_time = self.target_block_time * self.adjustment_interval as u64;
        if actual_time < target_time / 2 {
            self.difficulty = self.difficulty.saturating_add(1);
            println!("⛏️  出块过快(+1)，难度增至 {}", self.difficulty);
        } else if actual_time > target_time * 2 {
            self.difficulty = self.difficulty.saturating_sub(1);
            println!("⛏️  出块过慢(-1)，难度降至 {}", self.difficulty);
        }
        // 在 target_time/2 ~ target_time*2 之间就不调
    }
    /// 添加新区块到链上
    pub fn add_block(&mut self, transactions: Vec<Transaction>, miner: &str) {
        let prev_hash = self.latest_block().hash.clone();
        let index = self.latest_block().index + 1;
        let total_fee = transactions.iter().fold(0, |sum, tx| sum + tx.fee);
        let coinbase = Transaction {
            sender: COINBASE_ADDR.to_string(),
            receiver: miner.to_string(),
            amount: REWARD + total_fee,
            signature: String::new(),
            fee: 0,
        };
        let mut all_txs = vec![coinbase];
        all_txs.extend(transactions);
        let (valid_txs, _invalid_txs) = self.filter_valid_txs(all_txs);
        let mut new_block = Block::new(index, valid_txs, prev_hash);
        // 2. 挖矿（工作量证明）
        new_block.mine_block(self.difficulty);
        // 3. 把区块加到链上
        self.chain.push(new_block);
        self.adjust_difficulty();
    }
    /// 验证整条链是否合法（核心逻辑：检测是否被篡改）
    pub fn is_valid(&self) -> bool {
        // 从第二个区块开始遍历（创世块没有前哈希，不需要验证）
        for i in 1..self.chain.len() {
            let current = &self.chain[i];
            let prev = &self.chain[i - 1];
            // 校验1：当前区块的哈希是否被篡改
            if current.hash != current.calculate_hash() {
                println!("❌ 区块{}的哈希不匹配，被篡改！", current.index);
                return false;
            }
            // 校验2：当前区块的前哈希是否和前一个区块的哈希一致
            if current.prev_hash != prev.hash {
                println!("❌ 区块{}的前哈希不匹配，链断裂！", current.index);
                return false;
            }
            // 校验3：工作量证明是否合法（哈希必须有 difficulty 个前导 0）
            if &current.hash[..self.difficulty] != "0".repeat(current.mined_difficulty) {
                println!("❌ 区块{}的工作量证明无效！", current.index);
                return false;
            }

            // 校验4：区块第一笔必须是 coinbase，且只有一笔
            if current.transactions.is_empty() || current.transactions[0].sender != COINBASE_ADDR {
                println!("❌ 区块{}缺少或放错了coinbase", current.index);
                return false;
            }
            if current.transactions[1..]
                .iter()
                .any(|tx| tx.sender == COINBASE_ADDR)
            {
                println!("❌ 区块{}有不止一笔coinbase", current.index);
                return false;
            }

            // 校验5：每笔交易签名有效
            for tx in &current.transactions {
                if !tx.verify() {
                    println!("❌ 区块{}存在签名无效的交易", current.index);
                    return false;
                }
            }
        }
        println!("✅ 链验证通过，未被篡改");
        true
    }

    /// 计算余额
    pub fn compute_balances(&self) -> HashMap<String, u64> {
        let mut balances = self.balance.clone();
        for block in &self.chain {
            for tx in &block.transactions {
                if tx.sender != COINBASE_ADDR {
                    if let Some(_) = balances
                        .entry(tx.sender.clone())
                        .or_insert(0)
                        .checked_sub(tx.amount)
                    {
                        *balances.entry(tx.sender.clone()).or_insert(0) -= tx.amount + tx.fee;
                    }
                }
                *balances.entry(tx.receiver.clone()).or_insert(0) += tx.amount;
            }
        }
        balances
    }

    /// 从交易列表中过滤出合法/非法的交易
    pub fn filter_valid_txs(&self, txs: Vec<Transaction>) -> (Vec<Transaction>, Vec<Transaction>) {
        let mut valid = Vec::new();
        let mut invalid = Vec::new();
        // 从已上链的区块推导当前余额，然后逐笔模拟本批交易
        let mut sim_balances = self.compute_balances();
        for tx in txs {
            if tx.sender == COINBASE_ADDR {
                // coinbase: 凭空创造货币，加到接收方余额
                *sim_balances.entry(tx.receiver.clone()).or_insert(0) += tx.amount;
                valid.push(tx);
            } else if tx.verify()
                && sim_balances.get(&tx.sender).copied().unwrap_or(0) >= tx.amount + tx.fee
            {
                // 普通交易：发送方扣钱，接收方加钱
                *sim_balances.get_mut(&tx.sender).unwrap() -= tx.amount + tx.fee;
                *sim_balances.entry(tx.receiver.clone()).or_insert(0) += tx.amount;
                valid.push(tx);
            } else {
                invalid.push(tx);
            }
        }
        (valid, invalid)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{mempool::MemeryPool, transaction::generate_wallet};
    use ed25519_dalek::SigningKey;

    /// 生成钱包和地址对
    fn wallet() -> (SigningKey, String) {
        let w = generate_wallet();
        let addr = hex::encode(w.verifying_key().to_bytes());
        (w, addr)
    }

    /// 建一条 3 个区块的链（Alice→Bob 15, Bob→Charlie 10, Charlie→Alice 2）
    fn chain_with_three_blocks() -> (
        Blockchain,
        SigningKey,
        String,
        SigningKey,
        String,
        SigningKey,
        String,
    ) {
        let mut c = Blockchain::new(2);
        let (alice, alice_addr) = wallet();
        let (bob, bob_addr) = wallet();
        let (charlie, charlie_addr) = wallet();
        c.add_block(
            vec![Transaction::new(&alice, &bob_addr, 15, 1)],
            &alice_addr,
        );
        c.add_block(
            vec![Transaction::new(&bob, &charlie_addr, 10, 1)],
            &bob_addr,
        );
        c.add_block(
            vec![Transaction::new(&charlie, &alice_addr, 2, 1)],
            &charlie_addr,
        );
        (c, alice, alice_addr, bob, bob_addr, charlie, charlie_addr)
    }

    // ── 基础区块 ──────────────────────────────────

    #[test]
    fn test_genesis() {
        let c = Blockchain::new(2);
        assert_eq!(c.chain.len(), 1);
        assert_eq!(c.chain[0].index, 0);
        assert_eq!(c.chain[0].prev_hash, "0".repeat(64));
        assert!(c.chain[0].transactions.is_empty());
    }

    #[test]
    fn test_add_block_updates_balance() {
        let mut c = Blockchain::new(2);
        let (alice, alice_addr) = wallet();
        let (_, bob_addr) = wallet();
        c.add_block(
            vec![Transaction::new(&alice, &bob_addr, 15, 1)],
            &alice_addr,
        );
        let b = c.compute_balances();
        assert_eq!(b[&alice_addr], 35); // coinbase 50 - 15
        assert_eq!(b[&bob_addr], 15);
    }

    // ── 链验证 ──────────────────────────────────

    #[test]
    fn test_valid_chain_passes() {
        let (c, _, _, _, _, _, _) = chain_with_three_blocks();
        assert!(c.is_valid());
    }

    #[test]
    fn test_detect_tampered_transactions() {
        let (mut c, alice, _, _, bob_addr, _, _) = chain_with_three_blocks();
        c.chain[1].transactions = vec![Transaction::new(&alice, &bob_addr, 999, 1)];
        c.chain[1].hash = c.chain[1].calculate_hash();
        assert!(!c.is_valid());
    }

    #[test]
    fn test_detect_broken_link() {
        let (mut c, _, _, _, _, _, _) = chain_with_three_blocks();
        c.chain[2].prev_hash = "a".repeat(64);
        assert!(!c.is_valid());
    }

    #[test]
    fn test_detect_invalid_pow() {
        let (mut c, _, _, _, _, _, _) = chain_with_three_blocks();
        c.chain[1].nonce = 0;
        c.chain[1].hash = c.chain[1].calculate_hash();
        assert!(!c.is_valid());
    }

    // ── filter_valid_txs ──────────────────────────

    #[test]
    fn test_filter_accepts_valid_tx() {
        let (c, alice, _, _, bob_addr, _, _) = chain_with_three_blocks();
        let tx = Transaction::new(&alice, &bob_addr, 10, 1);
        let (v, i) = c.filter_valid_txs(vec![tx]);
        assert_eq!(v.len(), 1);
        assert_eq!(i.len(), 0);
    }

    #[test]
    fn test_filter_rejects_overspend() {
        let (c, alice, _, _, bob_addr, _, _) = chain_with_three_blocks();
        let tx = Transaction::new(&alice, &bob_addr, 999, 1);
        let (v, i) = c.filter_valid_txs(vec![tx]);
        assert_eq!(v.len(), 0);
        assert_eq!(i.len(), 1);
    }

    #[test]
    fn test_filter_rejects_bad_signature() {
        let (c, _, _, bob, bob_addr, _, _) = chain_with_three_blocks();
        let mut tx = Transaction::new(&bob, &bob_addr, 5, 1);
        tx.sender = "a".repeat(64);
        let (v, i) = c.filter_valid_txs(vec![tx]);
        assert_eq!(v.len(), 0);
        assert_eq!(i.len(), 1);
    }

    #[test]
    fn test_filter_coinbase_always_valid() {
        let c = Blockchain::new(2);
        let coinbase = Transaction {
            sender: COINBASE_ADDR.to_string(),
            receiver: "miner".to_string(),
            amount: REWARD,
            signature: String::new(),
            fee: 0,
        };
        let (v, i) = c.filter_valid_txs(vec![coinbase]);
        assert_eq!(v.len(), 1);
        assert_eq!(i.len(), 0);
    }

    // ── Mempool 集成 ────────────────────────────

    #[test]
    fn test_mempool_full_flow() {
        let mut c = Blockchain::new(2);
        let (alice, alice_addr) = wallet();
        let (_bob, bob_addr) = wallet();
        let (_miner, miner_addr) = wallet();

        // 给 Alice 50 初始资金
        c.add_block(vec![], &alice_addr);

        let mut pool = MemeryPool::new();
        pool.submit(Transaction::new(&alice, &bob_addr, 20, 1))
            .unwrap();

        let selected = pool.select(10);
        c.add_block(selected, &miner_addr);
        pool.remove(&c.latest_block().transactions[1..]);

        assert_eq!(c.compute_balances()[&alice_addr], 29); // 50 - 20
        assert_eq!(c.compute_balances()[&bob_addr], 20);
        assert_eq!(c.compute_balances()[&miner_addr], 51);
        assert!(pool.candidate.is_empty());
    }
}
