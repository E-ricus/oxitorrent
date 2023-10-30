use anyhow::Result;
use bytes::{BufMut, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub fn as_bytes_mut<T: Sized>(data: &mut T) -> &mut [u8] {
    let ptr = data as *mut T as *mut u8;
    let len = std::mem::size_of::<T>();
    unsafe { std::slice::from_raw_parts_mut(ptr, len) }
}

#[repr(C)]
pub struct Handshake {
    pub length: u8,
    pub protocol: [u8; 19],
    pub reserved_bytes: [u8; 8],
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl Handshake {
    pub fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        Self {
            length: 19,
            protocol: *b"BitTorrent protocol",
            reserved_bytes: [0; 8],
            info_hash,
            peer_id,
        }
    }
}

#[repr(C)]
pub struct Request {
    index: [u8; 4],
    begin: [u8; 4],
    length: [u8; 4],
}

impl Request {
    pub fn new(index: u32, begin: u32, length: u32) -> Self {
        Self {
            index: index.to_be_bytes(),
            begin: begin.to_be_bytes(),
            length: length.to_be_bytes(),
        }
    }

    pub fn index(&self) -> u32 {
        u32::from_be_bytes(self.index)
    }
    pub fn begin(&self) -> u32 {
        u32::from_be_bytes(self.begin)
    }
    pub fn length(&self) -> u32 {
        u32::from_be_bytes(self.length)
    }
}

#[derive(Debug)]
pub struct Piece {
    index: [u8; 4],
    begin: [u8; 4],
    block: Vec<u8>,
}

impl Piece {
    pub fn from_u8(bytes: &[u8]) -> Result<Self> {
        Ok(Self {
            index: bytes[..4].try_into()?,
            begin: bytes[4..8].try_into()?,
            block: bytes[8..].to_vec(),
        })
    }

    pub fn index(&self) -> u32 {
        u32::from_be_bytes(self.index)
    }
    pub fn begin(&self) -> u32 {
        u32::from_be_bytes(self.begin)
    }
    pub fn block(&self) -> &[u8] {
        &self.block
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub tag: MessageTag,
    pub payload: Vec<u8>,
}

impl Message {
    fn to_bytes(&self) -> BytesMut {
        let mut buffer = BytesMut::new();

        let len_slice = u32::to_be_bytes(self.payload.len() as u32 + 1);

        buffer.reserve(4 + self.payload.len() + 1);

        // Write the length and string to the buffer.
        buffer.extend_from_slice(&len_slice);
        buffer.put_u8(self.tag as u8);
        buffer.extend_from_slice(&self.payload);
        buffer
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageTag {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
}

impl MessageTag {
    fn from_u8(tag: u8) -> Result<Self> {
        let message_tag = match tag {
            0 => MessageTag::Choke,
            1 => MessageTag::Unchoke,
            2 => MessageTag::Interested,
            3 => MessageTag::NotInterested,
            4 => MessageTag::Have,
            5 => MessageTag::Bitfield,
            6 => MessageTag::Request,
            7 => MessageTag::Piece,
            8 => MessageTag::Cancel,
            tag => {
                return Err(anyhow::anyhow!("Unknown tag: {}", tag));
            }
        };
        Ok(message_tag)
    }
}

#[allow(dead_code)]
pub struct Peer {
    stream: TcpStream,
    peer_id: [u8; 20],
}

impl Peer {
    pub fn new(stream: TcpStream, peer_id: [u8; 20]) -> Self {
        Self { stream, peer_id }
    }

    pub async fn send_message(&mut self, message: Message) -> Result<()> {
        eprintln!("Sending message: {:?}", message);
        let bytes = message.to_bytes();
        self.stream.write_all(&bytes).await?;

        eprintln!("Message sent!\n");

        Ok(())
    }

    pub async fn read_message(&mut self) -> Result<Message> {
        let mut message_length: [u8; 4] = [0; 4];

        self.stream.read_exact(&mut message_length).await?;

        let message_length = u32::from_be_bytes(message_length);
        eprintln!("Length: {}\n", message_length);

        let mut message_type: [u8; 1] = [0; 1];
        self.stream.read_exact(&mut message_type).await?;

        let tag = message_type[0];
        let message_tag = MessageTag::from_u8(tag)?;

        eprintln!("Message type: {:?}\n", message_tag);

        let mut payload: Vec<u8> = vec![0; message_length as usize - 1];
        // Read a message of length message_length - 1 (message_type is already read)
        self.stream.read_exact(&mut payload).await?;
        eprintln!("Length of recieved payload: {}\n", payload.len());

        let message = Message {
            tag: message_tag,
            payload,
        };
        Ok(message)
    }
}
