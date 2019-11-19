use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};

pub struct Request {
    pub data: Vec<u8>,
    pub tx: UnboundedSender<ResponseChunk>
}

pub struct RequestCallback {
    pub tag: u16,
    pub sent: Option<u16>,
    pub total: Option<u16>,
    pub tx: UnboundedSender<ResponseChunk>
}

pub struct ResponseChunk {
    pub tag: u16,
    pub part: u16,
    pub total: u16,
    pub data: Vec<u8>
}

pub struct ResponseStream {
    pub rx: UnboundedReceiver<ResponseChunk>
}