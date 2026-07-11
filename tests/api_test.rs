use axum::{Router, routing::{get, post}, serve};
use rustchain::{
    api::{AppState, get_chain, get_detail, get_mempool, submit_tx},
    block::{Block, Blockchain},
    mempool::Miner,
    transaction::Transaction,
    REWARD,
};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use std::time::Duration;

/// 辅助函数：给矿工地址发钱的区块
fn fund_block(chain: &Blockchain, miner_addr: &str) -> Block {
    let prev = chain.latest_block();
    let coinbase = Transaction::new_coinbase(miner_addr, 0);
    let mut block = Block::new(prev.index + 1, vec![coinbase], prev.hash.clone());
    block.mine_block(chain.difficulty);
    block
}

#[tokio::test]
async fn test_http_full_flow() {
    // 1. 创建链 + 矿工
    let chain = Arc::new(Mutex::new(Blockchain::new(2)));
    let mut miner = Miner::start_new(chain.clone());

    // 2. 给矿工发初始资金 (挖一个空块)
    {
        let mut c = chain.lock().unwrap();
        let block = fund_block(&c, &miner.address());
        c.add_block(block).unwrap();
    }

    // 3. 哑 P2P 通道
    let (p2p_tx, _rx) = mpsc::unbounded_channel();
    miner.broadcaster = Some(p2p_tx.clone());

    // 4. 启动 HTTP 服务器
    let app = Router::new()
        .route("/chain", get(get_chain))
        .route("/detail/{address}", get(get_detail))
        .route("/mempool", get(get_mempool))
        .route("/tx", post(submit_tx))
        .with_state(AppState {
            blockchain: chain.clone(),
            test_miner: miner.clone(),
            p2p_tx,
        });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(200)).await;

    let base = format!("http://{}", addr);
    let client = reqwest::Client::new();

    // 5. GET /chain — 验证链上有 2 个区块（genesis + fund）
    let resp = client.get(format!("{}/chain", base)).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let chain_val: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(chain_val.as_array().unwrap().len(), 2);
    assert_eq!(chain_val[0]["index"], 0);
    assert_eq!(chain_val[1]["index"], 1);

    // 6. GET /detail/{miner} — 验证初始余额
    let resp = client
        .get(format!("{}/detail/{}", base, miner.address()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let detail: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(detail["balance"], REWARD);
    assert_eq!(detail["nonce"], 0);

    // 7. 创建签名交易: miner → 随机地址
    let receiver = rustchain::transaction::generate_wallet();
    let receiver_addr = hex::encode(receiver.verifying_key().to_bytes());
    let tx = Transaction::new(&miner.key_pair, &receiver_addr, 10, 1, 0);
    let tx_json = serde_json::to_value(&tx).unwrap();

    // 8. POST /tx — 提交交易
    let resp = client
        .post(format!("{}/tx", base))
        .json(&tx_json)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");

    // 9. GET /detail/{miner} — 验证提交后余额不变（还在 mempool，未上链）
    let resp = client
        .get(format!("{}/detail/{}", base, miner.address()))
        .send()
        .await
        .unwrap();
    let detail: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(detail["balance"], REWARD);

    // 10. GET /mempool — 验证交易在池中
    let resp = client.get(format!("{}/mempool", base)).send().await.unwrap();
    let mempool: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(mempool.as_array().unwrap().len(), 1);
    assert_eq!(mempool[0]["amount"], 10);
    assert_eq!(mempool[0]["fee"], 1);
    assert_eq!(mempool[0]["nonce"], 0);
    assert_eq!(mempool[0]["sender"], miner.address());
    assert_eq!(mempool[0]["receiver"], receiver_addr);

    // 11. POST /tx — 提交无效签名，验证被拒绝
    let bad_tx = Transaction {
        sender: "fake".to_string(),
        receiver: "nowhere".to_string(),
        amount: 999,
        signature: "bad".to_string(),
        fee: 0,
        nonce: 0,
    };
    let resp = client
        .post(format!("{}/tx", base))
        .json(&serde_json::to_value(&bad_tx).unwrap())
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "error");

    // 12. GET /detail/unknown — 未知地址返回 null
    let resp = client
        .get(format!("{}/detail/{}", base, "nonexistent"))
        .send()
        .await
        .unwrap();
    let detail: serde_json::Value = resp.json().await.unwrap();
    assert!(detail.is_null());
}

#[tokio::test]
async fn test_http_get_chain_empty() {
    let chain = Arc::new(Mutex::new(Blockchain::new(2)));
    let miner = Miner::start_new(chain.clone());
    let (p2p_tx, _rx) = mpsc::unbounded_channel();

    let app = Router::new()
        .route("/chain", get(get_chain))
        .with_state(AppState {
            blockchain: chain,
            test_miner: miner,
            p2p_tx,
        });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let resp = reqwest::get(format!("http://{}/chain", addr)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let chain_val: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(chain_val.as_array().unwrap().len(), 1);
    assert_eq!(chain_val[0]["index"], 0);
}
