use axum::{
    Router,
    routing::{get, post},
    serve,
};
use rustchain::{
    REWARD,
    api::{AppState, get_chain, get_detail, get_mempool, submit_tx},
    block::{Block, Blockchain},
    mempool::Miner,
    transaction::Transaction,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

/// 辅助函数：给地址发钱的空块
fn fund_block(chain: &Blockchain, addr: &str) -> Block {
    let prev = chain.latest_block();
    let coinbase = Transaction::new_coinbase(addr, 0);
    let mut block = Block::new(prev.index + 1, vec![coinbase], prev.hash.clone());
    block.mine_block(chain.difficulty);
    block
}

#[tokio::test]
async fn test_http_full_cycle() {
    // 1. 创建链 + 矿工
    let chain = Arc::new(Mutex::new(Blockchain::new(2)));
    let mut miner = Miner::start_new(chain.clone());

    // 2. 给矿工初始资金
    let block = fund_block(&chain.lock().unwrap(), &miner.address());
    chain.lock().unwrap().add_block(block).unwrap();
    assert_eq!(
        chain.lock().unwrap().address_details[&miner.address()].balance,
        REWARD
    );

    // 3. 设置广播通道并启动挖矿
    let (p2p_tx, _rx) = mpsc::unbounded_channel();
    miner.broadcaster = Some(p2p_tx.clone());
    miner.start_mining_loop();

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

    // 5. 初始状态：nonce 0，余额 >= REWARD（矿工可能在后台挖了几个块）
    let chain_resp: serde_json::Value = client
        .get(format!("{}/chain", base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(chain_resp.as_array().unwrap().len() >= 2);

    let detail: serde_json::Value = client
        .get(format!("{}/detail/{}", base, miner.address()))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(detail["balance"].as_u64().unwrap() >= REWARD);
    assert_eq!(detail["nonce"], 0);

    // 6. 创建并提交签名交易
    let receiver = rustchain::transaction::generate_wallet();
    let receiver_addr = hex::encode(receiver.verifying_key().to_bytes());
    let sender_addr = miner.address();
    let tx = Transaction::new(&miner.key_pair, &receiver_addr, 10, 1, 0);
    let tx_json = serde_json::to_value(&tx).unwrap();

    let resp = client
        .post(format!("{}/tx", base))
        .json(&tx_json)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.json::<serde_json::Value>().await.unwrap()["status"],
        "ok"
    );

    // 7. 等待矿工打包交易（最长等 15 秒，每 500ms 轮询）
    let mut mined_height = 0;
    let mut found = false;
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(500)).await;

        let chain_val: serde_json::Value = client
            .get(format!("{}/chain", base))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let height = chain_val.as_array().unwrap().len() - 1;

        if height > mined_height {
            mined_height = height;
            // 检查最新区块里有没有我们的交易
            let block = &chain_val.as_array().unwrap()[height as usize];
            let txs = block["transactions"].as_array().unwrap();
            for tx_val in txs {
                if tx_val["sender"] == sender_addr && tx_val["amount"] == 10 {
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
    }
    assert!(found, "交易未在超时时间内被打包");

    // 8. 验证链上状态：nonce 增加（至少发过一笔），矿工余额未归零
    let detail: serde_json::Value = client
        .get(format!("{}/detail/{}", base, sender_addr))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["nonce"], 1, "发送方 nonce 应为 1");
    assert!(
        detail["balance"].as_u64().unwrap() > 0,
        "发送方余额不应归零"
    );

    // 9. 验证接收方余额
    let detail: serde_json::Value = client
        .get(format!("{}/detail/{}", base, receiver_addr))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["balance"].as_u64().unwrap(), 10);

    // 10. 交易池应该空了
    let mempool: serde_json::Value = client
        .get(format!("{}/mempool", base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(mempool.as_array().unwrap().is_empty());
}
