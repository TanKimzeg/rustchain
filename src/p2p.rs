use crate::block::Block;
use crate::mempool::Miner;
use crate::transaction::Transaction;
use ed25519_dalek::SigningKey;
use futures::StreamExt;
use libp2p::SwarmBuilder;
use libp2p::gossipsub::{self, IdentTopic, MessageId};
use libp2p::identity;
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

pub const BLOCKCHAIN_TOPIC: &str = "rustchain";

/// P2P 网络里交换的消息类型
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum P2PMessage {
    NewBlock(Block),
    NewTransaction(Transaction),
    SyncRequest,
    SyncResponse(Vec<Block>),
}

/// libp2p 的行为组合
#[derive(NetworkBehaviour)]
pub struct BlockchainBehaviour {
    pub gossip: gossipsub::Behaviour,
    pub mdns: libp2p::mdns::tokio::Behaviour,
}

/// 构建 swarm（主要用于测试）
pub fn build_swarm(
    signing_key: &SigningKey,
) -> (libp2p::swarm::Swarm<BlockchainBehaviour>, IdentTopic) {
    let keypair = identity::Keypair::ed25519_from_bytes(signing_key.to_bytes()).unwrap();
    let peer_id = keypair.public().to_peer_id();

    let message_id_fn = |message: &gossipsub::Message| MessageId::from(&message.data[..20]);
    let gossip_config = gossipsub::ConfigBuilder::default()
        .validation_mode(gossipsub::ValidationMode::Permissive)
        .message_id_fn(message_id_fn)
        .build()
        .unwrap();

    let mut gossip = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(keypair.clone()),
        gossip_config,
    )
    .unwrap();

    let topic = IdentTopic::new(BLOCKCHAIN_TOPIC);
    gossip.subscribe(&topic).unwrap();

    let mdns =
        libp2p::mdns::tokio::Behaviour::new(libp2p::mdns::Config::default(), peer_id).unwrap();

    let behaviour = BlockchainBehaviour { gossip, mdns };

    let swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )
        .unwrap()
        .with_behaviour(|_| behaviour)
        .unwrap()
        .with_swarm_config(|c| c)
        .build();

    (swarm, topic)
}

/// 广播消息到 gossip 网络
pub fn publish_message(
    swarm: &mut libp2p::swarm::Swarm<BlockchainBehaviour>,
    topic: &IdentTopic,
    msg: P2PMessage,
) {
    let data = serde_json::to_vec(&msg).unwrap();
    let _ = swarm
        .behaviour_mut()
        .gossip
        .publish(topic.clone(), data);
    log::info!("📡 广播: {:?}", msg);
}

/// 处理 P2P 收到的消息
async fn handle_p2p_event(
    event: libp2p::swarm::SwarmEvent<BlockchainBehaviourEvent>,
    miner: &Miner,
    broadcaster: &mpsc::UnboundedSender<P2PMessage>,
) {
    match event {
        SwarmEvent::Behaviour(BlockchainBehaviourEvent::Gossip(gossipsub::Event::Message {
            message,
            ..
        })) => {
            if let Ok(msg) = serde_json::from_slice::<P2PMessage>(&message.data) {
                match msg {
                    P2PMessage::NewBlock(block) => {
                        log::info!("📩 P2P 收到区块 #{}", block.index);
                        let mut chain = miner.chain.lock().unwrap();
                        chain.add_block(block).ok();
                    }
                    P2PMessage::NewTransaction(tx) => {
                        log::info!("📩 P2P 收到交易");
                        miner.submit_tx(tx).ok();
                    }
                    P2PMessage::SyncRequest => {
                        log::info!("📩 P2P 收到同步请求");
                        let chain = miner.chain.lock().unwrap();
                        let blocks = chain.chain.clone();
                        drop(chain);
                        broadcaster.send(P2PMessage::SyncResponse(blocks)).ok();
                    }
                    P2PMessage::SyncResponse(blocks) => {
                        log::info!("📩 P2P 收到同步响应 ({} 个区块)", blocks.len());
                        let mut chain = miner.chain.lock().unwrap();
                        if blocks.len() > chain.chain.len() {
                            for block in blocks {
                                chain.add_block(block).ok();
                            }
                            log::info!("同步完成，当前高度 {}", chain.chain.len() - 1);
                        }
                    }
                }
            }
        }
        SwarmEvent::Behaviour(BlockchainBehaviourEvent::Mdns(libp2p::mdns::Event::Discovered(
            list,
        ))) => {
            for (peer_id, addr) in list {
                log::info!("发现新节点: {} @ {}，发起同步", peer_id, addr);
                broadcaster.send(P2PMessage::SyncRequest).ok();
            }
        }
        _ => {}
    }
}

/// 启动 P2P 节点，返回发送端用于广播消息
pub fn build_p2p(
    signing_key: &SigningKey,
    miner: Miner,
    listen_port: u16,
) -> mpsc::UnboundedSender<P2PMessage> {
    let (mut swarm, topic) = build_swarm(signing_key);

    let listen_addr: libp2p::Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", listen_port)
        .parse()
        .unwrap();
    swarm.listen_on(listen_addr).unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel();

    let tx_clone = tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                event = swarm.select_next_some() => {
                    handle_p2p_event(event, &miner, &tx_clone).await;
                }
                Some(msg) = rx.recv() => {
                    publish_message(&mut swarm, &topic, msg);
                }
            }
        }
    });

    tx
}
