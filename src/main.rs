use bittorrent_starter_rust::torrent::Torrent;
use clap::{Parser, Subcommand};
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

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        Commands::Decode { value } => {
            let decoded_value = bittorrent_starter_rust::decode_bencoded_value(&value);
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
