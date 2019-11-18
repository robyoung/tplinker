use byteorder::{BigEndian, WriteBytesExt};

use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

use crate::error::Error;

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

pub fn send(ip: &str, msg: &str) -> Result<String, Error> {
    let payload = encrypt(msg)?;
    let mut stream = TcpStream::connect(ip)?;

    stream.set_read_timeout(Some(Duration::new(5, 0)))?;
    stream.write_all(&payload)?;

    let mut resp = vec![];
    stream.read_to_end(&mut resp)?;

    Ok(decrypt(&mut resp.split_off(4)))
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
