use bittorrent_starter_rust::peer::{self, Handshake};
use bittorrent_starter_rust::torrent::Torrent;
use bittorrent_starter_rust::tracker::{self, TrackerRequest, TrackerResponse};
use clap::{Parser, Subcommand};
use std::net::SocketAddrV4;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

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
    Peers { torrent: PathBuf },
    Handshake { torrent: PathBuf, peer: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        Commands::Decode { value } => {
            let decoded_value = bittorrent_starter_rust::decode_bencoded_value(&value);
            println!("{}", decoded_value.0);
        }

        Commands::Info { torrent } => {
            let file = std::fs::read(torrent)?;
            let t: Torrent = serde_bencode::from_bytes(&file)?;
            let info_hash = t.info_hash()?;

            println!("Tracker URL: {}", t.announce);
            println!("Length: {}", t.info.length);
            println!("Info Hash: {}", hex::encode(info_hash));
            println!("Piece Length: {}", t.info.plength);
            println!("Piece Hashes:");
            for piece in t.info.pieces.0 {
                println!("{}", hex::encode(piece));
            }
        }
        Commands::Peers { torrent } => {
            let file = std::fs::read(torrent)?;
            let t: Torrent = serde_bencode::from_bytes(&file)?;
            let info_hash = t.info_hash()?;

            let tracker_request = TrackerRequest {
                peer_id: String::from("00112233445566778899"),
                port: 6881,
                uploaded: 0,
                downloaded: 0,
                left: t.info.length,
                compact: 1,
            };

            let query = serde_urlencoded::to_string(&tracker_request)?;
            let url = format!(
                "{}?{}&info_hash={}",
                t.announce,
                query,
                tracker::hash_encoder(&info_hash)
            );
            let response = reqwest::get(url).await?;
            let response = response.bytes().await?;
            let response: TrackerResponse = serde_bencode::from_bytes(&response)?;
            for peer in response.peers.0 {
                println!("{peer}");
            }
        }
        Commands::Handshake { torrent, peer } => {
            let file = std::fs::read(torrent)?;
            let t: Torrent = serde_bencode::from_bytes(&file)?;
            let info_hash = t.info_hash()?;

            let peer = peer.parse::<SocketAddrV4>()?;
            let mut connection = TcpStream::connect(peer).await?;

            let mut handshake = Handshake::new(info_hash, *b"00112233445566778899");

            // Drops unsafe slice pointer after reading it
            {
                // Generates a mutable slice pointer to handshake
                let bytes = peer::as_bytes_mut(&mut handshake);

                connection.write_all(bytes).await?;

                // Reads to the same bytes slice pointing to the handshake struct
                connection.read_exact(bytes).await?;
            }
            println!("Peer ID: {}", hex::encode(handshake.peer_id));
        }
    }
    Ok(())
}
