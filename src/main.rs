use axum::{
    Router,
    routing::{get, post},
};
use futures::StreamExt;
use rustchain::api::{
    AppState, get_chain, get_detail, get_mempool, load_chain, save_chain, submit_tx,
};
use rustchain::block::Blockchain;
use rustchain::mempool::Miner;
use rustchain::p2p::{build_p2p, handle_p2p_event};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    // 共享状态
    let blockchain = Arc::new(Mutex::new(
        Blockchain::load("blockchain.json").unwrap_or(Blockchain::new(4)),
    ));
    let miner = Arc::new(Miner::start_new(blockchain.clone()));

    // 启动 P2P（后台）
    let mut swarm = build_p2p(&miner.key_pair);
    let listen_addr: libp2p::Multiaddr = "/ip4/0.0.0.0/tcp/3001".parse().unwrap();
    swarm.listen_on(listen_addr).unwrap();
    println!("P2P 节点正在监听 :3001");

    let p2p_miner = miner.clone();
    tokio::spawn(async move {
        loop {
            let event = swarm.select_next_some().await;
            handle_p2p_event(event, &p2p_miner).await;
        }
    });

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
        });
    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("🚀 HTTP API running on http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}
