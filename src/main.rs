extern crate tplinker;

use std::net::SocketAddr;
use tplinker::{
    devices::{Device, HS100},
    discovery::discover,
};

fn pad(value: &str, padding: usize) -> String {
    let pad = " ".repeat(padding.saturating_sub(value.len()));
    format!("{}{}", value, pad)
}

fn main() {
    for (addr, device) in discover().unwrap() {
        let sysinfo = device.sysinfo();
        println!(
            "{}\t{}\t{}\t{}\t{}",
            addr,
            pad(&sysinfo.alias, 18),
            pad(&sysinfo.hw_type, 20),
            pad(&sysinfo.dev_name, 40),
            sysinfo.model,
        );
    }

    let host: SocketAddr = "192.168.0.10:9999".parse().unwrap();
    let device = HS100::from_addr(host);

    println!("{:?}", device.sysinfo().unwrap());
}
