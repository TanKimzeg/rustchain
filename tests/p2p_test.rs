use ed25519_dalek::SigningKey;
use futures::StreamExt;
use libp2p::swarm::SwarmEvent;
use rand::rngs::OsRng;
use rustchain::p2p::{BlockchainBehaviourEvent, P2PMessage, build_swarm, publish_message};
use std::time::Duration;

/// 生成测试用的 SigningKey
fn test_key() -> SigningKey {
    SigningKey::generate(&mut OsRng)
}

/// 等待连接和订阅同步，然后发布消息并验证接收
#[tokio::test]
async fn test_gossip_message_exchange() {
    let (mut s1, t1) = build_swarm(&test_key());
    let (mut s2, _t2) = build_swarm(&test_key());

    // 监听随机端口
    s1.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).unwrap();
    let addr1 = loop {
        match s1.next().await.unwrap() {
            SwarmEvent::NewListenAddr { address, .. } => break address,
            _ => continue,
        }
    };
    s2.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).unwrap();
    // 消耗 s2 的 listen 事件
    tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            if matches!(s2.next().await.unwrap(), SwarmEvent::NewListenAddr { .. }) {
                break;
            }
        }
    })
    .await
    .ok();

    // 连接 s2 → s1
    s2.dial(addr1).unwrap();

    let tx = rustchain::transaction::Transaction::new(&test_key(), "receiver", 10, 1, 0);
    let sent_msg = P2PMessage::NewTransaction(tx);

    // Phase 1：等待双方连接建立
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut c1 = false;
        let mut c2 = false;
        loop {
            tokio::select! {
                event = s1.select_next_some() => {
                    if matches!(&event, SwarmEvent::ConnectionEstablished { .. }) { c1 = true; }
                }
                event = s2.select_next_some() => {
                    if matches!(&event, SwarmEvent::ConnectionEstablished { .. }) { c2 = true; }
                }
            }
            if c1 && c2 { break; }
        }
    })
    .await
    .expect("连接建立超时");
    log::info!("连接建立成功");

    // Phase 1.5：等待 gossipsub 订阅同步
    tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            tokio::select! {
                _event = s1.select_next_some() => {}
                _event = s2.select_next_some() => {}
            }
        }
    })
    .await
    .ok(); // 超时就够了，订阅已经过了一遍事件循环

    // Phase 2：发布消息
    publish_message(&mut s1, &t1, sent_msg.clone());

    // Phase 3：等待 s2 收到消息
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let event = s2.select_next_some().await;
            if let SwarmEvent::Behaviour(BlockchainBehaviourEvent::Gossip(
                libp2p::gossipsub::Event::Message { message, .. },
            )) = event {
                if let Ok(received) = serde_json::from_slice::<P2PMessage>(&message.data) {
                    log::info!("s2 收到消息: {:?}", received);
                    return received;
                }
            }
        }
    })
    .await;

    match result {
        Ok(received) => assert_eq!(received, sent_msg),
        Err(_) => panic!("超时：s2 未在 5 秒内收到消息"),
    }
}
