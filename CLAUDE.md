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

# 启动 HTTP 服务器
cargo run
```

## 项目架构

### 模块结构

```
main.rs             — HTTP 服务器入口（axum + tokio）
lib.rs              — 模块注册，全局常量
│
├── block.rs        — 区块 + 区块链核心逻辑
│   ├── Block          — 区块结构（index, hash, merkle_root, mined_difficulty...）
│   ├── Blockchain     — 链结构（chain, difficulty）无 balance 快照
│   ├── mine_block()   — PoW 挖矿
│   ├── add_block()    — 校验 → 上链 → 难度调整（纯链操作）
│   ├── check_block()  — 单块校验：hash、prev_hash、PoW、coinbase、签名
│   ├── is_valid()     — Result<()> 全链校验：check_block + balance + nonce
│   ├── compute_balances() — 从空 HashMap 重推余额（无快照）
│   ├── get_tx_count()      — 从链重推 nonce 状态
│   └── 8 个测试
│
├── transaction.rs  — 交易结构 + 签名
│   ├── Transaction    — sender, receiver, amount, fee, signature, nonce
│   ├── new()          — 创建并签名（含 nonce）
│   ├── new_coinbase() — 统一创建 coinbase 交易
│   ├── verify()       — ed25519 验签，coinbase 跳过
│   ├── generate_wallet() — 随机 ed25519 密钥对
│   └── 4 个测试
│
├── merkle.rs       — Merkle Tree（两两配对逐层哈希）
│   ├── compute_merkle_root() — 纯函数
│   └── 4 个测试
│
├── mempool.rs      — 交易池 + 矿工
│   ├── MemeryPool     — HashSet<Transaction>
│   ├── submit()       — 验签后入池
│   ├── select()       — 按 fee 降序取 N 笔
│   ├── remove()       — 删除已上链交易
│   ├── Miner          — 独立矿工（address, pool, chain, tx_count）
│   │   ├── assemble_block() — 选tx → coinbase → 过滤 → 挖矿 → 返回 Block
│   │   ├── filter_valid_txs() — 余额 + nonce 逐笔模拟
│   │   ├── start_mining_loop() — 后台 5 秒循环挖矿
│   │   └── start_new() — 一键创建矿工并启动循环
│   └── 9 个测试（5 MemeryPool + 4 Miner）
│
└── api.rs          — HTTP 处理器
    ├── AppState       — Arc<Mutex<Blockchain>> + Miner
    ├── get_chain()    — GET /chain
    ├── get_balance()  — GET /balance/{address}
    ├── get_mempool()  — GET /mempool
    ├── submit_tx()    — POST /tx
    ├── save_chain()   — POST /save
    ├── load_chain()   — POST /load
    └── 5 个测试
```

### 数据流

```
用户生成签名交易 → POST /tx → MemeryPool.submit()
                                   │
                Miner 后台循环（5s） │
                                   ▼
              Miner.assemble_block():
                 pool.select(10) → 创建 coinbase
                 → filter_valid_txs(余额 + nonce)
                 → Block::new (Merkle root) → mine_block (PoW)
                 → pool.remove(已上链及无效交易)
                                   │
                                   ▼
              Blockchain.add_block(block):
                 check_block → push → adjust_difficulty
```

### 全局常量

定义在 `lib.rs`：

- `COINBASE_ADDR: &str = "COINBASE"` — coinbase 交易发送方标识
- `REWARD: u64 = 50` — 基础挖矿奖励
- `INIT_TARGET_TIME: u64 = 2` — 目标出块时间（秒）
- `INIT_ADJ_INTERVAL: u32 = 4` — 每几个块调整一次难度

### 关键设计决策

1. **账户模型（非 UTXO）** — 使用 `HashMap<String, u64>` 记录余额，类似 Ethereum。余额从空 HashMap 开始，逐笔重推，无快照缓存。

2. **状态全推导** — `compute_balances()` 和 `get_tx_count()` 都从链上交易重新计算，没有缓存状态。链是唯一事实来源。

3. **Nonce 防重放** — 每个交易带 nonce 计数器，纳入签名。`get_tx_count()` 推导链上 nonce 状态，校验连续性。

4. **Miner 与 Blockchain 解耦** — Blockchain 只做存储和验证，Miner 负责交易组装、过滤、挖矿。两者通过 Block 交互。

5. **同区块逐笔模拟** — `filter_valid_txs` 在同一区块内逐笔模拟余额变化，coinbase 在同块创造的钱可被后续交易花掉。

6. **每个区块记录自己的难度** — `Block.mined_difficulty` 字段。`check_block()` 用该块的难度校验 PoW，而非当前全局难度。

7. **手续费归矿工** — Miner 汇总区块内所有交易的 fee 加到 coinbase 金额中。

8. **测试模块化** — 每个测试只测一个场景，函数名就是场景描述。每个模块有自己的 `#[cfg(test)] mod tests`。

### 依赖

| 包               | 用途                                           |
|------------------|------------------------------------------------|
| axum 0.8         | HTTP 框架（Router, State, extractors）          |
| tokio            | 异步运行时                                      |
| ed25519-dalek 2.x| 数字签名（VerifyingKey / SigningKey）           |
| sha2             | SHA-256 哈希                                   |
| serde / serde_json| 序列化                                        |
| hex              | 字节 ↔ 十六进制字符串                          |
| chrono           | 时间戳                                         |
| rand             | 生成随机密钥对                                  |
