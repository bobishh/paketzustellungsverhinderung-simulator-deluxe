//! Short-lived invitations advertised on the local network.
use std::{
    collections::HashMap,
    io,
    net::{Ipv4Addr, SocketAddrV4, UdpSocket},
    time::{Duration, Instant},
};

use socket2::{Domain, Protocol, Socket, Type};

use super::{Invitation, Origin, decode_invite, invite_link};

const GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 73, 29);
const PORT: u16 = 42169;
const PREFIX: &[u8] = b"paket/ride/1 ";
const MAX_PACKET: usize = 4096;
const LEASE: Duration = Duration::from_secs(7);

pub(crate) fn advertiser() -> io::Result<UdpSocket> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.set_nonblocking(true)?;
    socket.set_multicast_ttl_v4(1)?;
    socket.set_multicast_loop_v4(true)?;
    Ok(socket)
}

pub(crate) fn announce(socket: &UdpSocket, invite: &Invitation) {
    if let Ok(json) = serde_json::to_string(invite) {
        let packet = [PREFIX, json.as_bytes()].concat();
        if packet.len() < MAX_PACKET {
            let _ = socket.send_to(&packet, (GROUP, PORT));
        }
    }
}

pub struct AvailableRide {
    pub origin: Origin,
    pub link: String,
}

pub struct Discovery {
    socket: UdpSocket,
    rides: HashMap<String, (Instant, AvailableRide)>,
}

impl Discovery {
    pub fn listen() -> io::Result<Self> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        socket.set_reuse_address(true)?;
        #[cfg(unix)]
        socket.set_reuse_port(true)?;
        socket.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, PORT).into())?;
        socket.join_multicast_v4(&GROUP, &Ipv4Addr::UNSPECIFIED)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket: socket.into(),
            rides: HashMap::new(),
        })
    }

    pub fn poll(&mut self) -> Vec<(&str, &AvailableRide)> {
        let now = Instant::now();
        let mut packet = [0; MAX_PACKET];
        for _ in 0..32 {
            match self.socket.recv(&mut packet) {
                Ok(size) if size < packet.len() => self.record(&packet[..size], now),
                Ok(_) => {}
                Err(_) => break,
            }
        }
        self.rides
            .retain(|_, (seen, _)| now.duration_since(*seen) < LEASE);
        let mut rides: Vec<_> = self
            .rides
            .iter()
            .map(|(room, (_, ride))| (room.as_str(), ride))
            .collect();
        rides.sort_unstable_by_key(|(room, _)| *room);
        rides
    }

    fn record(&mut self, packet: &[u8], now: Instant) {
        if let Some((endpoint, ride)) = parse_advertisement(packet) {
            self.rides.insert(endpoint, (now, ride));
        }
    }
}

fn parse_advertisement(packet: &[u8]) -> Option<(String, AvailableRide)> {
    let json = std::str::from_utf8(packet.strip_prefix(PREFIX)?).ok()?;
    let invite = decode_invite(json).ok()?;
    let endpoint = invite.addr.id.to_string();
    if invite.room.as_deref() != Some(endpoint.as_str()) || invite.addr.addrs.is_empty() {
        return None;
    }
    Some((
        endpoint,
        AvailableRide {
            origin: invite.origin,
            link: invite_link(json),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearby_ride_provides_a_joinable_invite_for_its_location() {
        let endpoint = iroh::SecretKey::generate().public();
        let origin = Origin {
            lat: 52.42758,
            lon: 13.74116,
        };
        let mut invite = Invitation {
            origin,
            room: Some(endpoint.to_string()),
            addr: iroh::EndpointAddr::new(endpoint)
                .with_ip_addr("192.168.1.10:42000".parse().unwrap()),
        };
        let packet = |invite: &Invitation| [PREFIX, &serde_json::to_vec(invite).unwrap()].concat();
        let (id, listed) = parse_advertisement(&packet(&invite)).unwrap();
        assert_eq!(id, endpoint.to_string());
        assert!(listed.origin.matches(origin));
        assert!(crate::invite_origin(&listed.link).unwrap().matches(origin));
        invite.room = Some("guest".into());
        assert!(parse_advertisement(&packet(&invite)).is_none());
        assert!(parse_advertisement(b"paket/ride/1 invalid").is_none());
    }
}
