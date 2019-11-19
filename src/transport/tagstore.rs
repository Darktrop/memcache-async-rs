use std::collections::HashMap;
use crate::transport::request::{ResponseChunk, RequestCallback};
use std::collections::hash_map::Entry;
use crate::error::*;

pub(crate) struct TagStore {
    hash_map: HashMap<u16, RequestCallback>
}

impl TagStore {
    pub fn new(capacity: usize) -> Self {
        TagStore {
            hash_map: HashMap::with_capacity(capacity)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.hash_map.is_empty()
    }

    pub fn add(&mut self, req: RequestCallback) {
        self.hash_map.insert(req.tag, req);
    }

    pub fn add_response(&mut self, chunk: ResponseChunk) -> Result<()> {
        match self.hash_map.entry(chunk.tag) {
            Entry::Occupied(mut v) => {
                let callback = v.get_mut();
                callback.sent = Some(callback.sent.unwrap_or(0) + 1);
                if callback.total.is_none() {
                    callback.total = Some(chunk.total);
                }
                if let Err(_) = callback.tx.try_send(chunk) {
                    trace!("Client closed channel");
                    v.remove();
                } else {
                    if callback.total.unwrap() == callback.sent.unwrap() {
                        v.remove();
                    }
                }
                Ok(())
            },
            Entry::Vacant(_) => {
                return Err(ErrorKind::ClientError(format!("Received result for non existing tag {}", chunk.tag)).into())
            },
        }
    }
}