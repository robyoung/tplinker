use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
    time::Duration,
};

use crate::{datatypes, error::Result, protocol};

// TODO: consider moving this to query builder
const QUERY: &'static str = r#"{
    "system": {"get_sysinfo": null},
    "emeter": {"get_realtime": null},
    "smartlife.iot.dimmer": {"get_dimmer_parameters": null},
    "smartlife.iot.common.emeter": {"get_realtime": null},
    "smartlife.iot.smartbulb.lightingservice": {"get_light_state": null}
}"#;

pub fn discover() -> Result<HashMap<SocketAddr, datatypes::DeviceData>> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(Duration::from_secs(3)))?;

    let req = protocol::encrypt(QUERY)?;

    for _ in 0..3 {
        socket.send_to(&req[4..req.len()], "255.255.255.255:9999")?;
    }

    let mut buf = [0u8; 4096];

    let mut devices = HashMap::new();
    while let Ok((size, addr)) = socket.recv_from(&mut buf) {
        let data = protocol::decrypt(&mut buf[0..size]);
        if let Ok(device) = serde_json::from_str::<datatypes::DeviceData>(&data) {
            devices.insert(addr, device);
        }
    }

    Ok(devices)
}
