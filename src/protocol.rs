use std::{
    convert::TryInto,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

use byteorder::{BigEndian, ByteOrder, WriteBytesExt};

use crate::error::Error;

#[cfg(test)]
use std::cell::Cell;

// Prepare and encrypt message to send to the device
// see: https://www.softscheck.com/en/reverse-engineering-tp-link-hs110/
pub fn encrypt(plain: &str) -> Result<Vec<u8>, Error> {
    let len = plain.len();
    let msgbytes = plain.as_bytes();
    let mut cipher = vec![];
    cipher.write_u32::<BigEndian>(len as u32)?;

    let mut key = 0xAB;
    let mut payload: Vec<u8> = Vec::with_capacity(len);

    for i in 0..len {
        payload.push(msgbytes[i] ^ key);
        key = payload[i];
    }

    for i in &payload {
        cipher.write_u8(*i).unwrap();
    }

    Ok(cipher)
}

// Decrypt received string
// see: https://www.softscheck.com/en/reverse-engineering-tp-link-hs110/
pub fn decrypt(cipher: &mut [u8]) -> String {
    let len = cipher.len();

    let mut key = 0xAB;
    let mut next: u8;

    for item in cipher.iter_mut().take(len) {
        next = *item;
        *item ^= key;
        key = next;
    }

    String::from_utf8_lossy(cipher).into_owned()
}

pub trait Protocol {
    fn send(&self, ip: SocketAddr, msg: &str) -> Result<String, Error>;
}

pub struct DefaultProtocol;

impl DefaultProtocol {
    pub fn new() -> DefaultProtocol {
        DefaultProtocol {}
    }
}

impl Protocol for DefaultProtocol {
    fn send(&self, ip: SocketAddr, msg: &str) -> Result<String, Error> {
        let payload = encrypt(msg)?;
        let mut stream = TcpStream::connect(ip)?;

        stream.set_read_timeout(Some(Duration::new(5, 0)))?;
        stream.write_all(&payload)?;

        let mut resp = vec![];
        let mut buffer: [u8; 4096] = [0; 4096];
        let mut length: Option<u32> = None;

        loop {
            if let Ok(read) = stream.read(&mut buffer) {
                if length.is_none() {
                    length = Some(BigEndian::read_u32(&buffer[0..4]));
                }
                resp.extend_from_slice(&buffer[0..read]);
                let lval: u32 = length.unwrap();
                if lval > 0 && resp.len() >= (lval + 4).try_into().unwrap() || read == 0 {
                    break;
                }
            }
        }

        let decrypted = decrypt(&mut resp.split_off(4));

        Ok(decrypted)
    }
}

#[cfg(test)]
pub struct ProtocolMock {
    req: Cell<Option<(String, String)>>,
    resp: Cell<Result<String, Error>>,
}

#[cfg(test)]
impl ProtocolMock {
    pub fn new() -> ProtocolMock {
        ProtocolMock {
            req: Cell::new(None),
            resp: Cell::new(Ok(String::from(""))),
        }
    }

    pub fn set_send_return_value(&self, resp: Result<String, Error>) {
        self.resp.set(resp);
    }
}

#[cfg(test)]
impl Protocol for ProtocolMock {
    fn send(&self, ip: SocketAddr, msg: &str) -> Result<String, Error> {
        self.req.set(Some((ip.to_string(), msg.to_string())));
        self.resp.replace(Ok(String::from("")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt() {
        let json = "{\"system\":{\"get_sysinfo\":{}}}";

        let data = encrypt(json);
        let resp = decrypt(&mut data.unwrap().split_off(4));

        assert_eq!(json, resp);
    }
}
