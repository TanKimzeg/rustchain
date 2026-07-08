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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::{generate_wallet, Transaction};

    fn make_tx(amount: u64) -> Transaction {
        let sender = generate_wallet();
        let receiver = hex::encode(generate_wallet().verifying_key().to_bytes());
        Transaction::new(&sender, &receiver, amount)
    }

    #[test]
    fn test_empty_list() {
        assert_eq!(compute_merkle_root(&[]), "");
    }

    #[test]
    fn test_root_length() {
        const HASH_LEN: usize = 64;
        assert_eq!(compute_merkle_root(&[make_tx(10)]).len(), HASH_LEN);
        assert_eq!(compute_merkle_root(&[make_tx(10), make_tx(20)]).len(), HASH_LEN);
        assert_eq!(
            compute_merkle_root(&[make_tx(10), make_tx(20), make_tx(30)]).len(),
            HASH_LEN
        );
    }

    #[test]
    fn test_deterministic() {
        let txs = vec![make_tx(10), make_tx(20)];
        let root1 = compute_merkle_root(&txs);
        let root2 = compute_merkle_root(&txs);
        assert_eq!(root1, root2);
    }

    #[test]
    fn test_single_and_double_differ() {
        let tx = make_tx(10);
        let root1 = compute_merkle_root(&[tx.clone()]);
        let root2 = compute_merkle_root(&[tx, make_tx(20)]);
        assert_ne!(root1, root2);
    }
}
