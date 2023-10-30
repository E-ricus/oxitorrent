use serde::{Deserialize, Serialize};

use peers::Peers;

#[derive(Debug, Serialize)]
pub struct TrackerRequest {
    pub peer_id: String,
    pub port: u16,
    pub uploaded: usize,
    pub downloaded: usize,
    pub left: usize,
    pub compact: u8,
}

pub fn hash_encoder(t: &[u8; 20]) -> String {
    let mut encoded = String::with_capacity(3 * t.len());
    for &byte in t {
        encoded.push('%');
        encoded.push_str(&hex::encode([byte]));
    }
    encoded
}

#[derive(Debug, Deserialize)]
pub struct TrackerResponse {
    pub interval: usize,
    pub peers: Peers,
}

mod peers {
    use std::{
        fmt,
        net::{Ipv4Addr, SocketAddrV4},
    };

    use serde::{
        de::{self, Visitor},
        Deserialize, Deserializer,
    };

    #[derive(Debug, Clone)]
    pub struct Peers(pub Vec<SocketAddrV4>);

    struct PeerVisitor;

    impl<'de> Visitor<'de> for PeerVisitor {
        type Value = Peers;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a byte string with length multiple of 20")
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v.len() % 6 != 0 {
                return Err(E::custom("length is not correct"));
            }
            Ok(Peers(
                v.chunks_exact(6)
                    .map(|s| {
                        SocketAddrV4::new(
                            Ipv4Addr::new(s[0], s[1], s[2], s[3]),
                            u16::from_be_bytes([s[4], s[5]]),
                        )
                    })
                    .collect(),
            ))
        }
    }

    impl<'de> Deserialize<'de> for Peers {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_bytes(PeerVisitor)
        }
    }
}
