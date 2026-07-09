# 从零构建区块链：Rust 实践教程

**这不是一本理论书，而是一段编程旅程。** 每个章节从一个"问题"出发——现有代码跑一下，发现它哪里不对，然后通过修改代码解决它。每一步做完 `cargo test` 都是绿的。

---

## 目录

1. [为什么需要区块链](#1-为什么需要区块链)
2. [区块——账本的一页](#2-区块账本的一页)
3. [工作量证明——让篡改有成本](#3-工作量证明让篡改有成本)
4. [交易——真正的转账](#4-交易真正的转账)
5. [余额推导——信任计算而不是存储](#5-余额推导信任计算而不是存储)
6. [Coinbase——钱从哪里来](#6-coinbase钱从哪里来)
7. [交易过滤——谁说了算](#7-交易过滤谁说了算)
8. [完整的链验证](#8-完整的链验证)
9. [Merkle Tree——轻量级验证](#9-merkle-tree轻量级验证)
10. [Mempool——交易的生命周期](#10-mempool交易的生命周期)
11. [手续费——经济激励](#11-手续费经济激励)
12. [难度调整——自适应](#12-难度调整自适应)
13. [持久化——让链活下来](#13-持久化让链活下来)
14. [测试的艺术](#14-测试的艺术)
15. [回顾与下一步](#15-回顾与下一步)
16. [Nonce——防重放攻击](#16-nonce防重放攻击)
17. [Miner 解耦——矿工独立循环](#17-miner-解耦矿工独立循环)
18. [HTTP API——与链交互](#18-http-api与链交互)

---

## 1. 为什么需要区块链

### 问题

Alice 想转 5 块钱给 Bob。最简单的方案：

```
Alice 在自己的账本上写：-5
Bob   在自己的账本上写：+5
```

问题：两个人各记各的账，Alice 可以说"我没转"，Bob 可以说"我没收到"。

### 传统方案

找一个双方都信任的第三方（银行）来记账：

```
银行账本：Alice -5, Bob +5
```

问题：银行可以被收买、被黑客攻击、或者服务器宕机。

### 区块链方案

**不信任任何一个人，但信任数学和规则**。核心思想：

1. **每个人都有一个账本副本**——没有单点故障
2. **账本以"区块"链接而成**——改了前面任何一页，后面所有页全部作废
3. **写入新页需要工作量证明**——改账需要付出真金白银的电费
4. **所有交易都有数字签名**——不能伪造他人转账

这本教程就是一步步实现这个系统。

---

## 2. 区块——账本的一页

### 概念

区块链 = 区块组成的链表。每个区块包含：

```
区块 1                区块 2                区块 3
┌─────────────┐       ┌─────────────┐       ┌─────────────┐
│ index: 0     │       │ index: 1     │       │ index: 2     │
│ timestamp    │───→   │ timestamp    │───→   │ timestamp    │
│ data: "A→B"  │       │ data: "B→C"  │       │ data: "C→A"  │
│ hash: 0x3f2a │       │ hash: 0x8d1e │       │ hash: 0x5c7b │
│ prev: 0x0000 │       │ prev: 0x3f2a │       │ prev: 0x8d1e │
└─────────────┘       └─────────────┘       └─────────────┘
```

关键：每个区块存了"前一个区块的哈希"。如果有人改了区块 1 的数据，区块 1 的哈希就变了，区块 2 里存的 `prev: 0x3f2a` 就对不上了——**链断裂**，任何人都能发现。

### 实现

```rust
struct Block {
    index: u32,          // 第几个块
    timestamp: u64,      // 创建时间
    transactions: Vec<Transaction>,
    hash: String,        // 本区块的哈希
    prev_hash: String,   // 前一个区块的哈希
    nonce: u64,          // 后面会用到
}

fn calculate_hash(&self) -> String {
    let input = format!("{}{}{}{}", self.index, self.timestamp,
                        self.transactions, self.prev_hash);
    sha256(input)
}
```

### 验证

```rust
fn is_valid(&self) -> bool {
    for i in 1..self.chain.len() {
        // 哈希必须对得上（没被篡改）
        if chain[i].hash != chain[i].calculate_hash() { return false; }
        // 链不能断
        if chain[i].prev_hash != chain[i-1].hash { return false; }
    }
    true
}
```

### 学到了什么

- **哈希函数**是区块链的"胶水"——它把独立的数据块粘成一条不可篡改的链
- **链式结构**不是存储优化，而是安全设计——你不需要信任任何人，只需要自己重新算一遍哈希就能验证整条链

---

## 3. 工作量证明——让篡改有成本

### 问题

上一步的 `is_valid` 能检测篡改。但攻击者可以：

```
篡改区块 1 → 重新计算 hash → 更新区块 2 的 prev_hash
          → 重新计算 hash → 递归更新所有后面的区块
```

如果只是"哈希匹配 + 链不断"，攻击者可以把整条链重算一遍，篡改完全检测不出来。

### 解决方案：PoW（Proof of Work）

要求区块的哈希必须以 N 个 `0` 开头（比如 `0000a1f2...`）。怎么做到的？调整 `nonce` 字段：

```rust
fn mine_block(&mut self, difficulty: usize) {
    let prefix = "0".repeat(difficulty);
    while &self.calculate_hash()[..difficulty] != prefix {
        self.nonce += 1;  // 碰运气
    }
    self.hash = self.calculate_hash();
}
```

难度 4 意味着只有 1/65536 的哈希合格。挖矿需要大量尝试，但验证只需一次计算：

```rust
// 验证（瞬间完成）
chain[i].hash.starts_with("0000")
```

### 为什么这叫"工作量证明"

- **挖矿难**：找符合要求的 nonce 需要大量计算（证明你花了电费）
- **验证易**：看一眼就知道对不对
- **篡改成本**：攻击者篡改一个块后，必须重新挖这个块和后面所有块——成本随链长指数增长

### 学到了什么

- PoW 不是为了让挖矿"公平"，而是为了让**篡改成本 > 篡改收益**
- **不对称性**（挖难验易）是所有 PoW 区块链安全的基础
- 这也解释了为什么 PoW 公链（Bitcoin、以太坊 1.0）的能耗那么高——那是安全预算

---

## 4. 交易——真正的转账

### 问题

现在区块里存的是 `Vec<String>`（字符串），任何人都可以写 `"Alice → Bob 5"`。怎么证明这条指令真的是 Alice 发出的？

### 解决方案：数字签名

每个用户有一个密钥对：

```
私钥（自己藏好）→ 签名交易
公钥（公开）   → 别人可以验证签名，但不能伪造
```

```rust
struct Transaction {
    sender: String,     // 发送方公钥（hex 编码）
    receiver: String,   // 接收方公钥（hex 编码）
    amount: u64,        // 转账金额
    signature: Vec<u8>, // 发送方的签名
    fee: u64,           // 手续费（后面会讲）
}
```

### 签名排除自身（最常见的坑）

```rust
// 签名时——signature 是空的
let tx_data = serde_json::to_string(&tx);  // {"signature":[], ...}
tx.signature = sign(tx_data);

// 验证时——signature 已经填满了
let tx_data = serde_json::to_string(&tx);  // {"signature":[xxx,...], ...}
// ↑ 跟签名时的数据不一样！验证永远失败！
```

**解法**：签名时只签 `sender + receiver + amount`，排除 `signature` 字段自身。

```rust
fn serialize_for_signing(&self) -> String {
    serde_json::json!({
        "sender": self.sender,
        "receiver": self.receiver,
        "amount": self.amount,
        "fee": self.fee,
    }).to_string()
}
```

### Coinbase 特殊处理

区块第一笔交易是 coinbase（系统给矿工的奖励）。它不需要签名：

```rust
fn verify(&self) -> bool {
    if self.sender == COINBASE_ADDR { return true; }
    // 正常验签逻辑...
}
```

### 为什么用 ed25519-dalek 2.x 而不是 1.x

ed25519-dalek 2.x 相比 1.x 有几次改名：

| 1.x                 | 2.x                                         |
| ------------------- | ------------------------------------------- |
| `PublicKey`         | **`VerifyingKey`**                          |
| `Keypair`           | **`SigningKey`**                            |
| `kp.public`         | **`signing_key.verifying_key()`**           |
| `from_bytes(&[u8])` | **`from_bytes(&[u8; 32])`**（需要定长数组） |

如果你搜到的教程用了旧 API，记得对照着改。

### 学到了什么

- **数字签名就是法律上的"亲笔签名"**——无法伪造、无法抵赖
- **签名排除自身**是一个容易犯但一旦理解就永远不会再犯的设计模式
- 2.x API 的 `from_bytes` 要求 `[u8; 32]` 而不是 `&[u8]`——反映了"公钥必须是 32 字节"的类型约束

---

## 5. 余额推导——信任计算而不是存储

### 问题

有了签名交易，但余额怎么算？一种做法：

```rust
// 每笔交易发生时直接更新余额
self.balance[sender] -= amount;
self.balance[receiver] += amount;
```

问题：如果有人篡改了 `balance` 字段怎么办？信任存储在内存里的这个 HashMap？

### 解法的核心思想

**余额不是存储的，是计算出来的**。任何时候、任何节点，只要拿到整条链，就能独立算出余额——不需要信任任何人给的账本。

```rust
fn compute_balances(&self) -> HashMap<String, u64> {
    let mut balances = self.balance.clone();  // 当前快照
    for block in &self.chain {
        for tx in &block.transactions {
            if tx.sender != COINBASE_ADDR {
                // 发送方扣钱
                *balances.entry(tx.sender.clone()).or_insert(0) -= tx.amount;
            }
            *balances.entry(tx.receiver.clone()).or_insert(0) += tx.amount;
        }
    }
    balances
}
```

每次 `add_block` 后断言：

```rust
assert_eq!(my_chain.balance, my_chain.compute_balances());
```

两边对不上 → 要么 balance 被篡改，要么 compute_balances 有 bug。

### 更深层的含义

不管是 Bitcoin 的 UTXO 模型还是 Ethereum 的账户模型，核心是一样的：**状态从链上推导，不信任本地缓存**。

Bitcoin 节点启动时重放所有交易重建 UTXO 集，Ethereum 节点重放所有交易重建账户状态树——叫法不同，本质相同。两个节点同步时，不需要互相相信"我的余额是 100"，只需要检查对方的链是否跟自己一样，然后各自独立算出余额。

**信任计算过程，而不是信任存储结果**——这是区块链区别于传统数据库的根本。

### 学到了什么

- **状态可推导**是区块链的去中心化基础
- 纯函数（从链到余额的映射）比可变状态更容易测试、更容易理解
- 这也解释了为什么全节点需要存整条链——没有链就推不出余额

---

## 6. Coinbase——钱从哪里来

### 问题

测试里是这样造钱的：

```rust
my_chain.balance.insert(alice_addr, 100);  // 空投
```

这是作弊。在真实区块链中只有一种方式产生新钱：**挖矿奖励**（或 PoS 的质押奖励）。没有 pre-mine，没有 ICO，每一枚新币都来自共识机制。

### 解决方案

删掉空投。每个区块的第一笔交易是 coinbase，奖励给挖出这个区块的矿工：

```rust
fn add_block(&mut self, transactions: Vec<Transaction>, miner: &str) {
    // 计算区块内所有交易的手续费总和
    let total_fees: u64 = transactions.iter().map(|tx| tx.fee).sum();

    let coinbase = Transaction {
        sender: COINBASE_ADDR.to_string(),
        receiver: miner.to_string(),
        amount: REWARD + total_fees,  // 基础奖励 + 手续费
        signature: String::new(),
        fee: 0,
    };

    let mut all_txs = vec![coinbase];
    all_txs.extend(transactions);
    // ... 挖矿、上链
}
```

同时调整：

- `compute_balances` 跳过 coinbase 的扣钱（`"COINBASE"` 没有余额，不减）
- `verify` 对 coinbase 直接返回 `true`
- 测试里完全移除 `balance.insert`，所有钱来自挖矿

### 为什么叫 coinbase

Coinbase 交易没有发送方，它凭空创造新币。这也是"加密货币"名字的由来：**币是从区块里挖出来的，不是预分配的**。Ethereum 的每个区块也有类似的"矿工奖励"交易，只不过它使用账户模型直接更新矿工余额。

### 学到了什么

- **去中心化货币发行的唯一入口就是 coinbase**——没有 ICO，没有预挖
- 这也解释了**通胀设计**——coinbase 金额决定了新币的发行速度，可通过减半等机制预设上限
- Coinbase 同时解决了"为什么矿工要挖矿"的经济激励问题

---

## 7. 交易过滤——谁说了算

### 问题

现在任何人都可以调用 `add_block` 传交易——余额不足的、签名伪造的、发送方不存在的。谁来把关？

### 答案：每个人自己把关

每个节点独立过滤。收到的交易先过一遍 `filter_valid_txs`，只有合法的才打包：

```rust
fn filter_valid_txs(&self, txs: Vec<Transaction>) -> (Vec<Transaction>, Vec<Transaction>) {
    let mut valid = Vec::new();
    let mut invalid = Vec::new();
    let mut sim_balances = self.compute_balances();

    for tx in txs {
        if tx.sender == COINBASE_ADDR {
            // coinbase 凭空造钱，跳过余额校验
            *sim_balances.entry(tx.receiver.clone()).or_insert(0) += tx.amount;
            valid.push(tx);
        } else if tx.verify()
            && sim_balances.get(&tx.sender).copied().unwrap_or(0) >= tx.amount + tx.fee
        {
            // 逐笔模拟：先扣后加
            *sim_balances.get_mut(&tx.sender).unwrap() -= tx.amount + tx.fee;
            *sim_balances.entry(tx.receiver.clone()).or_insert(0) += tx.amount;
            valid.push(tx);
        } else {
            invalid.push(tx);
        }
    }
    (valid, invalid)
}
```

### 为什么是逐笔模拟，而不是批量查余额

因为区块内的交易是**顺序依赖**的——coinbase 在这个块里创造了 50 块，同块里的下一笔交易才能花这 50 块。

```
区块 3（当前正在打包，还没上链）：
  tx 1: coinbase → Alice +50
  tx 2: Alice → Bob -15     ← 这取决于 tx1 是否已被处理
```

`compute_balances()` 只看**已上链**的区块，看不到当前正在打包的这批交易。所以必须逐笔模拟，同区块内交易之间共享余额状态。

### 学到了什么

- **去中心化的核心就是每个节点做一模一样的校验逻辑**
- 交易不是独立存在的——同区块内的交易有**顺序依赖**
- 这也解释了为什么区块内的交易顺序是确定的（一旦上链就不可更改）

---

## 8. 完整的链验证

### 问题

`is_valid` 之前只检查哈希和链完整性。但如果有人构造了一个哈希正确、链连着、但签名全是伪造的区块呢？或者 coinbase 放了 10 笔而不是 1 笔？

### 完整校验清单

```rust
fn is_valid(&self) -> bool {
    for i in 1..self.chain.len() {
        let current = &self.chain[i];
        let prev = &self.chain[i - 1];

        // 校验1：哈希没有被篡改
        if current.hash != current.calculate_hash() { return false; }

        // 校验2：链没有断裂
        if current.prev_hash != prev.hash { return false; }

        // 校验3：工作量证明（用区块自己的难度）
        let block_prefix = "0".repeat(current.mined_difficulty);
        if &current.hash[..current.mined_difficulty] != block_prefix { return false; }

        // 校验4：有且仅有一笔 coinbase，且是第一笔
        if current.transactions.is_empty()
            || current.transactions[0].sender != COINBASE_ADDR { return false; }
        if current.transactions[1..].iter().any(|tx| tx.sender == COINBASE_ADDR) {
            return false;
        }

        // 校验5：每笔交易签名有效
        for tx in &current.transactions {
            if !tx.verify() { return false; }
        }
    }
    true
}
```

### 每个校验对应一种攻击

| 校验          | 防御的攻击         |
| ------------- | ------------------ |
| 哈希匹配      | 篡改交易数据       |
| 前哈希匹配    | 插入/删除/重排区块 |
| 工作量证明    | 快速重算整个链     |
| coinbase 结构 | 伪造货币发行       |
| 签名验证      | 冒用他人身份转账   |

### 学到了什么

- 链验证不是一道简单的"哈希对上就行"
- 每一条规则对应一种具体的攻击场景
- 安全性来自多层防守，不是单一机制

---

## 9. Merkle Tree——轻量级验证

### 问题

现在区块哈希是这样算的：

```rust
hash = sha256(index + timestamp + txs_json + prev_hash + nonce)
```

其中 `txs_json` 是所有交易的完整列表。这意味着：

- 验证一笔交易是否在区块中，必须下载整个区块的所有交易
- 轻量级节点（手机钱包）无法参与验证

### 解决方案

把 `txs_json` 换成 **merkle_root**——交易的两两哈希树根：

```
叶子层：    tx1          tx2          tx3          tx4
           │            │            │            │
哈希层：  hash(tx1)    hash(tx2)    hash(tx3)    hash(tx4)
            \          /              \          /
层1：     hash(h1+h2)              hash(h3+h4)
              \                      /
根：         hash(h12 + h34) = merkle_root
```

```rust
pub fn compute_merkle_root(txs: &[Transaction]) -> String {
    if txs.is_empty() { return String::new(); }

    let mut layer: Vec<String> = txs.iter()
        .map(|tx| sha256(tx.to_string()))
        .collect();

    while layer.len() > 1 {
        if layer.len() % 2 == 1 {
            layer.push(layer.last().unwrap().clone()); // 奇数时复制最后一个
        }
        layer = layer.chunks(2).map(|pair| {
            sha256(format!("{}{}", pair[0], pair[1]))
        }).collect();
    }
    layer.into_iter().next().unwrap()
}
```

### SPV 验证

有了 Merkle Tree，证明一笔交易在区块中只需要：

```
给你 tx3、以及 [hash(tx4), hash(h12)]
你算出：hash(tx3) → 跟 hash(tx4) 配对 → hash(h34)
      → 跟 hash(h12) 配对 → merkle_root
      → 跟区块头里的 merkle_root 对比
```

**不需要下载 tx1、tx2、tx4**。这就是手机钱包能在只下载区块头的情况下验证交易的原因。

### 边界情况

- 0 笔交易 → 返回空字符串
- 1 笔交易 → 直接返回这笔交易的哈希
- 奇数笔 → 复制最后一笔凑成偶数

### 学到了什么

- Merkle Tree 把验证成本从 O(n) 降到 O(log n)
- 它是"轻客户端"存在的基础——没有 Merkle Tree 就没有 SPV
- 理解 Merkle Tree 就理解了 Bitcoin 白皮书里"Simplified Payment Verification"那一章

---

## 10. Mempool——交易的生命周期

### 问题

目前 `add_block` 直接接收交易列表，但现实世界不是这样的。用户随时发起交易，矿工隔一段时间打包一批——交易产生和区块生成是**异步**的。

### 解决方案：Mempool（交易池）

```
用户创建交易 → submit 到 mempool
              ↓
mempool 按手续费排序等待
              ↓
矿工定时从 mempool select 一批交易 → add_block
              ↓
已上链的交易从 mempool remove
```

```rust
pub struct MemeryPool {
    pub candidate: HashSet<Transaction>,
}

impl MemeryPool {
    pub fn submit(&mut self, tx: Transaction) -> Result<(), String> {
        if tx.verify() {
            self.candidate.insert(tx);
            Ok(())
        } else {
            Err("Invalid Transaction".to_string())
        }
    }

    pub fn select(&self, count: usize) -> Vec<Transaction> {
        let mut txs: Vec<_> = self.candidate.iter().cloned().collect();
        txs.sort_by(|a, b| b.fee.cmp(&a.fee));  // 高手续费优先
        txs.into_iter().take(count).collect()
    }

    pub fn remove(&mut self, txs: &[Transaction]) {
        txs.iter().for_each(|tx| { self.candidate.remove(tx); });
    }
}
```

### 完整流程（集成测试）

```rust
// 1. Alice 先挖一个块获得初始资金
chain.add_block(vec![], &alice_addr);

// 2. 用户提交交易到 mempool
let mut pool = MemeryPool::new();
pool.submit(Transaction::new(&alice, &bob_addr, 20, 1)).unwrap();

// 3. 矿工从 mempool 挑交易打包
let txs = pool.select(10);
chain.add_block(txs, &miner_addr);

// 4. 已上链的从池子里清掉
pool.remove(&chain.latest_block().transactions[1..]);
```

### 学到了什么

- Mempool 是连接"交易产生"和"区块打包"的桥梁
- 它引入了一种**异步消息模型**——用户和矿工不需要同步
- 按 fee 排序的 select 机制决定了矿工的收入——这也是为什么 Bitcoin、Ethereum 等公链存在手续费市场
- `remove` 操作比看起来更重要：如果不清理，mempool 会无限膨胀，变成 DoS 攻击面

---

## 11. 手续费——经济激励

### 概念

每个交易可以附带一笔 **fee**（手续费）。矿工打包时会优先选 fee 高的交易。

```
区块奖励 = REWARD + 区块内所有交易的 fee 总和
```

```rust
// add_block 里汇总手续费
let total_fees: u64 = transactions.iter().map(|tx| tx.fee).sum();
let coinbase_amount = REWARD + total_fees;
```

### 为什么手续费要签进交易里

```rust
fn serialize_for_signing(&self) -> String {
    serde_json::json!({
        "sender":   self.sender,
        "receiver": self.receiver,
        "amount":   self.amount,
        "fee":      self.fee,    // ← 必须签进去
    }).to_string()
}
```

如果不签，攻击者可以拦截你的交易，把 `fee: 1` 改成 `fee: 0`，你的交易就永远排在后面。

### 经济学含义

- **手续费 = 优先权**——着急就多给，不着急就少给
- **手续费 = 安全预算**——总的交易费越高，矿工越有动力维护网络
- **区块奖励衰减后，手续费将成为矿工的主要收入**

### 学到了什么

- 手续费不是附属品，它是区块链可持续运行的经济基础
- 把 fee 签进交易是安全设计，不是多此一举
- 你做的 `fee` 字段和 mempool 的按 fee 排序，构成了一个**微型市场**

---

## 12. 难度调整——自适应

### 问题

当前难度是写死的 `difficulty: usize`。问题是：

- 如果更多人加入挖矿 → 出块变快 → 链分叉概率增加
- 如果矿工离开 → 出块变慢 → 交易确认需要等很久

Bitcoin 的目标是每 10 分钟一个块。如果算力变化了，难度必须跟着变。

### 解决方案

每 N 个块检查一次出块时间，快了就加难度，慢了就减难度：

```rust
fn adjust_difficulty(&mut self) {
    let interval = self.adjustment_interval;
    if self.chain.len() % interval as usize != 0 { return; }

    let start = self.chain.len() - interval as usize;
    let actual = self.chain.last().unwrap().timestamp
               - self.chain[start].timestamp;
    let target = self.target_block_time * interval as u64;

    if actual < target / 2 {
        self.difficulty += 1;
    } else if actual > target * 2 {
        self.difficulty = difficulty.saturating_sub(1);
    }
}
```

### 每个区块记录自己的难度

关键设计：**区块头里存 `mined_difficulty`**，记录这个块是以什么难度挖出来的。

```rust
pub struct Block {
    // ...其他字段...
    pub mined_difficulty: usize,  // 这个块的挖矿难度
}
```

验证时用区块自己的难度，而不是当前难度：

```rust
// ✅ 正确：区块 1 用难度 2 挖的，就用 2 来验
let block_prefix = "0".repeat(current.mined_difficulty);
if &current.hash[..current.mined_difficulty] != block_prefix {
    return false;
}
```

如果不这样，假设难度从 2 升到了 4，那么用当前难度 4 去验区块 1（只有 2 个前导 0）就会报"无效"——但区块 1 明明没问题。

### 参数的取值

- Bitcoin：`target_block_time = 600` 秒，`adjustment_interval = 2016` 块
- 本教程的测试：`target_block_time = 2` 秒，`adjustment_interval = 3` 块
- 测试用 3 块就调一次，方便快速验证

### 学到了什么

- 难度调整是区块链**自我调节能力**的体现——不需要硬分叉，不需要人工干预
- 每个区块记录自己的难度是"**链上状态**"思维——一切验证所需的信息都在链上，不需要外部输入
- 这也展示了区块链的**进化能力**——协议可以在参数层面自适应，不用改代码

---

## 13. 持久化——让链活下来

### 问题

所有数据在内存里，程序一退出一切消失。

### 解决方案：序列化到文件

因为所有核心结构都 derive 了 `Serialize/Deserialize`，序列化是一行代码的事：

```rust
impl Blockchain {
    pub fn save(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        let chain = serde_json::from_str(&json)?;
        Ok(chain)
    }
}
```

测试：

```rust
chain.save("test_chain.json").unwrap();
let loaded = Blockchain::load("test_chain.json").unwrap();
assert_eq!(chain.compute_balances(), loaded.compute_balances());
assert!(loaded.is_valid());
```

### 学到了什么

- **Serde 的设计哲学**——derive 宏自动生成序列化代码，零运行时反射
- 持久化不是区块链特有的问题，但**先做再优化**——JSON 简单、可读、好调试，以后可以换二进制格式
- `to_string_pretty` 不是性能最优的，但它让你能直接看文件内容来 debug

---

## 14. 测试的艺术

### 原则

1. **一个测试只测一件事**
2. **测试名描述场景，而不是实现**
3. **用帮助函数消除重复**
4. **`cargo test` 必须每次都能绿**

### 反模式（我们早期犯的错）

```rust
#[test]
fn test_blockchain() {
    // 50 行代码，测试了：创世块、上链、余额、篡改、mempool
    // 失败时只知道 "test_blockchain FAILED"
    // 完全不知道是哪个场景出了问题
}
```

### 正确做法

```rust
#[test]
fn test_genesis() { ... }

#[test]
fn test_add_block_updates_balance() { ... }

#[test]
fn test_detect_tampered_transactions() { ... }

#[test]
fn test_detect_broken_link() { ... }

#[test]
fn test_detect_invalid_pow() { ... }

// 运行结果：
// test_detect_tampered_transactions ... FAILED
// ↑ 一眼知道哪个场景出了问题
```

### 帮助函数

```rust
fn wallet() -> (SigningKey, String) {
    let w = generate_wallet();
    let addr = hex::encode(w.verifying_key().to_bytes());
    (w, addr)
}

fn chain_with_three_blocks() -> (Blockchain, SigningKey, String, /* ... */) {
    // 创建一个预填充 3 个块的链，供多个测试复用
}
```

### 测试分布

| 模块             | 测试数 | 覆盖内容                                                                          |
| ---------------- | ------ | --------------------------------------------------------------------------------- |
| `transaction.rs` | 4      | 验签、coinbase、篡改拒绝、签名排除                                     |
| `merkle.rs`      | 4      | 空列表、长度、确定性、单双不同                                          |
| `mempool.rs`     | 9      | 池操作(5) + Miner 组装区块(3) + start_new(1)                            |
| `block.rs`       | 8      | 创世块、余额、链有效、篡改/断裂/PoW 检测、全流程、持久化               |
| `api.rs`         | 5      | 链查询、余额、无效签名拒绝、提交+查池流程                               |
| **总计**         | **30** |                                                                         |

### 学到了什么

- 测试不是"能跑就行"——它们是**活的文档**
- 一个好的测试名 = 一个功能场景的描述
- 帮助函数隔离了重复设置代码，让每个测试的"灵魂"（断言）更突出

---

## 15. 回顾与下一步

### 你走过的路

```
原始区块 ─→ PoW 挖矿 ─→ 签名交易 ─→ 余额推导
    │                                        │
    ├── Coinbase                              │
    │                                        │
    ├── 交易过滤 ─→ 链完整验证                 │
    │                                        │
    ├── Merkle Tree ─→ Mempool                │
    │                                        │
    ├── 手续费 ─→ 难度调整 ─→ 持久化           │
    │                                        │
    ├── Nonce 防重放 ─→ 状态推导重构            │
    │                                        │
    ├── Miner 解耦 ─→ HTTP API                │
    │                                        │
    └── 模块化测试 ←───────────────────────────┘
```

### 你现在理解的概念

| 概念         | 对应实际代码                         |
| ------------ | ------------------------------------ |
| 哈希链       | `Block { prev_hash, hash }`          |
| 工作量证明   | `mine_block()` + `is_valid` PoW 校验 |
| 数字签名     | `Transaction::verify()`              |
| 状态可推导   | `compute_balances()`                 |
| 去中心化发行 | Coinbase 交易                        |
| 交易验证     | `filter_valid_txs()`                 |
| 轻客户端     | Merkle Tree                          |
| 交易市场     | Mempool + fee 排序                   |
| 经济激励     | Coinbase = REWARD + fees             |
| 自我调节     | `adjust_difficulty()`                |
| 持久化       | `save()` / `load()`                  |
| 防重放       | `nonce` + `get_tx_count()`           |
| 关注点分离   | `Blockchain` / `Miner` 解耦           |
| RPC 接口     | axum HTTP API                        |

**这些就是主流公链核心机制的 80%——无论 Bitcoin 的 UTXO 还是 Ethereum 的账户模型，底层都是这些构件。**

### 接下来可以做什么

- **P2P 网络** — 多节点同步、分叉处理、共识协议
- **UTXO 模型** — 改用 inputs/outputs 替代 account 模型。Bitcoin 用 UTXO，Ethereum 用账户——两种设计各有优劣，值得都试一遍
- **智能合约** — 在交易里嵌入可执行脚本
- **钱包生成** — 助记词、BIP32 分层确定性钱包

---

## 16. Nonce——防重放攻击

### 问题：Bob 收到了两笔钱

当前交易只有 `sender / receiver / amount / fee / signature`。假设：

1. Alice 签了一笔交易：`Alice → Bob, 10 元`
2. 矿工打包进区块，Alice 扣了 10 元
3. Bob **拿到这笔交易原文**，重新广播到网络
4. 矿工看到签名合法、Alice 余额够，**又打包一次**
5. Alice 再扣 10 元，Bob 再收 10 元

Bob 可以无限重放，直到 Alice 没钱。这就是**重放攻击**。

### 解决方案：Nonce

每个账户维护一个计数器，每发一笔交易就 +1。交易里带上当前的 nonce，节点校验：

```
tx.nonce == account_nonce[sender]  → 接受，然后 account_nonce[sender]++
tx.nonce != account_nonce[sender]  → 拒绝（过时或乱序）
```

重放攻击的那笔交易 nonce 已经用过了，自然被拒绝。

### 改动

**Transaction 结构体新增 `nonce` 字段：**

```rust
pub struct Transaction {
    pub sender: String,
    pub receiver: String,
    pub amount: u64,
    pub signature: String,
    pub fee: u64,
    pub nonce: u64,  // ← 新增
}
```

**Nonce 必须纳入签名范围**，否则攻击者可以改 nonce 重新签名：

```rust
pub fn serialize_for_signing(&self) -> String {
    serde_json::json!({
        "sender":   self.sender,
        "receiver": self.receiver,
        "amount":   self.amount,
        "fee": self.fee,
        "nonce": self.nonce,  // ← 关键！不签名 = nonce 可被篡改
    })
    .to_string()
}
```

**`Transaction::new_coinbase()` 统一创建 coinbase：**

```rust
pub fn new_coinbase(miner_addr: &str, fees: u64) -> Self {
    Self {
        sender: COINBASE_ADDR.to_string(),
        receiver: miner_addr.to_string(),
        amount: REWARD + fees,
        signature: String::new(),
        fee: 0,
        nonce: 0,
    }
}
```

矿工奖励构造从多处手动 `Transaction { ... }` 集中到了一处。

### 链上 nonce 状态推导

类似余额推导，nonce 状态也从链上所有交易重推：

```rust
pub fn get_tx_count(&self) -> Result<HashMap<String, u64>, String> {
    let mut tx_count = HashMap::new();
    for block in &self.chain {
        for tx in &block.transactions {
            if tx_count.get(&tx.sender).unwrap_or(&0u64) == &tx.nonce {
                *tx_count.entry(tx.sender.clone()).or_insert(0u64) += 1;
            } else if tx.sender != COINBASE_ADDR {
                return Err(format!("nonce 不连续"));
            }
        }
    }
    Ok(tx_count)
}
```

这段代码判断逻辑很直白：对每个 sender，当前链上记录的 nonce 应该是 `0, 1, 2, 3, ...`。如果某笔交易的 nonce 不等于预期的下一个值，说明这条链上有 nonce 冲突。

### `is_valid` 同时检查余额和 nonce

```rust
pub fn is_valid(&self) -> Result<(), String> {
    for i in 1..self.chain.len() {
        self.check_block(&self.chain[i])?;
    }
    let _balances = self.compute_balances()?;
    let _tx_count = self.get_tx_count()?;
    Ok(())
}
```

余额 + nonce 双维度校验，缺一不可。

### 学到了什么

- **防重放靠状态机，不是靠签名**——签名只证明"是你发的"，不证明"你发过几次"。nonce 跟踪的是**状态**。
- **Nonce 必须被签名**——否则攻击者可以改了 nonce 重新广播，绕过 nonce 检查。
- `Result` 比 `bool` 更适合 `is_valid`——调用方直接知道"哪里坏了"而不是"就是坏了"。
- 状态推导是不可变的——链是事实来源，计算是纯函数，没有"缓存不一致"的问题。

---

## 17. Miner 解耦——矿工独立循环

### 问题：链不该挖矿

最初的代码里，`Blockchain` 做了所有事：

```rust
chain.add_block(txs, &miner_addr);
    // → 创建 coinbase
    // → 过滤交易
    // → 计算 Merkle root
    // → 挖矿（PoW 循环！）
    // → 上链
    // → 调整难度
```

链的逻辑和矿工的逻辑混在一起。但现实中，**链只关心"收到合法区块 → 追加到链尾"**，至于这个块是挖出来的还是别人广播的，链不需要知道。

### 解耦后的接口

**`Blockchain` 只做校验和上链：**

```rust
pub fn add_block(&mut self, block: Block) -> Result<(), String> {
    self.check_block(&block)?;
    self.chain.push(block);
    self.adjust_difficulty();
    Ok(())
}
```

**`Miner` 负责组装区块和挖矿：**

```rust
pub struct Miner {
    pub address: String,
    pub pool: Arc<Mutex<MemeryPool>>,
    pub chain: Arc<Mutex<Blockchain>>,
    pub tx_count: Arc<Mutex<HashMap<String, u64>>>,
}

impl Miner {
    pub fn assemble_block(&self) -> Block {
        // 1. 从交易池选交易
        let txs = self.pool.lock().unwrap().select(10);
        // 2. 创建 coinbase
        let coinbase = Transaction::new_coinbase(&self.address, fees);
        // 3. 过滤无效交易（含 nonce 检查）
        let (valid_txs, _) = self.filter_valid_txs(all_txs);
        // 4. 创建区块、挖矿
        let mut block = Block::new(...);
        block.mine_block(difficulty);
        block
    }

    pub fn start_mining_loop(&self) {
        // 后台线程：5 秒循环挖矿
        std::thread::spawn(move || loop {
            let block = miner.assemble_block();
            miner.chain.lock().unwrap().add_block(block).ok();
            std::thread::sleep(Duration::from_secs(5));
        });
    }
}
```

### 数据流

```
用户提交 tx → MemeryPool
                  ↓
Miner.select(10) → 组装区块 → filter_valid_txs（余额+nonce）
                  ↓
Block.new → mine_block（PoW）
                  ↓
Blockchain.add_block（校验+上链）
                  ↓
Miner 从 pool 移除已上链 tx
```

### 关键设计

- **`Miner::assemble_block()` 不依赖外部参数**——它自己持有 pool、chain、nonce 状态的 `Arc` 引用，自包含
- **`filter_valid_txs` 移到 Miner**——因为"什么交易合法"是矿工决定的，不是链决定的
- **`start_new` 一键创建运行中的矿工**——同时初始化随机地址、空交易池、启动后台挖矿

### 学到了什么

- **关注点分离**：链 = 存储 + 验证，矿工 = 组装 + 计算。两个职责，两个结构体。
- **共享状态用 `Arc<Mutex<>>`**——矿工和 API 共享同一个链和交易池实例，这是 Rust 给多线程安全上的约束。
- **矿工决定交易有效性**——`filter_valid_txs` 在 Miner 里而不是 Blockchain 里，因为不同矿工可能有不同的交易过滤策略。

---

## 18. HTTP API——与链交互

### 问题：只能通过测试和代码操作

现在链、交易池、矿工都写好了，但只能通过 Rust 代码或测试来使用。要让普通人（和其他程序）能用，需要一个 HTTP 接口。

### 技术选型

- **axum 0.8** — Rust 生态最主流的 async web 框架之一
- **tokio** — Rust 异步运行时，axum 基于它
- **`Arc<Mutex<>>`** — 共享链和矿工状态

### 项目结构

```
src/
├── main.rs       # HTTP 服务器入口
├── lib.rs        # 模块声明
├── api.rs        # 路由处理器
├── block.rs      # 区块链
├── transaction.rs
├── mempool.rs    # 交易池 + Miner
└── merkle.rs
```

### AppState

所有处理器共享同一个链和矿工实例：

```rust
#[derive(Clone)]
pub struct AppState {
    pub blockchain: Arc<Mutex<Blockchain>>,
    pub test_miner: Miner,  // Miner 内部也是 Arc<Mutex<>>，Clone 无成本
}

impl AppState {
    pub fn new() -> Self {
        let chain = Arc::new(Mutex::new(
            Blockchain::load("blockchain.json")
                .unwrap_or(Blockchain::new(4)),
        ));
        Self {
            blockchain: chain.clone(),
            test_miner: Miner::start_new(chain),
        }
    }
}
```

### 路由定义

```rust
let app = Router::new()
    .route("/chain", get(get_chain))
    .route("/balance/{address}", get(get_balance))
    .route("/mempool", get(get_mempool))
    .route("/tx", post(submit_tx))
    .route("/save", post(save_chain))
    .route("/load", post(load_chain))
    .with_state(AppState::new());
```

### 处理器示例

**查询余额：**

```rust
pub async fn get_balance(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Json<Value> {
    let chain = state.blockchain.lock().unwrap();
    let balances = chain.compute_balances().unwrap();
    Json(json!({
        "address": address,
        "balance": balances.get(&address).copied().unwrap_or(0),
    }))
}
```

**提交交易：**

```rust
pub async fn submit_tx(
    State(state): State<AppState>,
    Json(tx): Json<Transaction>,
) -> Json<Value> {
    let mut pool = state.test_miner.pool.lock().unwrap();
    match pool.submit(tx) {
        Ok(_) => Json(json!({"status": "ok"})),
        Err(e) => Json(json!({"status": "error", "message": e})),
    }
}
```

### 启动服务器

```bash
cargo run
# 🚀 RustChain server running on http://127.0.0.1:3000
```

测试：

```bash
# 查看链
curl http://127.0.0.1:3000/chain

# 查询余额
curl http://127.0.0.1:3000/balance/COINBASE

# 提交交易（需要先生成签名交易）
curl -X POST http://127.0.0.1:3000/tx \
  -H "Content-Type: application/json" \
  -d '{"sender":"...","receiver":"...","amount":10,"fee":1,"nonce":0,"signature":"..."}'

# 交易池
curl http://127.0.0.1:3000/mempool

# 持久化
curl -X POST http://127.0.0.1:3000/save
```

### 测试 API

API 处理器的测试直接调用函数（不需要启动服务器）：

```rust
#[tokio::test]
async fn test_submit_tx_and_query_mempool() {
    let state = make_test_state();
    // ...
    let resp = submit_tx(State(state.clone()), Json(tx)).await;
    assert_eq!(resp.0["status"], "ok");

    let mempool_resp = get_mempool(State(state)).await;
    assert_eq!(mempool_resp.0.as_array().unwrap().len(), 1);
}
```

### 当前测试覆盖

| 模块             | 测试数 | 覆盖内容                                                  |
| ---------------- | ------ | --------------------------------------------------------- |
| `transaction.rs` | 4      | 验签、coinbase、篡改拒绝、签名排除                        |
| `merkle.rs`      | 4      | 空列表、长度、确定性、单双不同                            |
| `mempool.rs`     | 9      | 池操作(5) + Miner 组装区块(3) + start_new(1)              |
| `block.rs`       | 8      | 创世块、余额、链有效、篡改/断裂/PoW 检测、全流程、持久化 |
| `api.rs`         | 5      | 链查询、余额、无效签名拒绝、提交流程                     |
| **总计**         | **30** |                                                           |

### 学到了什么

- **axum 的 `State` 提取器**让共享状态注入到处理器变得简单
- **`Miner` 自带交易池**——API 不再需要单独管理 pool，所有操作通过 Miner
- **后台矿工自动循环**——启动服务器后矿工就自动挖矿，不需要手动触发
- **测试直接调用函数**——比启动 HTTP 服务器测试更快、更可控

---

## 如何用本教程

### 给读者

每个章节对应代码仓库里的一个 git commit。你可以按顺序阅读，每读完一章运行一下 `cargo test` 确认当前代码是完整的、可工作的。

### 给自学者的建议

1. **不要复制代码** —— 每个问题先自己想：这个场景下我该怎么办？
2. **跑测试** —— 改动代码后第一件事 `cargo test`
3. **故意制造错误** —— 篡改一个数字看看测试会不会挂，挂了说明保护生效了
4. **改出来再读** —— 先动手，再回头看原理，理解更深刻
