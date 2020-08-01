//! Discover devices on the local network asynchronously
//!
use std::{
    collections::HashMap,
    net::SocketAddr,
    time::Duration,
};

use tokio::{
    net::UdpSocket,
    time::timeout as tokio_timeout,
};

use crate::{
    datatypes::DeviceData,
    discovery::QUERY,
    error::Result,
    protocol,
};

/// Discover TPLink smart devices on the local network
pub async fn with_timeout(timeout: Duration) -> Result<Vec<(SocketAddr, DeviceData)>> {
    let mut socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_broadcast(true)?;

    let req = protocol::encrypt(QUERY)?;

    for _ in 0_u8..3 {
        socket.send_to(&req[4..req.len()], "255.255.255.255:9999").await?;
    }

    let mut buf = [0_u8; 4096];

    let mut devices = HashMap::new();
    while let Ok(Ok((size, addr))) = tokio_timeout(timeout, socket.recv_from(&mut buf)).await {
        let data = protocol::decrypt(&mut buf[0..size]);
        if let Ok(device_data) = serde_json::from_str::<DeviceData>(&data) {
            devices.insert(addr, device_data);
        }
    }

    Ok(devices.into_iter().collect())
}

/// Discover TPLink smart devices on the local network
///
/// Uses the default timeout of 3 seconds.
pub async fn discover() -> Result<Vec<(SocketAddr, DeviceData)>> {
    with_timeout(Duration::from_secs(3)).await
}
