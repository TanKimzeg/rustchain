use std::sync::{Arc, Mutex};

use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::{Value, json};

use crate::{block::Blockchain, mempool::Miner, transaction::Transaction};

#[derive(Clone)]
pub struct AppState {
    pub blockchain: Arc<Mutex<Blockchain>>,
    pub test_miner: Miner,
}

impl AppState {
    pub fn new() -> Self {
        let chain = Arc::new(Mutex::new(
            Blockchain::load("blockchain.json").unwrap_or(Blockchain::new(4)),
        ));
        Self {
            blockchain: chain.clone(),
            test_miner: Miner::start_new(chain.clone()),
        }
    }
}

/// GET /chain — 返回整条链
pub async fn get_chain(State(state): State<AppState>) -> Json<Value> {
    let chain = state.blockchain.lock().unwrap();
    Json(serde_json::to_value(&chain.chain).unwrap())
}

/// GET /detail/{address} — 查询地址余额
pub async fn get_detail(State(state): State<AppState>, Path(address): Path<String>) -> Json<Value> {
    let chain = state.blockchain.lock().unwrap();
    let details = &chain.address_details;
    Json(serde_json::to_value(details.get(&address)).unwrap())
}

/// GET /mempool — 查看待处理交易
pub async fn get_mempool(State(state): State<AppState>) -> Json<Value> {
    let pool = state.test_miner.pool.lock().unwrap();
    let txs: Vec<&Transaction> = pool.candidate.iter().collect();
    Json(serde_json::to_value(&txs).unwrap())
}

/// POST /tx — 提交交易到交易池
pub async fn submit_tx(State(state): State<AppState>, Json(tx): Json<Transaction>) -> Json<Value> {
    let mut pool = state.test_miner.pool.lock().unwrap();
    match pool.submit(tx) {
        Ok(_) => Json(json!({"status": "ok"})),
        Err(e) => Json(json!({"status": "error", "message": e})),
    }
}

/// POST /save — 持久化区块链
pub async fn save_chain(State(state): State<AppState>) -> Json<Value> {
    let chain = state.blockchain.lock().unwrap();
    match chain.dump("blockchain.json") {
        Ok(_) => Json(json!({"status": "saved"})),
        Err(e) => Json(json!({"status": "error", "message": e.to_string()})),
    }
}

/// POST /load — 从文件加载区块链
pub async fn load_chain(state: State<AppState>) -> Json<Value> {
    let mut chain = state.blockchain.lock().unwrap();
    match Blockchain::load("blockchain.json") {
        Ok(loaded) => {
            *chain = loaded;
            Json(json!({"status": "loaded"}))
        }
        Err(e) => Json(json!({"status": "error", "message": e.to_string()})),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{COINBASE_ADDR, REWARD, block::Block, transaction::generate_wallet};
    use axum::extract::State;
    use ed25519_dalek::SigningKey;

    fn make_wallet() -> (SigningKey, String) {
        let w = generate_wallet();
        let addr = hex::encode(w.verifying_key().to_bytes());
        (w, addr)
    }

    fn make_test_state() -> AppState {
        let chain = Arc::new(Mutex::new(Blockchain::new(2)));
        let miner = Miner::start_new(chain.clone());
        AppState {
            blockchain: chain,
            test_miner: miner,
        }
    }

    #[tokio::test]
    async fn test_get_chain_returns_genesis() {
        let state = make_test_state();
        let resp = get_chain(State(state)).await;
        let chain = resp.0.as_array().unwrap();
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0]["index"], 0);
    }

    #[tokio::test]
    async fn test_get_balance_returns_zero_for_unknown() {
        let state = make_test_state();
        let resp = get_detail(State(state), Path("nonexistent".into())).await;
        assert!(resp.0.is_null());
    }

    #[tokio::test]
    async fn test_get_balance_after_mining() {
        let state = make_test_state();
        let (_alice, alice_addr) = make_wallet();
        // 挖一个空块给 Alice 发钱
        {
            let mut chain = state.blockchain.lock().unwrap();
            let prev = chain.latest_block();
            let coinbase = Transaction::new_coinbase(&alice_addr, 0);
            let mut block = Block::new(prev.index + 1, vec![coinbase], prev.hash.clone());
            block.mine_block(chain.difficulty);
            chain.add_block(block).unwrap();
        }
        let resp = get_detail(State(state), Path(alice_addr)).await;
        assert_eq!(resp.0["balance"], REWARD);
    }

    #[tokio::test]
    async fn test_submit_tx_rejects_invalid_signature() {
        let state = make_test_state();
        let tx = Transaction {
            sender: "a".repeat(64),
            receiver: "b".repeat(64),
            amount: 100,
            signature: "bad".to_string(),
            fee: 1,
            nonce: 0,
        };
        let resp = submit_tx(State(state), Json(tx)).await;
        assert_eq!(resp.0["status"], "error");
    }

    #[tokio::test]
    async fn test_submit_tx_and_query_mempool() {
        let state = make_test_state();
        let (alice, alice_addr) = make_wallet();
        // 给 Alice 发钱
        {
            let mut chain = state.blockchain.lock().unwrap();
            let prev = chain.latest_block();
            let coinbase = Transaction {
                sender: COINBASE_ADDR.to_string(),
                receiver: alice_addr.clone(),
                amount: REWARD,
                signature: String::new(),
                fee: 0,
                nonce: 0,
            };
            let mut block = Block::new(prev.index + 1, vec![coinbase], prev.hash.clone());
            block.mine_block(chain.difficulty);
            chain.add_block(block).unwrap();
        }
        let receiver = hex::encode(generate_wallet().verifying_key().to_bytes());
        let tx = Transaction::new(&alice, &receiver, 10, 1, 3);
        let resp = submit_tx(State(state.clone()), Json(tx)).await;
        assert_eq!(resp.0["status"], "ok");
        // 验证交易池里有它
        let mempool_resp = get_mempool(State(state)).await;
        let txs = mempool_resp.0.as_array().unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0]["amount"], 10);
    }
}
