//! A neighborhood session on the native MetaMesh runtime.
//!
//! Peers share an origin, avatar poses, and street actions. Scene geometry streams locally.

#[path = "discovery.rs"]
mod discovery;

pub use discovery::{AvailableRide, Discovery};

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use iroh::EndpointAddr;
use meta_mesh_native::{NativeNode, NativeNodeOptions};
use paket_geo::LatLon;
use serde::{Deserialize, Serialize};

const TOPIC: &str = "paket/street";
const INVITE_PREFIX: &str = "paket://join/";

pub use crate::protocol::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Invitation {
    origin: Origin,
    addr: EndpointAddr,
    #[serde(default)]
    room: Option<String>,
}

pub struct SharedStreet {
    pub origin: Origin,
    pub device: String,
    pub invitation: String,
    pub is_host: bool,
    outbound: Sender<Fact>,
    inbound: Mutex<Receiver<Fact>>,
}

pub fn host(origin: Origin) -> Result<SharedStreet, String> {
    spawn_session(origin, None)
}

pub fn invite_link(invitation: &str) -> String {
    format!("{INVITE_PREFIX}{}", URL_SAFE_NO_PAD.encode(invitation))
}

fn decode_invite(input: &str) -> Result<Invitation, String> {
    let input = input.trim();
    let decoded = match input.strip_prefix(INVITE_PREFIX) {
        Some(encoded) => URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|error| error.to_string())?,
        None => input.as_bytes().to_vec(),
    };
    let invite: Invitation = serde_json::from_slice(&decoded).map_err(|error| error.to_string())?;
    if !invite.origin.lat.is_finite()
        || invite.origin.lat.abs() > 85.0
        || !invite.origin.lon.is_finite()
        || invite.origin.lon.abs() > 180.0
    {
        return Err("invalid location in invitation".into());
    }
    Ok(invite)
}

pub fn invite_origin(input: &str) -> Result<Origin, String> {
    decode_invite(input).map(|invite| invite.origin)
}

pub fn join(invitation: &str) -> Result<SharedStreet, String> {
    let invite = decode_invite(invitation)?;
    spawn_session(invite.origin, Some(invite))
}

impl SharedStreet {
    pub fn host_device(&self) -> String {
        decode_invite(&self.invitation)
            .map(|invite| invite.room.unwrap_or_else(|| invite.addr.id.to_string()))
            .unwrap_or_default()
    }
    pub fn publish(&self, fact: Fact) -> Result<(), String> {
        if !fact.origin().matches(self.origin) {
            return Err("fact origin does not match the session".into());
        }
        self.outbound
            .send(fact)
            .map_err(|_| "session stopped".to_string())
    }

    pub fn poll(&self) -> Vec<Fact> {
        self.inbound
            .lock()
            .map(|receiver| receiver.try_iter().collect())
            .unwrap_or_default()
    }
}

fn spawn_session(origin: Origin, bootstrap: Option<Invitation>) -> Result<SharedStreet, String> {
    let is_host = bootstrap.is_none();
    let (outbound, outbound_rx) = mpsc::channel();
    let (inbound_tx, inbound) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();
    thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = ready_tx.send(Err(error.to_string()));
                return;
            }
        };
        if let Err(error) = runtime.block_on(run(
            origin,
            bootstrap,
            outbound_rx,
            inbound_tx,
            ready_tx.clone(),
        )) {
            let _ = ready_tx.send(Err(error.clone()));
            eprintln!("paket session ended: {error}");
        }
    });
    let (device, invitation) = ready_rx
        .recv_timeout(Duration::from_secs(20))
        .map_err(|_| "session did not start".to_string())??;
    Ok(SharedStreet {
        origin,
        device,
        invitation,
        is_host,
        outbound,
        inbound: Mutex::new(inbound),
    })
}

async fn run(
    origin: Origin,
    bootstrap: Option<Invitation>,
    outbound_rx: Receiver<Fact>,
    inbound_tx: Sender<Fact>,
    ready_tx: Sender<Result<(String, String), String>>,
) -> Result<(), String> {
    let node = NativeNode::start_with_options(NativeNodeOptions {
        allow_any: true,
        ..NativeNodeOptions::default()
    })
    .await
    .map_err(|error| error.to_string())?;
    let device = node.endpoint_id().to_string();
    let room = match &bootstrap {
        None => Some(device.clone()),
        Some(invite) => invite.room.clone(),
    };
    let topic = room
        .as_ref()
        .map_or_else(|| TOPIC.into(), |room| format!("{TOPIC}/{room}"));
    let peers = bootstrap.iter().map(|invite| invite.addr.clone()).collect();
    let (sender, mut receiver) = node
        .join_gossip(&topic, peers)
        .await
        .map_err(|error| error.to_string())?;
    if bootstrap.is_none() {
        let deadline = Instant::now() + Duration::from_secs(3);
        while node.addr().relay_urls().next().is_none() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    let mut advertisement = Invitation {
        origin,
        addr: node.addr(),
        room,
    };
    let advertiser = bootstrap
        .is_none()
        .then(discovery::advertiser)
        .and_then(Result::ok);
    let mut last_announcement = Instant::now() - Duration::from_secs(2);
    let invitation = serde_json::to_string(&advertisement).map_err(|error| error.to_string())?;
    let _ = ready_tx.send(Ok((device.clone(), invitation)));

    let mut latest_pose: Option<Fact> = None;
    let mut shift: Option<Fact> = None;
    let mut delivery: Option<Fact> = None;
    let mut actions = VecDeque::new();
    let mut delivery_sent = Instant::now() - Duration::from_secs(1);
    loop {
        if let Some(socket) = &advertiser
            && last_announcement.elapsed() >= Duration::from_secs(2)
        {
            advertisement.addr = node.addr();
            discovery::announce(socket, &advertisement);
            last_announcement = Instant::now();
        }
        let mut disconnected = false;
        loop {
            match outbound_rx.try_recv() {
                Ok(fact) if fact.origin().matches(origin) => match fact {
                    Fact::Shift { .. } => shift = Some(fact),
                    Fact::Pose { .. } | Fact::Actor { .. } => latest_pose = Some(fact),
                    Fact::StreetAction { .. } => {
                        if actions.len() == 64 {
                            actions.pop_front();
                        }
                        actions.push_back((Instant::now(), fact));
                    }
                    Fact::Delivery { .. } => delivery = Some(fact),
                },
                Ok(_) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }
        if delivery_sent.elapsed() >= Duration::from_millis(400) {
            if let Some(fact) = &shift {
                broadcast_fact(&sender, fact).await;
            }
            if let Some(fact) = &delivery {
                broadcast_fact(&sender, fact).await;
            }
            actions.retain(|(created, fact)| {
                matches!(
                    fact,
                    Fact::StreetAction {
                        event: StreetAction::Door { .. },
                        ..
                    }
                ) || created.elapsed() < Duration::from_secs(5)
            });
            // Repeat recent street actions so peers joining mid-tick still receive them.
            for (_, fact) in actions.iter().rev().take(8) {
                broadcast_fact(&sender, fact).await;
            }
            delivery_sent = Instant::now();
        }
        if let Some(fact) = latest_pose.take() {
            broadcast_fact(&sender, &fact).await;
        }
        match tokio::time::timeout(Duration::from_millis(20), receiver.recv()).await {
            Ok(Ok(Some(message))) => {
                if message.delivered_from.to_string() == device {
                    continue;
                }
                match serde_json::from_slice::<Fact>(&message.content) {
                    Ok(fact) if fact.origin().matches(origin) => {
                        let _ = inbound_tx.send(fact);
                    }
                    Ok(fact) => {
                        eprintln!(
                            "paket gossip ignored origin {:.5},{:.5}",
                            fact.origin().lat,
                            fact.origin().lon
                        );
                    }
                    Err(error) => eprintln!("paket gossip ignored message: {error}"),
                }
            }
            Ok(Ok(None)) => break,
            Ok(Err(error)) => eprintln!("paket gossip recv failed: {error}"),
            Err(_) => {}
        }
        if disconnected {
            break;
        }
    }
    node.close().await.map_err(|error| error.to_string())?;
    Ok(())
}

async fn broadcast_fact(sender: &meta_mesh_native::GossipTopicSender, fact: &Fact) {
    let bytes = match serde_json::to_vec(fact) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("paket gossip encode failed: {error}");
            return;
        }
    };
    match tokio::time::timeout(Duration::from_millis(200), sender.broadcast(bytes)).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => eprintln!("paket gossip send failed: {error}"),
        Err(_) => eprintln!("paket gossip send timed out"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_and_legacy_json_decode_to_the_same_origin() {
        let origin = Origin {
            lat: 52.491_279_403_127_82,
            lon: 13.394_749_603_108_613,
        };
        let invite = Invitation {
            origin,
            addr: EndpointAddr::new(iroh::SecretKey::generate().public()),
            room: Some("ride".into()),
        };
        let json = serde_json::to_string(&invite).unwrap();
        let url = invite_link(&json);
        assert!(invite_origin(&url).unwrap().matches(origin));
        assert!(invite_origin(&json).unwrap().matches(origin));
        assert!(invite_origin("paket://join/!!!").is_err());
    }

    #[test]
    fn old_pose_snapshots_receive_defaults() {
        let pose: Fact = serde_json::from_str(r#"{"kind":"pose","origin":{"lat":52.49,"lon":13.39},"device":"peer","east":3,"north":6,"heading":0.2}"#).unwrap();
        assert!(matches!(
            pose,
            Fact::Pose {
                up: 0.7,
                velocity_east: 0.0,
                velocity_north: 0.0,
                sequence: 0,
                ..
            }
        ));
    }
}
