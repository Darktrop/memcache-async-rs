use crate::error::*;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::runtime::TaskExecutor;
use crate::transport::tcp;
use crate::codec::header::*;
use crate::transport::tcp::TransportClient;
use std::io::{Cursor, Read, Write};
use byteorder::{ReadBytesExt, BigEndian};

pub trait FromMemcacheValue {
    fn from_memcache_value<R>(read: &mut R, size: usize) -> Self where R: Read;
}

trait IntoMemcacheValue {
    fn intoMemcacheValue(&self) -> Vec<u8>;
}

impl FromMemcacheValue for Vec<u8> {
    fn from_memcache_value<R>(read: &mut R, size: usize) -> Self where R: Read {
        let mut buf = vec![0_u8; size];
        read.read_exact(&mut buf).unwrap();
        buf
    }
}

impl FromMemcacheValue for String {
    fn from_memcache_value<R>(read: &mut R, size: usize) -> Self where R: Read {
        let mut buf = String::new();
        read.read_to_string(&mut buf).unwrap();
        buf
    }
}

pub struct MemcacheTransport {
    executor: TaskExecutor
}

impl MemcacheTransport {
    pub fn new(executor: tokio::runtime::TaskExecutor) -> Self {
        MemcacheTransport {
            executor
        }
    }

    pub async fn connect(&mut self, uri: String) -> Result<MemcacheClient> {
        let (f, client) = tcp::Transport::connect(uri).await?;
        self.executor.spawn(f.then(|r| {
            if let Err(e) = r {
                error!("Error happens: {}" , e);
            }
            futures::future::ready(())
        }));
        Ok(MemcacheClient {
            client
        })
    }
}

#[derive(Clone)]
pub struct MemcacheClient {
    client: TransportClient
}

impl MemcacheClient {
    pub async fn get<T>(&mut self, key: String) -> Result<T>
    where T:FromMemcacheValue {
        let header = MemcacheClient::create_get_request(&key)?;
        let mut request = vec!();
        header.write(&mut request)?;
        request.extend_from_slice(key.as_bytes());
        debug!("sending get with key: {}", key);
        let mut responsestream = self.client.query(&request).await?;
        let mut cursor = Cursor::new(Vec::new());
        while let Some(item) = responsestream.rx.next().await {
            debug!("received chunk {} / {}", item.part, item.total);
            cursor.write_all(&item.data);
        }
        cursor.set_position(0);
        let header = PacketHeader::read(&mut cursor)?;
        let flags = cursor.read_u32::<BigEndian>()?;
        let body_length = header.total_body_length as usize- header.extras_length as usize;
        Ok(T::from_memcache_value(&mut cursor, body_length))
    }

    async fn set<T>(&self, key: String, value: T) -> Result<()>
    where T:IntoMemcacheValue {
        unimplemented!()
    }

    fn create_get_request(key: &str) -> Result<PacketHeader> {
        if key.len() > 250 {
            return Err(ErrorKind::ClientError(String::from("key is too long")).into());
        }
        let request_header = PacketHeader {
            magic: Magic::Request as u8,
            opcode: Opcode::Get as u8,
            key_length: key.len() as u16,
            total_body_length: key.len() as u32,
            ..Default::default()
        };
        Ok(request_header)
    }
}