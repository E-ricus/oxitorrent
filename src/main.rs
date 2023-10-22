use serde_json::{self, Value};
use std::env;

// Available if you need it!
// use serde_bencode

fn decode_bencoded_value(encoded_value: &str) -> Value {
    let mut iter = encoded_value.chars();
    match iter.next() {
        // String encoded
        Some(c) if c.is_ascii_digit() => {
            // TODO: Refactor
            let colon_index = encoded_value.find(':').unwrap();
            let number_string = &encoded_value[..colon_index];
            let number = number_string.parse::<i64>().unwrap();
            let string = &encoded_value[colon_index + 1..colon_index + 1 + number as usize];
            Value::String(string.to_string())
        }
        Some('i') => {
            if let Some('e') = iter.next_back() {
                let number = iter.collect::<String>();
                Value::Number(number.parse().unwrap())
            } else {
                panic!("Unhandled encoded value: {}", encoded_value)
            }
        }
        _ => panic!("Unhandled encoded value: {}", encoded_value),
    }
}

#[test]
fn decode_str() {
    let encoded = "4:hola";
    let decoded = decode_bencoded_value(encoded);
    assert_eq!(Value::String("hola".to_string()), decoded);
}

#[test]
fn decode_number() {
    let encoded = "i52e";
    let decoded = decode_bencoded_value(encoded);
    assert_eq!(Value::Number(52.into()), decoded);
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value);
    } else {
        println!("unknown command: {}", args[1])
    }
}
