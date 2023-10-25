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

pub fn as_bytes_mut(data: &mut Handshake) -> &mut [u8] {
    let ptr = data as *mut Handshake as *mut u8;
    let len = std::mem::size_of::<Handshake>();
    unsafe { std::slice::from_raw_parts_mut(ptr, len) }
}
