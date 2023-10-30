pub mod peer;
pub mod torrent;
pub mod tracker;
pub mod worker;

use serde_json::{self, Value};

pub fn decode_bencoded_value(encoded_value: &str) -> (Value, &str) {
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
