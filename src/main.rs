use anyhow::{self, Context, Result};
use bittorrent_starter_rust::peer::{self, *};
use bittorrent_starter_rust::torrent::Torrent;
use bittorrent_starter_rust::tracker::{self, TrackerRequest, TrackerResponse};
use clap::{Parser, Subcommand};
use sha1::{Digest, Sha1};
use std::fs;
use std::io::Write;
use std::net::SocketAddrV4;
use std::path::PathBuf;

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
    Download {
        #[arg(short)]
        output: PathBuf,
        torrent: PathBuf,
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
            let torrent = read_torrent(torrent)?;
            let info_hash = torrent.info_hash()?;
            println!("Tracker URL: {}", torrent.announce);
            println!("Length: {}", torrent.info.length);
            println!("Info Hash: {}", hex::encode(info_hash));
            println!("Piece Length: {}", torrent.info.plength);
            println!("Piece Hashes:");
            for piece in torrent.info.pieces.0 {
                println!("{}", hex::encode(piece));
            }
        }
        Commands::Peers { torrent } => {
            let torrent = read_torrent(torrent)?;

            let peers = get_peers(&torrent).await?;
            for peer in peers {
                println!("{peer}");
            }
        }
        Commands::Handshake { torrent, peer } => {
            let torrent = read_torrent(torrent)?;
            let info_hash = torrent.info_hash()?;

            let peer = peer.parse::<SocketAddrV4>()?;
            let peer = Peer::connect_peer(peer, info_hash).await?;

            println!("Peer ID: {}", hex::encode(peer.peer_id));
        }
        Commands::DownloadPiece {
            output,
            torrent,
            piece_index,
        } => {
            download_piece(torrent, output, piece_index).await?;
        }
        Commands::Download { output, torrent } => {
            download(torrent, output).await?;
        }
    }
    Ok(())
}

fn read_torrent(torrent: PathBuf) -> Result<Torrent> {
    let file = fs::read(torrent)?;
    Ok(serde_bencode::from_bytes::<Torrent>(&file)?)
}

async fn get_peers(torrent: &Torrent) -> Result<Vec<SocketAddrV4>> {
    let info_hash = torrent.info_hash()?;
    let tracker_request = TrackerRequest {
        peer_id: String::from("00112233445566778899"),
        port: 6881,
        uploaded: 0,
        downloaded: 0,
        left: torrent.info.length,
        compact: 1,
    };

    let query = serde_urlencoded::to_string(&tracker_request)?;
    let url = format!(
        "{}?{}&info_hash={}",
        torrent.announce,
        query,
        tracker::hash_encoder(&info_hash)
    );
    let response = reqwest::get(url).await?;
    let response = response.bytes().await?;
    let response: TrackerResponse = serde_bencode::from_bytes(&response)?;
    Ok(response.peers.0)
}

async fn download_piece(torrent: PathBuf, output: PathBuf, piece_index: usize) -> Result<()> {
    let torrent = read_torrent(torrent)?;
    let info_hash = torrent.info_hash()?;
    assert!(piece_index < torrent.info.pieces.0.len());

    let peers = get_peers(&torrent).await?;
    let peer_address = peers[0];

    let mut peer = Peer::connect_peer(peer_address, info_hash).await?;

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
    request_piece(&torrent, piece_index, &mut peer, output).await?;
    Ok(())
}

async fn download(torrent: PathBuf, output: PathBuf) -> Result<()> {
    let torrent = read_torrent(torrent)?;
    let info_hash = torrent.info_hash()?;

    let peers = get_peers(&torrent).await?;
    // TODO: Use the worker module to add each peer to allow the download of each part simultaneously
    let peer_address = peers[0];

    let mut peer = Peer::connect_peer(peer_address, info_hash).await?;

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

    let mut pieces: Vec<u8> = Vec::with_capacity(torrent.info.length);
    for piece_index in 0..torrent.info.pieces.0.len() {
        let path = format!("{}-part-{piece_index}", output.to_str().unwrap()).into();
        let piece = request_piece(&torrent, piece_index, &mut peer, path).await?;
        pieces.extend(piece);
    }

    let mut file = fs::File::create(output).context("Creating output file failed")?;
    file.write_all(&pieces)
        .context("Writing to output file failed")?;
    file.flush().context("Output file flush failed")?;

    Ok(())
}

async fn request_piece(
    torrent: &Torrent,
    piece_index: usize,
    peer: &mut Peer,
    output: PathBuf,
) -> Result<Vec<u8>> {
    let piece_hash = &torrent.info.pieces.0[piece_index];
    let piece_size =
        (torrent.info.plength).min(torrent.info.length - torrent.info.plength * piece_index);

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

    let mut file = fs::File::create(output).context("Creating output file failed")?;
    file.write_all(&blocks)
        .context("Writing to output file failed")?;
    file.flush().context("Output file flush failed")?;
    Ok(blocks)
}
