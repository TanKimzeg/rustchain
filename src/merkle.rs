use sha2::{Digest, Sha256};
use hex;
use crate::transaction::Transaction;

pub fn compute_merkle_root(txs: &[Transaction]) -> String {
    // 1. 空列表返回空字符串
    if txs.is_empty() {
        return String::new();
    }

    // 2. 把每笔交易哈希 → Vec<String>
    let mut layer: Vec<String> = txs.iter()
        .map(|tx| {
            let mut hasher = Sha256::new();
            hasher.update(tx.to_string());
            hex::encode(hasher.finalize())
        })
        .collect();

    // 3. 逐层向上，直到只剩一个根
    while layer.len() > 1 {
        // 如果是奇数，复制最后一笔
        if layer.len() % 2 == 1 {
            layer.push(layer.last().unwrap().clone());
        }

        // 两两配对哈希
        layer = layer.chunks(2).map(|pair| {
            let mut hasher = Sha256::new();
            hasher.update(format!("{}{}", pair[0], pair[1]));
            hex::encode(hasher.finalize())
        }).collect();
    }

    layer.into_iter().next().unwrap_or_default()
}
