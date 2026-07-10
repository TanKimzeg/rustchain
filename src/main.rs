use axum::{
    Router,
    routing::{get, post},
};
use rustchain::api::{AppState, get_chain, get_detail, get_mempool, load_chain, save_chain, submit_tx};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    env_logger::init();
    let app = Router::new()
        .route("/chain", get(get_chain))
        .route("/detail/{address}", get(get_detail))
        .route("/mempool", get(get_mempool))
        .route("/tx", post(submit_tx))
        // .route("/mine", post(mine_block))
        .route("/save", post(save_chain))
        .route("/load", post(load_chain))
        .with_state(AppState::new());
    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    log::info!("🚀 RustChain server running on http://127.0.0.1:3000");
    axum::serve(listener, app)
        .await
        .expect("Server failed to start");
}
