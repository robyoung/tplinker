extern crate tplinker;

use std::net::SocketAddr;

use clap::{App, Arg, SubCommand};

use tplinker::{
    capabilities::Switch,
    datatypes::DeviceData,
    devices::Device,
};

fn command_discover(json: bool) {
    for (addr, data) in tplinker::discover().unwrap() {
        let device = Device::from_data(addr, &data);

        if json {
            discover_print_json(addr, data, device);
        } else {
            discover_print_human(addr, data, device);
        }
    }
}

fn discover_print_human(addr: SocketAddr, data: DeviceData, device: Device) {
    let sysinfo = data.sysinfo();
    println!(
        "{}\t{}\t{}\t{}\t{}",
        addr,
        pad(&sysinfo.alias, 18),
        pad(&sysinfo.hw_type, 20),
        pad(&sysinfo.dev_name, 40),
        sysinfo.model,
    );
    match device {
        Device::HS100(device) => is_on(&device),
        Device::HS110(device) => is_on(&device),
        Device::LB110(device) => is_on(&device),
        _ => println!("{} not switchable", sysinfo.alias),
    }
}

fn pad(value: &str, padding: usize) -> String {
    let pad = " ".repeat(padding.saturating_sub(value.len()));
    format!("{}{}", value, pad)
}

fn is_on<T: Switch>(device: &T) {
    println!("{:?}", device.is_on());
}

fn discover_print_json(_addr: SocketAddr, data: DeviceData, _device: Device) {
    println!("{}", serde_json::to_string(&data).unwrap());
}

fn main() {
    let matches = App::new("TPLink smart device CLI")
        .version("0.1")
        .author("Rob Young <rob@robyoung.digital>")
        .about("Discover and interact with TPLink smart devices on the local network.")
        .arg(Arg::with_name("json")
            .long("json")
            .takes_value(false)
            .help("Respond with JSON.")
        )
        .subcommand(SubCommand::with_name("discover")
            .about("Discover devices on the local network")
        )
        .get_matches();

    if matches.subcommand_matches("discover").is_some() {
        command_discover(matches.is_present("json"));
    }
}
