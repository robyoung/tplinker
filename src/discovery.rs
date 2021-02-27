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
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::Duration,
};

use crossbeam::thread::{self};
use if_addrs::{IfAddr, Interface};

use crate::error::Error;

use crate::{datatypes::DeviceData, error::Result, protocol};

// TODO: consider moving this to query builder
const QUERY: &str = r#"{
    "system": {"get_sysinfo": null},
    "emeter": {"get_realtime": null},
    "smartlife.iot.dimmer": {"get_dimmer_parameters": null},
    "smartlife.iot.common.emeter": {"get_realtime": null},
    "smartlife.iot.smartbulb.lightingservice": {"get_light_state": null}
    }"#;

fn can_interface_broadcast(iface: Interface) -> Option<(Ipv4Addr, Ipv4Addr)> {
    match iface.addr {
        IfAddr::V4(addr) => match (addr.ip.is_loopback(), addr.broadcast) {
            (false, Some(broadcast)) => Some((addr.ip, broadcast)),
            _ => None,
        },
        _ => None,
    }
}

fn discover_on_interface(
    timeout: Option<Duration>,
    ip: Ipv4Addr,
    broadcast: Ipv4Addr,
    request: &Vec<u8>,
) -> Result<HashMap<SocketAddr, DeviceData>> {
    let socket_addr = SocketAddr::new(IpAddr::V4(ip), 0);
    let udp_socket = UdpSocket::bind(socket_addr)?;
    udp_socket.set_broadcast(true)?;
    udp_socket.set_read_timeout(timeout)?;
    let dest_socket_addr = SocketAddr::new(IpAddr::V4(broadcast), 9999);
    for _ in 0..3 {
        let _ = udp_socket.send_to(&request[4..request.len()], dest_socket_addr);
    }

    let mut buf = [0_u8; 4096];
    let mut devices = HashMap::new();
    while let Ok((size, addr)) = udp_socket.recv_from(&mut buf) {
        let data = protocol::decrypt(&mut buf[0..size]);
        if let Ok(device_data) = serde_json::from_str::<DeviceData>(&data) {
            devices.insert(addr, device_data);
        }
    }
    Ok(devices)
}

/// Discover TPLink smart devices on the local network
///
/// # Errors
///
/// Will return `Err` if there is a `io::Error` communicating with the device or
/// a problem decoding the response.
pub fn with_timeout(timeout: Option<Duration>) -> Result<Vec<(SocketAddr, DeviceData)>> {
    let request = protocol::encrypt(QUERY).unwrap();
    let addrs = if_addrs::get_if_addrs()?;
    thread::scope(|s| {
        let handles = addrs
            .into_iter()
            .filter_map(can_interface_broadcast)
            .map(|(ip, broadcast)| {
                let request = &request;
                s.spawn(move |_| discover_on_interface(timeout, ip, broadcast, request))
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .filter_map(|join_handle| join_handle.join().ok().and_then(|r| r.ok()))
            .flat_map(|addresses| addresses)
            .collect::<Vec<_>>()
    })
    .map_err(|_e| Error::Other("cannot discover devices".to_string()))
}

/// Discover TPLink smart devices on the local network
///
/// Uses the default timeout of 3 seconds.
///
/// # Errors
///
/// Will return `Err` if [`with_timeout`](with_timeout) returns an `Err`.
pub fn discover() -> Result<Vec<(SocketAddr, DeviceData)>> {
    with_timeout(Some(Duration::from_secs(3)))
}
