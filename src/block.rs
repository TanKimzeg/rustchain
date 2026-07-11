use std::collections::HashMap;

use hex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use chrono::Utc;

use crate::{COINBASE_ADDR, INIT_ADJ_INTERVAL, INIT_TARGET_TIME, merkle, transaction::Transaction};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
            transactions: transactions,
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
        log::info!("开始挖矿，难度: {}（前导0数量）", difficulty);
        // 循环修改nonce，直到哈希满足前导0要求
        while &self.calculate_hash()[..difficulty] != prefix {
            self.nonce += 1;
        }
        // 挖矿成功，更新最终哈希
        self.hash = self.calculate_hash();
        // 存入难度
        self.mined_difficulty = difficulty;
        log::info!("挖矿成功！nonce: {}, 哈希: {}", self.nonce, self.hash);
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AddressDetail {
    pub balance: u64,
    pub nonce: u64,
    pub txs: Vec<Transaction>,
}

/// 增量地址元数据，由 `update_metadata_delta` 维护
impl AddressDetail {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blockchain {
    pub chain: Vec<Block>,        // 存储所有区块
    pub difficulty: usize,        // 统一挖矿难度
    pub target_block_time: u64,   // 目标出块时间（秒）
    pub adjustment_interval: u32, // 每几个块调整一次难度
    #[serde(skip)]
    pub address_details: HashMap<String, AddressDetail>, // 增量缓存的地址元数据
}
impl Blockchain {
    /// 初始化区块链：自动创建创世块
    pub fn new(difficulty: usize) -> Self {
        let genesis = Block::genesis();
        Blockchain {
            chain: vec![genesis],
            difficulty,
            target_block_time: INIT_TARGET_TIME,
            adjustment_interval: INIT_ADJ_INTERVAL,
            address_details: HashMap::new(),
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
            log::info!("⛏️  出块过快(+1)，难度增至 {}", self.difficulty);
        } else if actual_time > target_time * 2 {
            self.difficulty = self.difficulty.saturating_sub(1);
            log::info!("⛏️  出块过慢(-1)，难度降至 {}", self.difficulty);
        }
        // 在 target_time/2 ~ target_time*2 之间就不调
    }
    /// 验证区块
    /// 仅做简单的区块数据和交易签名的校验，不进行余额、双花等全链检查
    fn check_block(&self, block: &Block) -> Result<(), String> {
        // 校验1：当前区块的哈希是否被篡改
        if block.hash != block.calculate_hash() {
            return Err(format!("❌ 区块{}的哈希不匹配，被篡改！", block.index));
        }
        // 校验2：当前区块的前哈希是否和前一个区块的哈希一致
        if block.prev_hash != self.chain[block.index as usize - 1].hash {
            return Err(format!("❌ 区块{}的前哈希不匹配，链断裂！", block.index));
        }
        // 校验3：工作量证明是否合法（哈希必须有 difficulty 个前导 0）
        if &block.hash[..block.mined_difficulty] != "0".repeat(block.mined_difficulty) {
            return Err(format!("❌ 区块{}的工作量证明无效！", block.index));
        }

        // 校验4：区块第一笔必须是 coinbase，且只有一笔
        if block.transactions.is_empty() || block.transactions[0].sender != COINBASE_ADDR {
            return Err(format!("❌ 区块{}缺少或放错了coinbase", block.index));
        }
        if block.transactions[1..]
            .iter()
            .any(|tx| tx.sender == COINBASE_ADDR)
        {
            return Err(format!("❌ 区块{}有不止一笔coinbase", block.index));
        }

        // 校验5：每笔交易签名有效
        for tx in &block.transactions {
            if !tx.verify() {
                return Err(format!("❌ 区块{}存在签名无效的交易", block.index));
            }
        }
        Ok(())
    }
    /// 添加新区块到链上
    pub fn add_block(&mut self, block: Block) -> Result<(), String> {
        self.check_block(&block)?;
        // 增量更新地址元数据
        self.update_metadata_delta(&block);
        self.chain.push(block);
        self.adjust_difficulty();
        Ok(())
    }
    /// 遍历验证整条链是否合法
    pub fn is_valid(&self) -> Result<(), String> {
        // 从第二个区块开始遍历（创世块没有前哈希，不需要验证）
        for i in 1..self.chain.len() {
            let current = &self.chain[i];
            self.check_block(current)?;
        }
        let _balances = self.compute_balances()?;
        let _tx_count = self.get_tx_count()?;

        Ok(())
    }

    /// 计算余额
    pub fn compute_balances(&self) -> Result<HashMap<String, u64>, String> {
        // let mut balances = self.balance.clone();
        let mut balances = HashMap::new();
        for block in &self.chain {
            for tx in &block.transactions {
                if tx.sender != COINBASE_ADDR {
                    if let Some(_) = balances
                        .entry(tx.sender.clone())
                        .or_insert(0u64)
                        .checked_sub(tx.amount)
                    {
                        *balances.entry(tx.sender.clone()).or_insert(0) -= tx.amount + tx.fee;
                    } else {
                        return Err(format!(
                            "{}'s balance underflowed at Block #{}, tx: {}",
                            &tx.sender, &block.index, &tx.signature
                        ));
                    }
                }
                *balances.entry(tx.receiver.clone()).or_insert(0) += tx.amount;
            }
        }
        Ok(balances)
    }

    /// 计算地址的交易数量
    pub fn get_tx_count(&self) -> Result<HashMap<String, u64>, String> {
        let mut tx_count = HashMap::new();
        for block in &self.chain {
            for tx in &block.transactions {
                if tx_count.get(&tx.sender).unwrap_or(&0u64) == &tx.nonce {
                    *tx_count.entry(tx.sender.clone()).or_insert(0u64) += 1;
                } else if tx.sender != COINBASE_ADDR {
                    return Err(format!(
                        "Sender {} has Multiple nonce {} at Block #{} tx: {}",
                        &tx.sender, tx.nonce, block.index, &tx.signature
                    ));
                }
            }
        }
        Ok(tx_count)
    }

    /// 增量更新地址元数据（balance + tx_count）
    fn update_metadata_delta(&mut self, new_block: &Block) {
        for tx in &new_block.transactions {
            // 发送方扣款（含费）
            if tx.sender != COINBASE_ADDR {
                self.address_details.entry(tx.sender.clone()).or_default();
                self.address_details
                    .entry(tx.sender.clone())
                    .and_modify(|s| {
                        s.balance -= tx.amount+tx.fee;
                        s.nonce += 1;
                        s.txs.push(tx.clone());
                    });
            }
            // 接收方收款
            self.address_details.entry(tx.receiver.clone()).or_default();
            self.address_details
                .entry(tx.receiver.clone())
                .and_modify(|s| {
                    s.balance += tx.amount;
                    s.txs.push(tx.clone());
                });
        }
    }

    pub fn dump(&self, save_path: &str) -> std::io::Result<()> {
        let block_chain = serde_json::to_string_pretty(self)?;
        std::fs::write(save_path, block_chain)
    }

    pub fn load(path: &str) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let mut chain = serde_json::from_str::<Blockchain>(&json)?;
        chain.recover_state().unwrap();
        Ok(chain)
    }

    /// 重新推理状态
    fn recover_state(&mut self) -> Result<(), String> {
        for block in self.chain.clone() {
            self.update_metadata_delta(&block);
        }
        Ok(())
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

    /// 测试辅助：从交易列表创建已挖好的区块
    fn make_block(chain: &Blockchain, txs: Vec<Transaction>, miner_addr: &str) -> Block {
        let prev = chain.latest_block();
        let fees: u64 = txs.iter().map(|t| t.fee).sum();
        let coinbase = Transaction::new_coinbase(miner_addr, fees);
        let mut all_txs = vec![coinbase];
        all_txs.extend(txs);
        // let (valid, _) = chain.filter_valid_txs(all_txs);
        let mut block = Block::new(prev.index + 1, all_txs, prev.hash.clone());
        block.mine_block(chain.difficulty);
        block
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
        let _ = c.add_block(make_block(
            &c,
            vec![Transaction::new(&alice, &bob_addr, 15, 1, 0)],
            &alice_addr,
        ));
        let _ = c.add_block(make_block(
            &c,
            vec![Transaction::new(&bob, &charlie_addr, 10, 1, 0)],
            &bob_addr,
        ));
        let _ = c.add_block(make_block(
            &c,
            vec![Transaction::new(&charlie, &alice_addr, 2, 1, 0)],
            &charlie_addr,
        ));
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
        let _ = c.add_block(make_block(
            &c,
            vec![Transaction::new(&alice, &bob_addr, 15, 1, 3)],
            &alice_addr,
        ));
        let b = c.compute_balances().unwrap();
        assert_eq!(b[&alice_addr], 35); // coinbase 50 - 15
        assert_eq!(b[&bob_addr], 15);
    }

    // ── 链验证 ──────────────────────────────────

    #[test]
    fn test_valid_chain_passes() {
        let (c, _, _, _, _, _, _) = chain_with_three_blocks();
        assert!(c.is_valid().is_ok());
    }

    #[test]
    fn test_detect_tampered_transactions() {
        let (mut c, alice, _, _, bob_addr, _, _) = chain_with_three_blocks();
        c.chain[1].transactions = vec![Transaction::new(&alice, &bob_addr, 999, 1, 1)];
        c.chain[1].hash = c.chain[1].calculate_hash();
        assert!(!c.is_valid().is_ok());
    }

    #[test]
    fn test_detect_broken_link() {
        let (mut c, _, _, _, _, _, _) = chain_with_three_blocks();
        c.chain[2].prev_hash = "a".repeat(64);
        assert!(!c.is_valid().is_ok());
    }

    #[test]
    fn test_detect_invalid_pow() {
        let (mut c, _, _, _, _, _, _) = chain_with_three_blocks();
        c.chain[1].nonce = 0;
        c.chain[1].hash = c.chain[1].calculate_hash();
        assert!(!c.is_valid().is_ok());
    }

    // ── Mempool 集成 ────────────────────────────

    #[test]
    fn test_mempool_full_flow() {
        let mut c = Blockchain::new(2);
        let (alice, alice_addr) = wallet();
        let (_bob, bob_addr) = wallet();
        let (_miner, miner_addr) = wallet();

        // 给 Alice 50 初始资金
        let _ = c.add_block(make_block(&c, vec![], &alice_addr));

        let mut pool = MemeryPool::new();
        pool.push(Transaction::new(&alice, &bob_addr, 20, 1, 2));

        let selected = pool.select(10);
        let _ = c.add_block(make_block(&c, selected, &miner_addr));
        pool.remove(&c.latest_block().transactions[1..]);

        let balances = c.compute_balances().unwrap();
        assert_eq!(balances[&alice_addr], 29); // 50 - 20
        assert_eq!(balances[&bob_addr], 20);
        assert_eq!(balances[&miner_addr], 51);
        assert!(pool.candidate.is_empty());
    }

    #[test]
    fn test_save_and_load() {
        let mut chain = Blockchain::new(2);
        let (_alice, alice_addr) = wallet();
        let _ = chain.add_block(make_block(&chain, vec![], &alice_addr));

        chain.dump("test_chain.json").unwrap();
        let loaded = Blockchain::load("test_chain.json").unwrap();

        assert_eq!(chain.chain.len(), loaded.chain.len());
        assert_eq!(chain.compute_balances(), loaded.compute_balances());
        assert!(loaded.is_valid().is_ok());

        std::fs::remove_file("test_chain.json").ok();
    }
}
