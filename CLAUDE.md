# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test

```bash
# 构建
cargo build

# 运行全部测试
cargo test

# 运行单个测试（按名称匹配）
cargo test test_genesis
cargo test test_mempool_full_flow

# 指定模块的测试
cargo test --lib block::tests
cargo test --lib mempool::tests

# 显示测试输出（println 可见）
cargo test -- --nocapture

# 检查编译错误（不运行测试）
cargo check
```

## 项目架构

### 模块结构

```
lib.rs              — 入口，注册模块，定义全局常量
│
├── block.rs        — 区块 + 区块链核心逻辑
│   ├── Block          — 区块结构（index, hash, merkle_root, mined_difficulty...）
│   ├── Blockchain     — 链结构（chain, difficulty, balance）
│   ├── mine_block()   — PoW 挖矿，nonce 递增直到哈希满足 difficulty 个前导 0
│   ├── add_block()    — 插入 coinbase → 过滤交易 → 挖矿 → 上链 → 难度调整
│   ├── is_valid()     — 全量校验：hash、prev_hash、PoW、coinbase 结构、签名
│   ├── compute_balances() — 从 current balance + 链上交易推导余额
│   ├── filter_valid_txs() — 逐笔模拟余额变化，过滤非法交易
│   ├── adjust_difficulty() — 按出块时间自动升降难度
│   ├── save() / load() — JSON 持久化
│   └── 12 个测试
│
├── transaction.rs  — 交易结构 + 签名
│   ├── Transaction    — sender, receiver, amount, fee, signature
│   ├── new()          — 创建并签名（签名排除自身字段）
│   ├── verify()       — ed25519 验签，coinbase 跳过
│   ├── generate_wallet() — 随机 ed25519 密钥对
│   └── 4 个测试
│
├── merkle.rs       — Merkle Tree（两两配对逐层哈希）
│   ├── compute_merkle_root() — 纯函数
│   └── 4 个测试
│
└── mempool.rs      — 交易池
    ├── MemeryPool     — HashSet<Transaction>
    ├── submit()       — 验签后入池
    ├── select()       — 按 fee 降序取 N 笔
    ├── remove()       — 删除已上链交易
    └── 5 个测试
```

### 数据流

```
用户生成交易 → mempool.submit(tx)
                  ↓
矿工 pool.select(N) → add_block(selected, miner)
                  ↓
add_block: 插入 coinbase → filter_valid_txs → Block::new (Merkle root)
        → mine_block (PoW) → push to chain → adjust_difficulty
                  ↓
pool.remove(已上链交易)
```

### 全局常量

定义在 `lib.rs`：

- `COINBASE_ADDR: &str = "COINBASE"` — coinbase 交易发送方标识
- `REWARD: u64 = 50` — 基础挖矿奖励

### 关键设计决策

1. **账户模型（非 UTXO）** — 使用 `balance: HashMap<String, u64>` 记录余额，类似 Ethereum。余额 = `self.balance` 快照 + 链上所有交易的净效果。`compute_balances()` 每步调用都从快照重推。

2. **同区块逐笔模拟** — `filter_valid_txs` 不是一次性查已上链余额，而是逐笔处理同一区块内的交易（coinbase 在同块创造的钱可被后续交易花掉）。

3. **每个区块记录自己的难度** — `Block.mined_difficulty` 字段。`is_valid()` 用该块的难度校验 PoW，而非区块链当前难度，因为难度会随时间调整。

4. **手续费归矿工** — `add_block` 汇总区块内所有交易的 fee 加到 coinbase 金额中，发送方在 `filter_valid_txs` 中扣除 `amount + fee`。

5. **测试模块化** — 每个测试只测一个场景，函数名就是场景描述。每个模块有自己的 `#[cfg(test)] mod tests`。

### 依赖

| 包 | 用途 |
|----|------|
| ed25519-dalek 2.x | 数字签名（注意 API 与 1.x 差异：`VerifyingKey` 替代 `PublicKey`，`SigningKey` 替代 `Keypair`） |
| sha2 | SHA-256 哈希 |
| serde / serde_json | 序列化（save/load、Display、Merkle 哈希输入） |
| hex | 字节 ↔ 十六进制字符串 |
| chrono | 时间戳 |
| rand | 生成随机密钥对 |
