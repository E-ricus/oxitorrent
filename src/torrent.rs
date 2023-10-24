use pieces::Pieces;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct Torrent {
    pub announce: String,
    pub info: Info,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Info {
    pub length: usize,
    pub name: String,
    #[serde(rename = "piece length")]
    pub plength: usize,
    pub pieces: Pieces,
}

mod pieces {
    use std::fmt;

    use serde::{
        de::{self, Visitor},
        Deserialize, Deserializer, Serialize, Serializer,
    };

    #[derive(Debug, Clone)]
    pub struct Pieces(pub Vec<[u8; 20]>);

    struct PiecesVisitor;

    impl<'de> Visitor<'de> for PiecesVisitor {
        type Value = Pieces;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a byte string with length multiple of 20")
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v.len() % 20 != 0 {
                return Err(E::custom("length is not correct"));
            }
            Ok(Pieces(
                v.chunks_exact(20)
                    .map(|s| s.try_into().expect("length is 20"))
                    .collect(),
            ))
        }
    }

    impl<'de> Deserialize<'de> for Pieces {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_i32(PiecesVisitor)
        }
    }

    impl Serialize for Pieces {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let slice = self.0.concat();
            serializer.serialize_bytes(&slice)
        }
    }
}
