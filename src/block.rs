use std::collections::HashMap;

use hex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use chrono::Utc;

use crate::transaction::Transaction;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Block {
    pub index: u32,
    pub timestamp: u64,
    pub transactions: Vec<Transaction>,
    pub hash: String,
    pub prev_hash: String,
    pub nonce: u64,
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
            hash: String::new(),
            prev_hash,
            nonce,
        };
        block.hash = block.calculate_hash();
        block
    }
    /// 计算区块哈希
    pub fn calculate_hash(&self) -> String {
        let mut hasher = Sha256::new();
        let tx_json: Vec<String> = self.transactions.iter().map(|tx| tx.to_string()).collect();
        let input = format!(
            "{}{}{}{}{}",
            self.index,
            self.timestamp,
            tx_json.join(","),
            self.prev_hash,
            self.nonce
        );
        hasher.update(input);
        hex::encode(hasher.finalize())
    }

    pub fn new(index: u32, transactions: Vec<Transaction>, prev_hash: String) -> Self {
        let timestamp = Utc::now().timestamp() as u64;
        let mut block = Block {
            index,
            timestamp,
            transactions,
            nonce: 0,
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
        println!("挖矿成功！nonce: {}, 哈希: {}", self.nonce, self.hash);
    }
}
#[derive(Debug, Clone)]
pub struct Blockchain {
    pub chain: Vec<Block>,             // 存储所有区块
    pub difficulty: usize,             // 统一挖矿难度
    pub balance: HashMap<String, u64>, // 余额
}
impl Blockchain {
    /// 初始化区块链：自动创建创世块
    pub fn new(difficulty: usize) -> Self {
        let genesis = Block::genesis();
        Blockchain {
            chain: vec![genesis],
            difficulty,
            balance: HashMap::new(),
        }
    }
    /// 获取链上最新的区块
    pub fn latest_block(&self) -> &Block {
        self.chain.last().unwrap()
    }
    /// 添加新区块到链上
    pub fn add_block(&mut self, transactions: Vec<Transaction>) {
        let prev_hash = self.latest_block().hash.clone();
        let index = self.latest_block().index + 1;
        // 1. 初始化新区块
        let mut new_block = Block::new(index, transactions, prev_hash);
        // 2. 挖矿（工作量证明）
        new_block.mine_block(self.difficulty);
        // 3. 把区块加到链上
        self.chain.push(new_block);
    }
    /// 验证整条链是否合法（核心逻辑：检测是否被篡改）
    pub fn is_valid(&self) -> bool {
        let prefix = "0".repeat(self.difficulty);
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
            if &current.hash[..self.difficulty] != prefix {
                println!("❌ 区块{}的工作量证明无效！", current.index);
                return false;
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
                *balances.entry(tx.sender.clone()).or_insert(0) -= tx.amount;
                *balances.entry(tx.receiver.clone()).or_insert(0) += tx.amount;
            }
        }
        balances
    }
}
#[cfg(test)]
mod tests {
    use crate::transaction::generate_wallet;

    use super::*;

    #[test]
    fn test_blockchain() {
        // 创建难度为4的区块链（哈希前4位是0，普通电脑几秒就能挖出来）
        let mut my_chain = Blockchain::new(4);
        println!("创世块生成完成，哈希: {}\n", my_chain.latest_block().hash);
        // 模拟账户
        let alice_wallet = generate_wallet();
        let alice_addr = hex::encode(alice_wallet.verifying_key().to_bytes());
        let bob_wallet = generate_wallet();
        let bob_addr = hex::encode(bob_wallet.verifying_key().to_bytes());
        let charlie_wallet = generate_wallet();
        let charlie_addr = hex::encode(charlie_wallet.verifying_key().to_bytes());
        // Air drop
        my_chain.balance.insert(alice_addr.clone(), 100);
        my_chain.balance.insert(bob_addr.clone(), 100);
        my_chain.balance.insert(charlie_addr.clone(), 100);

        // 模拟添加3个包含交易的区块
        my_chain.add_block(vec![Transaction::new(&alice_wallet, &bob_addr, 5)]);
        let balance = my_chain.compute_balances();
        assert_eq!(*balance.get(&alice_addr).unwrap(), 95);
        assert_eq!(*balance.get(&bob_addr).unwrap(), 105);
        assert_eq!(*balance.get(&charlie_addr).unwrap(), 100);

        my_chain.add_block(vec![Transaction::new(&bob_wallet, &charlie_addr, 10)]);
        let balance = my_chain.compute_balances();
        assert_eq!(*balance.get(&alice_addr).unwrap(), 95);
        assert_eq!(*balance.get(&bob_addr).unwrap(), 95);
        assert_eq!(*balance.get(&charlie_addr).unwrap(), 110);

        my_chain.add_block(vec![Transaction::new(&charlie_wallet, &alice_addr, 2)]);
        let balance = my_chain.compute_balances();
        assert_eq!(*balance.get(&alice_addr).unwrap(), 97);
        assert_eq!(*balance.get(&bob_addr).unwrap(), 95);
        assert_eq!(*balance.get(&charlie_addr).unwrap(), 108);
        // 打印整条链的信息
        println!("\n========== 当前链信息 ==========");
        for block in &my_chain.chain {
            println!(
                "索引: {} | 哈希: {} | 前哈希: {} | 交易: {:?}",
                block.index,
                block.hash[..16].to_owned() + "...",
                block.prev_hash[..16].to_owned() + "...",
                block.transactions
            );
        }
        // 验证链是否合法
        println!("\n========== 验证链合法性 ==========");
        println!("篡改前链是否合法: {}", my_chain.is_valid());
        assert!(my_chain.is_valid());
        // 模拟篡改第一个区块的交易（改成不同的金额，这样哈希必定改变）
        println!("\n========== 开始篡改第一个区块的交易 ==========");
        my_chain.chain[1].transactions = vec![Transaction::new(&alice_wallet, &bob_addr, 999)];
        // 篡改后重新计算该区块的哈希（模拟攻击者只改交易和哈希，不重新挖矿）
        my_chain.chain[1].hash = my_chain.chain[1].calculate_hash();
        println!("篡改后链是否合法: {}", my_chain.is_valid());
        assert!(!my_chain.is_valid());
    }
}
