use crate::error::*;
use tokio::prelude::*;
use tokio::runtime::TaskExecutor;
use crate::transport::transport;
use crate::codec::header::*;
use crate::transport::transport::TransportClient;
use std::io::{Cursor, Read, Write};
use byteorder::{ReadBytesExt, BigEndian, WriteBytesExt};
use crate::codec;
use futures::AsyncWriteExt;

pub trait FromMemcacheValue: Sized {
    fn from_memcache_value<R :Read>(read: &mut R, size: usize, flags:u32) -> Result<Self>;
}

pub trait ToMemcacheValue {
    fn get_flags(&self) -> u32;
    fn get_length(&self) -> usize;
    fn write_to<W: Write>(&self, stream: &mut W) -> Result<()>;
}

impl FromMemcacheValue for Vec<u8> {
    fn from_memcache_value<R: Read>(read: &mut R, size: usize, _:u32) -> Result<Self> {
        let mut buf = vec![0_u8; size];
        read.read_exact(&mut buf).unwrap();
        Ok(buf)
    }
}

impl FromMemcacheValue for String {
    fn from_memcache_value<R: Read>(read: &mut R, _: usize, _:u32) -> Result<Self> {
        let mut buf = String::new();
        read.read_to_string(&mut buf).unwrap();
        Ok(buf)
    }
}

impl ToMemcacheValue for String {
    fn get_flags(&self) -> u32 {
        0
    }

    fn get_length(&self) -> usize {
        self.as_bytes().len()
    }

    fn write_to<W: Write>(&self, stream: &mut W) -> Result<()> {
        Ok(stream.write_all(self.as_bytes())?)
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
        let (f, client) = transport::Transport::tcp_connect(uri).await?;
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
    pub async fn get<T: FromMemcacheValue>(&mut self, key: String) -> Result<Option<T>> {
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

        let mut request = vec!();
        request_header.write(&mut request)?;
        request.extend_from_slice(key.as_bytes());
        debug!("sending get with key: {}", key);
        let mut responsestream = self.client.query(&request).await?;
        let mut cursor = Cursor::new(Vec::new());
        while let Some(item) = responsestream.rx.next().await {
            debug!("received chunk {} / {}", item.part, item.total);
            std::io::Write::write_all(&mut cursor, &item.data)?;
        }
        cursor.set_position(0);
        return codec::parse_get_response(&mut cursor);
    }

    pub async fn set<T: ToMemcacheValue>(&mut self, key: String, value: T, expiration: u32) -> Result<()> {
        if key.len() > 250 {
            return Err(ErrorKind::ClientError(String::from("key is too long")).into());
        }
        let request_header = PacketHeader {
            magic: Magic::Request as u8,
            opcode: Opcode::Set as u8,
            key_length: key.len() as u16,
            extras_length: 8,
            total_body_length: (8 + key.len() + value.get_length()) as u32,
            ..Default::default()
        };
        let mut request = Cursor::new(vec!());
        request_header.write(&mut request)?;
        request.write_u32::<BigEndian>(value.get_flags())?; // flags
        request.write_u32::<BigEndian>(expiration)?; // expiration
        std::io::Write::write_all(&mut request, key.as_bytes())?;
        value.write_to(&mut request)?;
        debug!("sending set with key: {}", key);
        let mut responsestream = self.client.query(&request.into_inner()).await?;

        let mut cursor = Cursor::new(Vec::new());
        while let Some(item) = responsestream.rx.next().await {
            debug!("received chunk {} / {}", item.part, item.total);
            std::io::Write::write_all(&mut cursor, &item.data)?;
        }
        cursor.set_position(0);
        return codec::parse_header_only_response(&mut cursor);
    }
}