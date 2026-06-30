use ed25519_dalek::{PUBLIC_KEY_LENGTH, Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

use crate::COINBASE_ADDR;

/// 交易结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub sender: String,     // 发送方公钥（hex 编码）
    pub receiver: String,   // 接收方公钥（hex 编码）
    pub amount: u64,        // 转账金额
    pub signature: String,  // 发送方的签名
}

impl Transaction {
    /// 创建并签名交易
    pub fn new(sender_keypair: &SigningKey, receiver: impl Into<String>, amount: u64) -> Self {
        let sender_hex = hex::encode(sender_keypair.verifying_key().to_bytes());

        let mut tx = Transaction {
            sender: sender_hex,
            receiver: receiver.into(),
            amount,
            signature: String::new(),
        };
        // 只对交易内容签名（排除 signature 字段自身）
        let tx_data = tx.serialize_for_signing();
        tx.signature = hex::encode(sender_keypair.sign(tx_data.as_bytes()).to_bytes().to_vec());
        tx
    }

    /// 返回待签名的数据（sender + receiver + amount，不含 signature）
    pub fn serialize_for_signing(&self) -> String {
        serde_json::json!({
            "sender":   self.sender,
            "receiver": self.receiver,
            "amount":   self.amount,
        })
        .to_string()
    }

    /// 验证交易签名是否合法
    pub fn verify(&self) -> bool {
        if &self.sender == COINBASE_ADDR {
            return true;
        }
        // 反序列化签名
        let sig = match hex::decode(self.signature.clone()) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let signature = match Signature::from_slice(sig.as_slice()) {
            Ok(s) => s,
            Err(_) => return false,
        };
        // hex 解码发送方公钥
        let pub_key_bytes = match hex::decode(&self.sender) {
            Ok(b) => b,
            Err(_) => return false,
        };
        // 转成 [u8; 32] 定长数组（VerifyingKey::from_bytes 要求的）
        let pub_key_arr: [u8; PUBLIC_KEY_LENGTH] = match pub_key_bytes.try_into() {
            Ok(a) => a,
            Err(_) => return false,
        };
        let pub_key = match VerifyingKey::from_bytes(&pub_key_arr) {
            Ok(p) => p,
            Err(_) => return false,
        };
        // 用同样的数据验证签名
        let tx_data = self.serialize_for_signing();
        pub_key.verify(tx_data.as_bytes(), &signature).is_ok()
    }

}

impl std::fmt::Display for Transaction  {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tx_json = serde_json::to_string(self).expect("Failed to Deserialized");
        write!(f, "{}", tx_json)
    }
}

/// 工具函数：生成随机密钥对（模拟用户钱包）
pub fn generate_wallet() -> SigningKey {
    let mut csprng = OsRng {};
    SigningKey::generate(&mut csprng)
}
