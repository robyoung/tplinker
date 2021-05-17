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
    #[allow(clippy::cast_possible_truncation)]
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

pub trait Protocol: Send {
    fn send(&self, ip: SocketAddr, msg: &str) -> Result<String, Error>;
}

#[derive(Default, Clone, Debug)]
pub struct DefaultProtocol;

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
        if resp.len() < 4 {
            Err(Error::from("response not big enough to decrypt"))
        } else {
            let result = decrypt(&mut resp.split_off(4));
            Ok(result)
        }
    }
}

#[cfg(test)]
pub(crate) mod mock {
    use super::*;

    pub(crate) struct ProtocolMock {
        req: Cell<Option<(String, String)>>,
        resp: Cell<Result<String, Error>>,
    }

    impl Default for ProtocolMock {
        fn default() -> Self {
            ProtocolMock {
                req: Cell::new(None),
                resp: Cell::new(Ok(String::from(""))),
            }
        }
    }

    impl ProtocolMock {
        pub fn set_send_return_value(&self, resp: Result<String, Error>) {
            self.resp.set(resp);
        }
    }

    impl Protocol for ProtocolMock {
        fn send(&self, ip: SocketAddr, msg: &str) -> Result<String, Error> {
            self.req.set(Some((ip.to_string(), msg.to_string())));
            self.resp.replace(Ok(String::from("")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpListener, sync::mpsc::channel, thread};

    #[test]
    fn encrypt_decrypt() {
        let json = "{\"system\":{\"get_sysinfo\":{}}}";

        let data = encrypt(json);
        let resp = decrypt(&mut data.unwrap().split_off(4));

        assert_eq!(json, resp);
    }

    #[test]
    fn protocol_send() {
        // arrange
        let protocol = DefaultProtocol::default();
        let msg = "{\"system\":{\"get_sysinfo\":{}}}";
        let resp = "great response";

        let (sender, ready) = channel();
        thread::spawn(move || {
            let listener: TcpListener;
            // Bind to lowest available port
            let mut port = 5818;
            loop {
                match TcpListener::bind(format!("127.0.0.1:{}", port)) {
                    Ok(ok) => {
                        listener = ok;
                        break;
                    }
                    Err(_) => {
                        port += 1;
                    }
                }
            }

            sender.send(port).unwrap();
            match listener.accept() {
                Ok((mut socket, _)) => {
                    socket.write(&encrypt(resp).unwrap()).unwrap();
                }
                _ => {}
            }
        });
        let port = ready.recv().unwrap();
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

        // act
        let result = protocol.send(addr, msg).unwrap();

        // assert
        assert_eq!(result, resp.to_string());
    }
}
