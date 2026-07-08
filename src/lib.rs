pub mod block;
pub mod transaction;
pub mod mempool;
pub mod merkle;

pub const COINBASE_ADDR: &str = "COINBASE";
pub const REWARD: u64 = 50;
pub const INIT_TARGET_TIME: u64 = 2;
pub const INIT_ADJ_INTERVAL: u32 = 4;
