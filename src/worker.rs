// TODO: Create a worker queue that will have all the parts
#![allow(dead_code)]

use crate::peer::Peer;

pub struct Worker {
    queue: Vec<Part>,
    peers: Vec<Peer>,
}

struct Part {
    piece_index: usize,
}
