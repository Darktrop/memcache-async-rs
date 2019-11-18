use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use crate::error::*;
use std::collections::HashMap;
use std::io;

#[allow(dead_code)]
pub enum Opcode {
    Get = 0x00,
    Set = 0x01,
    Add = 0x02,
    Replace = 0x03,
    Delete = 0x04,
    Increment = 0x05,
    Decrement = 0x06,
    Flush = 0x08,
    Stat = 0x10,
    Noop = 0x0a,
    Version = 0x0b,
    GetKQ = 0x0d,
    Append = 0x0e,
    Prepend = 0x0f,
    Touch = 0x1c,
    StartAuth = 0x21,
}

pub enum Magic {
    Request = 0x80,
    Response = 0x81,
}

#[allow(dead_code)]
pub enum ResponseStatus {
    NoError = 0x00,
    KeyNotFound = 0x01,
    KeyExits = 0x02,
    ValueTooLarge = 0x03,
    InvalidArguments = 0x04,
    AuthenticationRequired = 0x20,
}

#[derive(Debug, Default)]
pub struct PacketHeader {
    pub magic: u8,
    pub opcode: u8,
    pub key_length: u16,
    pub extras_length: u8,
    pub data_type: u8,
    pub vbucket_id_or_status: u16,
    pub total_body_length: u32,
    pub opaque: u32,
    pub cas: u64,
}

#[derive(Debug)]
pub struct StoreExtras {
    pub flags: u32,
    pub expiration: u32,
}

#[derive(Debug)]
pub struct CounterExtras {
    pub amount: u64,
    pub initial_value: u64,
    pub expiration: u32,
}

impl PacketHeader {
    pub fn write<W: io::Write>(self, writer: &mut W) -> Result<()> {
        writer.write_u8(self.magic)?;
        writer.write_u8(self.opcode)?;
        writer.write_u16::<BigEndian>(self.key_length)?;
        writer.write_u8(self.extras_length)?;
        writer.write_u8(self.data_type)?;
        writer.write_u16::<BigEndian>(self.vbucket_id_or_status)?;
        writer.write_u32::<BigEndian>(self.total_body_length)?;
        writer.write_u32::<BigEndian>(self.opaque)?;
        writer.write_u64::<BigEndian>(self.cas)?;
        return Ok(());
    }

    pub fn read<R: io::Read>(reader: &mut R) -> Result<PacketHeader> {
        let magic = reader.read_u8()?;
        if magic != Magic::Response as u8 {
            bail!(ErrorKind::ClientError(format!("Bad magic number in response header: {}", magic)));
        }
        let header = PacketHeader {
            magic,
            opcode: reader.read_u8()?,
            key_length: reader.read_u16::<BigEndian>()?,
            extras_length: reader.read_u8()?,
            data_type: reader.read_u8()?,
            vbucket_id_or_status: reader.read_u16::<BigEndian>()?,
            total_body_length: reader.read_u32::<BigEndian>()?,
            opaque: reader.read_u32::<BigEndian>()?,
            cas: reader.read_u64::<BigEndian>()?,
        };
        return Ok(header);
    }
}

#[cfg(test)]
mod tests {
    use crate::codec::header::*;
    use std::io::{Bytes, BufWriter, BufReader, Write};
    use byteorder::{BigEndian, ByteOrder, WriteBytesExt};

    #[test]
    fn test_packet_header_read_write() {
        let header = PacketHeader {
            opcode: Opcode::Get as u8,
            magic: Magic::Request as u8,
            ..Default::default()
        };
        let mut v: Vec<u8> = Vec::with_capacity(32);
        header.write(&mut v);
        let mut slice: &[u8] = v.as_mut_slice();
        let res = PacketHeader::read(&mut slice);
        assert_eq!(true, res.is_ok());
        assert_eq!(Opcode::Get as u8, res.unwrap().opcode);
    }
}