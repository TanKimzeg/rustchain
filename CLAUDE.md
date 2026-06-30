# rustchain — 学习区块链的 Rust 项目

## 项目状态

### 已实现
- **区块**：`Block` — index, timestamp, transactions, hash, prev_hash, nonce
- **区块链**：`Blockchain` — chain 存储、创世块、添加区块、is_valid 校验（含 PoW 前导 0 检查）
- **工作量证明**：挖矿，nonce 递增直到哈希满足 difficulty 个前导 0
- **交易**：`Transaction` — ed25519 签名/验签，签名排除自身字段
- **账户模型雏形**：`balance: HashMap<String, u32>`，但未与交易联动

### 依赖
ed25519-dalek, serde, serde_json, sha2, hex, chrono, rand
