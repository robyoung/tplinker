//! Discover devices on the local network
//!
//! ```no_run
//! use tplinker::{
//!   discovery::discover,
//!   devices::Device,
//!   capabilities::Switch,
//! };
//!
//! for (addr, data) in discover().unwrap() {
//!   let device = Device::from_data(addr, &data);
//!   let sysinfo = data.sysinfo();
//!   println!("{}\t{}\t{}", addr, sysinfo.alias, sysinfo.hw_type);
//!   match device {
//!     Device::HS110(device) => { device.switch_on().unwrap(); },
//!     _ => {},
//!   }
//! }
//! ```
use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
    time::Duration,
};

use crate::{datatypes::DeviceData, error::Result, protocol};

// TODO: consider moving this to query builder
const QUERY: &str = r#"{
    "system": {"get_sysinfo": null},
    "emeter": {"get_realtime": null},
    "smartlife.iot.dimmer": {"get_dimmer_parameters": null},
    "smartlife.iot.common.emeter": {"get_realtime": null},
    "smartlife.iot.smartbulb.lightingservice": {"get_light_state": null}
}"#;

/// Discover TPLink smart devices on the local network
pub fn with_timeout(timeout: Option<Duration>) -> Result<Vec<(SocketAddr, DeviceData)>> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(timeout)?;

    let req = protocol::encrypt(QUERY)?;

    for _ in 0..3 {
        socket.send_to(&req[4..req.len()], "255.255.255.255:9999")?;
    }

    let mut buf = [0_u8; 4096];

    let mut devices = HashMap::new();
    while let Ok((size, addr)) = socket.recv_from(&mut buf) {
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
pub fn discover() -> Result<Vec<(SocketAddr, DeviceData)>> {
    with_timeout(Some(Duration::from_secs(3)))
}
