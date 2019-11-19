use crate::client::FromMemcacheValue;
use crate::codec::header::{PacketHeader, ResponseStatus};
use crate::error::*;
use byteorder::{ReadBytesExt, BigEndian};
use std::io::Read;

pub mod header;

pub fn parse_get_response<R: Read, V: FromMemcacheValue>(reader: &mut R) -> Result<Option<V>> {
    let header = PacketHeader::read(reader)?;
    if header.vbucket_id_or_status == ResponseStatus::KeyNotFound as u16 {
        let mut buffer = vec![0; header.total_body_length as usize];
        reader.read_exact(buffer.as_mut_slice())?;
        return Ok(None);
    } else if header.vbucket_id_or_status != ResponseStatus::NoError as u16 {
        return Err(Error::from(ErrorKind::MemcacheError(header.vbucket_id_or_status)));
    }
    let flags = reader.read_u32::<BigEndian>()?;
    let value_length = header.total_body_length as usize - header.extras_length as usize ;
    return Ok(Some(V::from_memcache_value(reader, value_length, flags)?));
}

pub fn parse_header_only_response<R: Read>(reader: &mut R) -> Result<()> {
    let header = PacketHeader::read(reader)?;
    if header.vbucket_id_or_status != ResponseStatus::NoError as u16 {
        return Err(Error::from(ErrorKind::MemcacheError(header.vbucket_id_or_status)));
    }
    return Ok(());
}

