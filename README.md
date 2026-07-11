# RustChain

从零构建区块链的 Rust 实践项目。每个核心概念对应一个 git commit，循序渐进。

> 详细教程见 [TUTORIAL.md](./TUTORIAL.md)

## 架构

```
    主程序 main.rs  (axum :3000 + libp2p :3001)
       │
       ├── api.rs          HTTP 处理器
       ├── block.rs        区块 + 区块链核心
       ├── transaction.rs  交易 + ed25519 签名
       ├── merkle.rs       Merkle 树
       ├── mempool.rs      交易池 + 独立矿工
       └── p2p.rs          libp2p 网络 (gossipsub + mDNS)
```

## 使用

```bash
# 构建
cargo build

# 运行（HTTP API :3000 + P2P :3001）
cargo run

# 测试
cargo test

# API 示例
curl http://127.0.0.1:3000/chain
curl http://127.0.0.1:3000/detail/COINBASE
curl http://127.0.0.1:3000/mempool

# 启动第二个节点
cargo run -- --port 3002  # 需要实现 CLI 参数
```

## 概念覆盖

| 概念 | 实现 |
|------|------|
| PoW 挖矿 | mine_block()，前导零难度 |
| 数字签名 | ed25519-dalek 2.x |
| 账户模型 | compute_balances() 从链重推 |
| 防重放 | nonce 计数器 + get_tx_count() |
| 手续费 | 矿工收集区块内所有 fee |
| 难度调整 | 按出块时间自动调节 |
| 状态缓存 | AddressDetail 增量更新 |
| Merkle 树 | 两两哈希，O(log n) 验证 |
| 交易池 | MemeryPool，按 fee 排序 |
| HTTP API | axum 0.8，6 个端点 |
| P2P 网络 | libp2p gossipsub + mDNS |
| 区块广播 | 矿工挖矿后自动广播到网络 |
| 链同步 | 新节点自动请求全链同步 |
| 持久化 | serde_json 序列化 |

## 技术栈

axum 0.8 · tokio · libp2p 0.56 · ed25519-dalek 2.x · serde · sha2

## 测试

```bash
cargo test                    # 全部测试（29 单元 + 2 集成）
cargo test --lib              # 单元测试
cargo test --test api_test    # HTTP API 集成测试
cargo test --test p2p_test    # P2P 网络集成测试
```
