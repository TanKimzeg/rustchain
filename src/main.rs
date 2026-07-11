use axum::{
    Router,
    routing::{get, post},
};
use rustchain::api::{
    AppState, get_chain, get_detail, get_mempool, load_chain, save_chain, submit_tx,
};
use rustchain::block::Blockchain;
use rustchain::mempool::Miner;
use rustchain::p2p::build_p2p;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    env_logger::init();
    // 共享状态
    let blockchain = Arc::new(Mutex::new(
        Blockchain::load("blockchain.json").unwrap_or(Blockchain::new(4)),
    ));
    let miner = Arc::new(Miner::start_new(blockchain.clone()));

    // 启动 P2P（后台），拿到广播通道
    let p2p_tx = build_p2p(&miner.key_pair, (*miner).clone(), 3001);

    // HTTP API
    let app = Router::new()
        .route("/chain", get(get_chain))
        .route("/detail/{address}", get(get_detail))
        .route("/mempool", get(get_mempool))
        .route("/tx", post(submit_tx))
        .route("/save", post(save_chain))
        .route("/load", post(load_chain))
        .with_state(AppState {
            blockchain: blockchain.clone(),
            test_miner: (*miner).clone(),
            p2p_tx,
        });
    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    log::info!("🚀 HTTP API running on http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}
