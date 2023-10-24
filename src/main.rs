use clap::{Parser, Subcommand};
use pieces::Pieces;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value};
use sha1::{Digest, Sha1};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Decode { value: String },
    Info { torrent: PathBuf },
}

#[derive(Debug, Clone, Deserialize)]
struct Torrent {
    announce: String,
    info: Info,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Info {
    length: usize,
    name: String,
    #[serde(rename = "piece length")]
    plength: usize,
    pieces: Pieces,
}

fn decode_bencoded_value(encoded_value: &str) -> (Value, &str) {
    match encoded_value.chars().next() {
        // Number encoded
        Some('i') => {
            if let Some((n, rest)) =
                encoded_value
                    .split_at(1)
                    .1
                    .split_once('e')
                    .and_then(|(digits, rest)| {
                        let n = digits.parse::<i64>().ok()?;
                        Some((n, rest))
                    })
            {
                return (n.into(), rest);
            }
        }
        // List encoded
        Some('l') => {
            let mut elems = Vec::new();
            let mut rest = encoded_value.split_at(1).1;
            while !rest.is_empty() && !rest.starts_with('e') {
                let (e, reminder) = decode_bencoded_value(rest);
                elems.push(e);
                rest = reminder;
            }
            return (elems.into(), &rest[1..]);
        }
        // List encoded
        Some('d') => {
            let mut dict = serde_json::Map::new();
            let mut rest = encoded_value.split_at(1).1;
            while !rest.is_empty() && !rest.starts_with('e') {
                let (k, reminder) = decode_bencoded_value(rest);
                let k = match k {
                    Value::String(k) => k,
                    _ => panic!("invalid key"),
                };
                let (v, reminder) = decode_bencoded_value(reminder);
                dict.insert(k.to_string(), v);
                rest = reminder;
            }
            return (dict.into(), &rest[1..]);
        }
        // String encoded
        Some(c) if c.is_ascii_digit() => {
            if let Some((len, rest)) = encoded_value.split_once(':') {
                let len = len.parse::<usize>().unwrap();
                return (rest[..len].to_string().into(), &rest[len..]);
            }
        }
        _ => {}
    }
    panic!("Unhandled encoded value: {}", encoded_value)
}

#[test]
fn decode_str() {
    let encoded = "4:hola";
    let decoded = decode_bencoded_value(encoded);
    assert_eq!(Value::String("hola".to_string()), decoded.0);
}

#[test]
fn decode_number() {
    let encoded = "i52e";
    let decoded = decode_bencoded_value(encoded);
    assert_eq!(Value::Number(52.into()), decoded.0);
}

#[test]
fn decode_list() {
    let encoded = "li52e4:holae";
    let decoded = decode_bencoded_value(encoded);
    let expec = Value::Array(vec![
        Value::Number(52.into()),
        Value::String("hola".to_string()),
    ]);
    assert_eq!(expec, decoded.0);
}

#[test]
fn decode_dict() {
    let encoded = "d3:foo3:bar5:helloi52ee";
    let decoded = decode_bencoded_value(encoded);
    let expec = serde_json::json!({"foo":"bar", "hello": 52});
    assert_eq!(expec, decoded.0);
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        Commands::Decode { value } => {
            let decoded_value = decode_bencoded_value(&value);
            println!("{}", decoded_value.0);
        }

        Commands::Info { torrent } => {
            let file = std::fs::read(torrent)?;
            let t: Torrent = serde_bencode::from_bytes(&file)?;
            let mut hasher = Sha1::new();
            let encoded = serde_bencode::to_bytes(&t.info)?;
            hasher.update(&encoded);
            let info_hash = hasher.finalize();

            println!("Tracker URL: {}", t.announce);
            println!("Length: {}", t.info.length);
            println!("Info Hash: {}", hex::encode(info_hash));
            println!("Piece Length: {}", t.info.plength);
            println!("Piece Hashes:");
            for piece in t.info.pieces.0 {
                println!("{}", hex::encode(piece));
            }
        }
    }
    Ok(())
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
