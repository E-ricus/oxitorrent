use anyhow::{Context, Result};
use bittorrent_starter_rust::peer::{self, *};
use bittorrent_starter_rust::torrent::Torrent;
use bittorrent_starter_rust::tracker::{self, TrackerRequest, TrackerResponse};
use clap::{Parser, Subcommand};
use sha1::{Digest, Sha1};
use std::io::Write;
use std::net::SocketAddrV4;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const BLOCK_MAX: u32 = 16384;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
#[clap(rename_all = "snake_case")]
enum Commands {
    Decode {
        value: String,
    },
    Info {
        torrent: PathBuf,
    },
    Peers {
        torrent: PathBuf,
    },
    Handshake {
        torrent: PathBuf,
        peer: String,
    },
    DownloadPiece {
        #[arg(short)]
        output: PathBuf,
        torrent: PathBuf,
        piece_index: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
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
        Commands::DownloadPiece {
            output,
            torrent,
            piece_index,
        } => {
            let file = std::fs::read(torrent)?;
            let t: Torrent = serde_bencode::from_bytes(&file)?;
            let info_hash = t.info_hash()?;
            assert!(piece_index < t.info.pieces.0.len());

            // Get Peer
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
            let tracker_info: TrackerResponse = serde_bencode::from_bytes(&response)?;

            // TODO: Use all the peers
            let peer = tracker_info.peers.0[1];

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
            eprintln!("Peer ID: {}", hex::encode(handshake.peer_id));

            let mut peer = Peer::new(connection, handshake.peer_id);
            let msg_bitfield = peer.read_message().await?;
            // Bitfield has to be th first message always
            assert_eq!(msg_bitfield.tag, MessageTag::Bitfield);
            eprintln!("Got bitfield");

            // Send interested
            peer.send_message(Message {
                tag: MessageTag::Interested,
                payload: Vec::new(),
            })
            .await?;
            eprintln!("sent interested");

            // Await for unchoke
            let msg_unchocked = peer.read_message().await?;
            // Bitfield has to be th first message always
            assert_eq!(msg_unchocked.tag, MessageTag::Unchoke);
            assert!(msg_unchocked.payload.is_empty());
            eprintln!("got unchocked");

            // Request a piece by blocks
            let piece_hash = &t.info.pieces.0[piece_index];

            let piece_size = (t.info.plength).min(t.info.length - t.info.plength * piece_index);

            let mut blocks: Vec<u8> = Vec::with_capacity(piece_size);
            loop {
                let block_size = BLOCK_MAX.min((piece_size - blocks.len()) as u32);
                let mut request = Request::new(piece_index as u32, blocks.len() as u32, block_size);

                peer.send_message(Message {
                    tag: MessageTag::Request,
                    payload: Vec::from(peer::as_bytes_mut(&mut request)),
                })
                .await?;

                // Waits for a piece
                let piece_msg = peer.read_message().await?;
                assert_eq!(piece_msg.tag, MessageTag::Piece);
                assert!(!piece_msg.payload.is_empty());

                let piece = Piece::from_u8(&piece_msg.payload[..])?;
                assert_eq!(piece.block().len(), block_size as usize);
                blocks.extend(piece.block());
                if blocks.len() >= piece_size {
                    break;
                }
            }

            assert_eq!(blocks.len(), piece_size);
            let mut hasher = Sha1::new();
            hasher.update(&blocks);
            let hash: [u8; 20] = hasher.finalize().try_into()?;
            assert_eq!(&hash, piece_hash);

            let mut file = std::fs::File::create(output).context("Creating output file failed")?;

            file.write_all(&blocks)
                .context("Writing to output file failed")?;

            file.flush().context("Output file flush failed")?;
        }
    }
    Ok(())
}
