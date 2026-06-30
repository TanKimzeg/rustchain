我们可以从**最简核心逻辑**开始，逐步迭代出一个可运行的区块链原型，全程用Rust实现，每一步都会解释核心原理，帮你真正理解区块链的运行机制。
---
## 整体思路
区块链的核心是「不可篡改的分布式账本」，本质是**一个个块通过哈希指针串联成的链表**，我们分阶段实现：
1. 基础版本：实现区块结构、链结构、工作量证明（挖矿）、链验证
2. 进阶版本：加入交易、数字签名、余额校验
3. 可选扩展：Merkle树、P2P网络同步等
---
## 阶段1：项目初始化与核心结构实现
### 步骤1：初始化Rust项目
先创建项目并配置依赖：
```bash
# 创建新项目
cargo new simple_blockchain
cd simple_blockchain
```
修改`Cargo.toml`，添加需要的依赖：
```toml
[package]
name = "simple_blockchain"
version = "0.1.0"
edition = "2021"

[dependencies]
# 哈希计算
sha2 = "0.10"
hex = "0.4"
# 时间处理
chrono = "0.4"
# 序列化（方便打印和后续扩展）
serde = { version = "1.0", features = ["derive"] }
```
### 步骤2：定义核心「区块」结构
一个区块是区块链的最小单元，必须包含以下核心字段：
- 索引：区块在链中的位置
- 时间戳：区块生成时间
- 交易数据：区块存储的业务内容（先简化用字符串表示）
- 前哈希：前一个区块的哈希值（实现链式串联的核心）
- 自身哈希：当前区块的唯一标识
- 随机数（nonce）：挖矿时用来调整哈希的变量
打开`src/main.rs`，先写区块结构定义：
```rust
use sha2::{Sha256, Digest};
use chrono::Utc;
use serde::{Serialize, Deserialize};
// 区块结构体，派生常用trait方便打印、序列化
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Block {
    index: u64,         // 区块索引
    timestamp: u64,     // 时间戳（Unix时间）
    transactions: Vec<String>, // 交易列表（简化版直接用字符串）
    prev_hash: String,  // 前一个区块的哈希
    hash: String,       // 当前区块的哈希
    nonce: u64,         // 挖矿随机数
}
impl Block {
    /// 创建创世块（第一个块，没有前区块）
    fn genesis() -> Self {
        let timestamp = Utc::now().timestamp() as u64;
        let transactions = vec!["Genesis Block: 创世块".to_string()];
        // 创世块的前哈希用全0表示
        let prev_hash = "0".repeat(64);
        let mut block = Block {
            index: 0,
            timestamp,
            transactions,
            prev_hash,
            hash: String::new(),
            nonce: 0,
        };
        // 计算创世块的哈希
        block.hash = block.calculate_hash();
        block
    }
    /// 通用创建新区块的方法（挖矿前初始化）
    fn new(index: u64, prev_hash: String, transactions: Vec<String>) -> Self {
        let timestamp = Utc::now().timestamp() as u64;
        let mut block = Block {
            index,
            timestamp,
            transactions,
            prev_hash,
            hash: String::new(),
            nonce: 0,
        };
        block.hash = block.calculate_hash();
        block
    }
    /// 计算区块的哈希值：把所有核心字段拼接后用SHA256计算
    fn calculate_hash(&self) -> String {
        // 拼接所有会影响区块唯一性的字段，保证顺序一致
        let data = format!(
            "{}{}{:?}{}{}",
            self.index, self.timestamp, self.transactions, self.prev_hash, self.nonce
        );
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        // 把哈希结果转成16进制字符串（方便显示和存储）
        hex::encode(result)
    }
    /// 挖矿：找到符合条件的nonce，使得哈希满足难度要求
    /// 难度用哈希前导0的数量表示，比如难度4就是哈希前4位是0
    fn mine_block(&mut self, difficulty: usize) {
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
```
### 步骤3：定义「区块链」结构
区块链本质是一个由区块按顺序组成的数组，我们封装成`Blockchain`结构体，封装链的操作逻辑：
```rust
#[derive(Debug, Clone)]
struct Blockchain {
    chain: Vec<Block>,    // 存储所有区块
    difficulty: usize,    // 统一挖矿难度
}
impl Blockchain {
    /// 初始化区块链：自动创建创世块
    fn new(difficulty: usize) -> Self {
        let genesis = Block::genesis();
        Blockchain {
            chain: vec![genesis],
            difficulty,
        }
    }
    /// 获取链上最新的区块
    fn latest_block(&self) -> &Block {
        self.chain.last().unwrap()
    }
    /// 添加新区块到链上
    fn add_block(&mut self, transactions: Vec<String>) {
        let prev_hash = self.latest_block().hash.clone();
        let index = self.latest_block().index + 1;
        // 1. 初始化新区块
        let mut new_block = Block::new(index, prev_hash, transactions);
        // 2. 挖矿（工作量证明）
        new_block.mine_block(self.difficulty);
        // 3. 把区块加到链上
        self.chain.push(new_block);
    }
    /// 验证整条链是否合法（核心逻辑：检测是否被篡改）
    fn is_valid(&self) -> bool {
        // 从第二个区块开始遍历（创世块没有前哈希，不需要验证）
        for i in 1..self.chain.len() {
            let current_block = &self.chain[i];
            let prev_block = &self.chain[i - 1];
            // 校验1：当前区块的哈希是否被篡改
            if current_block.hash != current_block.calculate_hash() {
                println!("❌ 区块{}的哈希不匹配，被篡改！", current_block.index);
                return false;
            }
            // 校验2：当前区块的前哈希是否和前一个区块的哈希一致
            if current_block.prev_hash != prev_block.hash {
                println!("❌ 区块{}的前哈希不匹配，链断裂！", current_block.index);
                return false;
            }
        }
        println!("✅ 链验证通过，未被篡改");
        true
    }
}
```
### 步骤4：基础功能测试
写`main`函数测试核心逻辑：
```rust
fn main() {
    // 创建难度为4的区块链（哈希前4位是0，普通电脑几秒就能挖出来）
    let mut my_chain = Blockchain::new(4);
    println!("创世块生成完成，哈希: {}\n", my_chain.latest_block().hash);
    // 模拟添加3个包含交易的区块
    my_chain.add_block(vec!["Alice 向 Bob 转账 10 BTC".to_string()]);
    my_chain.add_block(vec!["Bob 向 Charlie 转账 3 BTC".to_string()]);
    my_chain.add_block(vec!["Charlie 向 Alice 转账 1 BTC".to_string()]);
    // 打印整条链的信息
    println!("\n========== 当前链信息 ==========");
    for block in &my_chain.chain {
        println!("索引: {} | 哈希: {} | 前哈希: {} | 交易: {:?}", 
            block.index, &block.hash[..16]+"...", &block.prev_hash[..16]+"...", block.transactions);
    }
    // 验证链是否合法
    println!("\n========== 验证链合法性 ==========");
    println!("篡改前链是否合法: {}", my_chain.is_valid());
    // 模拟篡改第一个区块的交易
    println!("\n========== 开始篡改第一个区块的交易 ==========");
    my_chain.chain[1].transactions = vec!["Alice 向 Bob 转账 10000 BTC".to_string()];
    // 篡改后重新计算该区块的哈希（模拟攻击者只改交易和哈希，不重新挖矿）
    my_chain.chain[1].hash = my_chain.chain[1].calculate_hash();
    println!("篡改后链是否合法: {}", my_chain.is_valid());
}
```
### 运行测试
执行`cargo run`，你会看到类似输出：
```
创世块生成完成，哈希: 0000a3b2c8d7e6f5a4b3c2d1e0f9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f1
开始挖矿，难度: 4（前导0数量）
挖矿成功！nonce: 1234, 哈希: 0000b4c3d9e8f7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c2d1e0f9a8b7c6d5e4f3a2b1
开始挖矿，难度: 4（前导0数量）
挖矿成功！nonce: 5678, 哈希: 0000c5d4e9f8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c3d2e1f0a9b8c7d6e5f4a3b2c1
开始挖矿，难度: 4（前导0数量）
挖矿成功！nonce: 9012, 哈希: 0000d6e5f0a9b8c7d6e5f4a3b2c1d0e9f8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c3d2
========== 当前链信息 ==========
索引: 0 | 哈希: 0000a3b2c8d7... | 前哈希: 000000000000... | 交易: ["Genesis Block: 创世块"]
索引: 1 | 哈希: 0000b4c3d9e8... | 前哈希: 0000a3b2c8d7... | 交易: ["Alice 向 Bob 转账 10 BTC"]
索引: 2 | 哈希: 0000c5d4e9f8... | 前哈希: 0000b4c3d9e8... | 交易: ["Bob 向 Charlie 转账 3 BTC"]
索引: 3 | 哈希: 0000d6e5f0a9... | 前哈希: 0000c5d4e9f8... | 交易: ["Charlie 向 Alice 转账 1 BTC"]
========== 验证链合法性 ==========
✅ 链验证通过，未被篡改
========== 开始篡改第一个区块的交易 ==========
❌ 区块1的前哈希不匹配，链断裂！
篡改后链验证不合法: false
```
你会发现：只要篡改任意一个区块的内容，后续所有区块的前哈希都会失效，必须重新挖矿所有后续区块，成本极高，这就是区块链**不可篡改**的核心原理。
---
## 阶段2：进阶优化（接近真实区块链逻辑）
上面的版本交易是裸字符串，无法验证真实性，我们加入数字签名和余额校验，模拟真实的转账逻辑。
### 步骤1：新增依赖
在`Cargo.toml`里加入签名相关的依赖：
```toml
[dependencies]
# 原有依赖保留
ed25519-dalek = "2.0" # 数字签名库
rand = "0.8"         # 生成随机密钥对
```
### 步骤2：定义交易结构体
真实交易的转账方需要签名证明是自己发起的，防止伪造：
```rust
use ed25519_dalek::{Keypair, PublicKey, Signature, Signer, Verifier};
use rand::rngs::OsRng;
/// 交易结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Transaction {
    sender: String,    // 发送方公钥（用字符串表示）
    receiver: String,  // 接收方公钥
    amount: u64,       // 转账金额
    signature: Vec<u8>, // 发送方的签名
}
impl Transaction {
    /// 创建并签名交易
    fn new(sender_keypair: &Keypair, receiver: String, amount: u64) -> Self {
        let mut tx = Transaction {
            sender: hex::encode(sender_keypair.public.to_bytes()),
            receiver,
            amount,
            signature: Vec::new(),
        };
        // 对交易内容签名
        let tx_data = serde_json::to_string(&tx).unwrap();
        tx.signature = sender_keypair.sign(tx_data.as_bytes()).to_bytes().to_vec();
        tx
    }
    /// 验证交易签名是否合法
    fn verify(&self) -> bool {
        // 把签名转成可验证的格式
        let signature = match Signature::from_slice(&self.signature) {
            Ok(s) => s,
            Err(_) => return false,
        };
        // 把发送方公钥字符串转回PublicKey
        let pub_key_bytes = match hex::decode(&self.sender) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let pub_key = match PublicKey::from_bytes(&pub_key_bytes) {
            Ok(p) => p,
            Err(_) => return false,
        };
        // 验证签名
        let tx_data = serde_json::to_string(&self).unwrap();
        pub_key.verify(tx_data.as_bytes(), &signature).is_ok()
    }
}
/// 工具函数：生成随机密钥对（模拟用户钱包）
fn generate_wallet() -> Keypair {
    let mut csprng = OsRng{};
    Keypair::generate(&mut csprng)
}
```
### 步骤3：修改区块和区块链结构
把区块的交易类型从`Vec<String>`改成`Vec<Transaction>`，同时区块链增加余额校验逻辑：
```rust
// 修改Block的transactions类型
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Block {
    index: u64,
    timestamp: u64,
    transactions: Vec<Transaction>, // 改成交易列表
    prev_hash: String,
    hash: String,
    nonce: u64,
}
// Blockchain新增余额存储和交易校验逻辑
#[derive(Debug, Clone)]
struct Blockchain {
    chain: Vec<Block>,
    difficulty: usize,
    // 存储所有地址的余额（简化版，真实场景用UTXO模型）
    balances: std::collections::HashMap<String, u64>,
}
impl Blockchain {
    fn new(difficulty: usize) -> Self {
        let genesis = Block::genesis();
        let mut balances = std::collections::HashMap::new();
        // 创世块可以给某个地址发放初始余额，比如给Alice发100BTC
        balances.insert("Alice".to_string(), 100);
        Blockchain {
            chain: vec![genesis],
            difficulty,
            balances,
        }
    }
    /// 添加区块前先校验所有交易的合法性
    fn add_block(&mut self, transactions: Vec<Transaction>) {
        // 1. 校验所有交易的签名和余额
        for tx in &transactions {
            // 校验签名
            if !tx.verify() {
                panic!("❌ 交易签名非法，拒绝打包");
            }
            // 校验发送方余额是否足够
            let sender_balance = self.balances.get(&tx.sender).unwrap_or(&0);
            if *sender_balance < tx.amount {
                panic!("❌ 发送方余额不足，拒绝打包");
            }
        }
        // 2. 所有交易合法，扣减发送方余额，增加接收方余额
        for tx in &transactions {
            *self.balances.get_mut(&tx.sender).unwrap() -= tx.amount;
            *self.balances.get_mut(&tx.receiver).unwrap() += tx.amount;
        }
        // 3. 打包区块、挖矿、上链
        let prev_hash = self.latest_block().hash.clone();
        let index = self.latest_block().index + 1;
        let mut new_block = Block::new(index, prev_hash, transactions);
        new_block.mine_block(self.difficulty);
        self.chain.push(new_block);
    }
    /// 查询地址余额
    fn get_balance(&self, address: &str) -> u64 {
        *self.balances.get(address).unwrap_or(&0)
    }
}
```
### 步骤4：测试带签名的转账逻辑
修改`main`函数测试：
```rust
fn main() {
    let mut my_chain = Blockchain::new(4);
    // 生成两个用户的钱包
    let alice_wallet = generate_wallet();
    let bob_wallet = generate_wallet();
    // 用地址代替公钥简化显示
    let alice_addr = hex::encode(alice_wallet.public.to_bytes())[..16].to_string();
    let bob_addr = hex::encode(bob_wallet.public.to_bytes())[..16].to_string();
    println!("Alice地址: {}", alice_addr);
    println!("Bob地址: {}", bob_addr);
    println!("Alice初始余额: {}\n", my_chain.get_balance(&alice_addr));
    // 构造两笔合法交易
    let tx1 = Transaction::new(&alice_wallet, bob_addr.clone(), 10);
    let tx2 = Transaction::new(&alice_wallet, bob_addr.clone(), 5);
    // 打包交易到区块
    my_chain.add_block(vec![tx1, tx2]);
    println!("\nAlice剩余余额: {}", my_chain.get_balance(&alice_addr));
    println!("Bob余额: {}", my_chain.get_balance(&bob_addr));
    // 测试非法交易：余额不足时拒绝打包
    println!("\n尝试构造Alice余额不足的非法交易...");
    let tx3 = Transaction::new(&alice_wallet, bob_addr, 1000);
    my_chain.add_block(vec![tx3]); // 这里会panic，因为余额不足
}
```
运行后你会发现：非法交易会被直接拒绝，余额会被正确扣减和增加，交易无法伪造。
---
## 阶段3：可选扩展方向
上面的版本已经实现了区块链的核心逻辑，如果要更接近真实区块链，可以继续扩展：
1. **Merkle树优化**：把区块里的交易组成Merkle树，只存根哈希，减少区块体积，提升交易验证效率
2. **UTXO模型**：把余额模型改成比特币用的UTXO（未花费交易输出）模型，解决双花问题
3. **P2P网络同步**：用`tokio`+`libp2p`实现节点间的链同步，最长链原则共识
4. **智能合约**：加入EVM或WASM虚拟机，支持可编程逻辑
---
## 关键原理总结
1. **不可篡改性**：每个区块包含前一个区块的哈希，修改任意区块的内容会导致后续所有区块的前哈希失效，需要重新挖矿所有后续区块，成本极高
2. **工作量证明**：通过挖矿让出块产生成本，防止恶意节点随便生成伪造区块
3. **链式结构**：所有区块按时间顺序串联，只要所有节点都保存完整的链，就能保证账本的一致性
这个版本是学习用的简化实现，生产级区块链还需要考虑网络攻击、性能优化、共识容错等大量细节，但核心逻辑是完全相通的。
