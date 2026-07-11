use crate::block::Block;
use crate::mempool::Miner;
use crate::transaction::Transaction;
use ed25519_dalek::SigningKey;
use libp2p::SwarmBuilder;
use libp2p::gossipsub::{self, MessageId};
use libp2p::identity;
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use serde::{Deserialize, Serialize};

/// P2P 网络里交换的消息类型
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum P2PMessage {
    NewBlock(Block),
    NewTransaction(Transaction),
}

/// libp2p 的行为组合
#[derive(NetworkBehaviour)]
pub struct BlockchainBehaviour {
    pub gossip: gossipsub::Behaviour,
    pub mdns: libp2p::mdns::tokio::Behaviour,
}

pub fn build_p2p(signing_key: &SigningKey) -> libp2p::swarm::Swarm<BlockchainBehaviour> {
    // 将 ed25519-dalek SigningKey 转为 libp2p Keypair
    let keypair = identity::Keypair::ed25519_from_bytes(signing_key.to_bytes()).unwrap();
    let peer_id = keypair.public().to_peer_id();

    // 1. 配置 gossipsub（使用 Miner 的 identity）
    let message_id_fn = |message: &gossipsub::Message| MessageId::from(&message.data[..20]);
    let gossip_config = gossipsub::ConfigBuilder::default()
        .validation_mode(gossipsub::ValidationMode::Permissive)
        .message_id_fn(message_id_fn)
        .build()
        .unwrap();

    let gossip = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(keypair.clone()),
        gossip_config,
    )
    .unwrap();

    // 2. 配置 mDNS（使用 Miner 的 peer_id）
    let mdns =
        libp2p::mdns::tokio::Behaviour::new(libp2p::mdns::Config::default(), peer_id).unwrap();

    let behaviour = BlockchainBehaviour { gossip, mdns };

    // 3. 用 SwarmBuilder 构建传输 + swarm
    SwarmBuilder::with_existing_identity(keypair)
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
        .build()
}

pub async fn handle_p2p_event(
    event: libp2p::swarm::SwarmEvent<BlockchainBehaviourEvent>,
    miner: &Miner,
) {
    match event {
        // gossipsub 收到消息
        SwarmEvent::Behaviour(BlockchainBehaviourEvent::Gossip(gossipsub::Event::Message {
            message,
            ..
        })) => {
            if let Ok(msg) = serde_json::from_slice::<P2PMessage>(&message.data) {
                match msg {
                    P2PMessage::NewBlock(block) => {
                        println!("P2P 收到新区块 #{}", block.index);
                        let mut chain = miner.chain.lock().unwrap();
                        chain.add_block(block).ok();
                    }
                    P2PMessage::NewTransaction(tx) => {
                        println!("P2P 收到新交易");
                        miner.submit_tx(tx).ok();
                    }
                }
            }
        }
        // mDNS 发现新节点
        SwarmEvent::Behaviour(BlockchainBehaviourEvent::Mdns(libp2p::mdns::Event::Discovered(
            list,
        ))) => {
            for (peer_id, addr) in list {
                println!("发现新节点: {} @ {}", peer_id, addr);
            }
        }
        _ => {}
    }
}
