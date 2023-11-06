use std::{net::SocketAddr, time::Duration};

use clap::{App, AppSettings, Arg, SubCommand};
use serde_json::{json, to_string as stringify, Value};

use tplinker::{
    capabilities::{DeviceActions, MultiSwitch, Switch},
    datatypes::{DeviceData, SysInfo},
    devices::{Device, RawDevice, HS100, HS105, HS110, HS300, KL110, LB110, LB120},
    error::Result as TpResult,
};

fn command_discover(timeout: Option<Duration>, format: Format) -> Vec<Value> {
    tplinker::discovery::with_timeout(timeout)
        .unwrap()
        .into_iter()
        .map(|(addr, data)| {
            let device = Device::from_data(addr, &data);
            format.discover(addr, device, data)
        })
        .collect()
}

fn command_status(addresses: Vec<SocketAddr>, format: Format) -> Vec<Value> {
    use rayon::prelude::*;
    addresses
        .into_par_iter()
        .filter_map(|addr| {
            device_from_addr(addr)
                .map(|(addr, dev, info)| format.status(addr, dev, &info))
                .map_err(|err| eprintln!("While querying {}: {}", addr, err))
                .ok()
        })
        .collect()
}

fn command_reboot(addresses: Vec<SocketAddr>, delay: Duration, format: Format) -> Vec<Value> {
    use rayon::prelude::*;
    addresses
        .into_par_iter()
        .filter_map(|addr| {
            device_from_addr(addr)
                .map(|(addr, dev, info)| {
                    let result = dev
                        .reboot_with_delay(delay)
                        .map(|_| Value::Bool(true))
                        .unwrap_or_else(|err| Value::String(format!("Error: {}", err)));
                    format.actioned(addr, dev, info, "Rebooted?", result)
                })
                .map_err(|err| eprintln!("While querying {}: {}", addr, err))
                .ok()
        })
        .collect()
}

fn command_set_alias(addr: SocketAddr, alias: &str, format: Format) -> Vec<Value> {
    let dev = RawDevice::from_addr(addr);
    let done = dev
        .set_alias(alias)
        .map(|_| Value::Bool(true))
        .unwrap_or_else(|err| Value::String(format!("Error: {}", err)));

    device_from_addr(addr)
        .map(|(addr, dev, info)| {
            // In case it errors but has actually succeeded
            let done = if info.alias == alias {
                Value::Bool(true)
            } else {
                done
            };
            vec![format.actioned(addr, dev, info, "Renamed", done)]
        })
        .unwrap_or_else(|err| {
            eprintln!("While querying {}: {}", addr, err);
            Vec::new()
        })
}

fn command_switch_toggle(
    addr: SocketAddr,
    state: &str,
    index: Option<usize>,
    format: Format,
) -> Vec<Value> {
    let (expected, statename) = match state {
        "toggle" => (None, "Toggled"),
        "on" => (Some(true), "Switched on"),
        "off" => (Some(false), "Switched off"),
        _ => unreachable!(),
    };

    device_from_addr(addr)
        .and_then(|(addr, dev, _info)| {
            let actual = device_is_on(&dev, index).unwrap();
            let expected = match expected {
                None => !actual,
                Some(e) => e,
            };

            let done = if expected == actual {
                Value::Bool(false)
            } else {
                match &dev {
                    Device::HS100(s) => toggle_switch(s, state),
                    Device::HS105(s) => toggle_switch(s, state),
                    Device::HS110(s) => toggle_switch(s, state),
                    Device::HS300(s) if index.is_some() => {
                        toggle_multiswitch(s, state, index.unwrap())
                    }
                    Device::LB110(s) => toggle_switch(s, state),
                    Device::LB120(s) => toggle_switch(s, state),
                    Device::KL110(s) => toggle_switch(s, state),
                    _ => panic!("not a switchable device: {}", addr),
                }
                .map(|_| Value::Bool(true))
                .unwrap_or_else(|err| {
                    // In case it errors but has actually succeeded
                    let current = device_is_on(&dev, index).unwrap();
                    if expected == current {
                        Value::Bool(true)
                    } else {
                        Value::String(format!("Error: {}", err))
                    }
                })
            };

            device_from_addr(addr).map(|(addr, dev, info)| (addr, dev, info, done))
        })
        .map(|(addr, dev, info, done)| vec![format.actioned(addr, dev, info, statename, done)])
        .unwrap_or_else(|err| {
            eprintln!("While querying {}: {}", addr, err);
            Vec::new()
        })
}

fn device_from_addr(addr: SocketAddr) -> TpResult<(SocketAddr, Device, SysInfo)> {
    let raw = RawDevice::from_addr(addr);
    let info = raw.sysinfo()?;

    // Re-interpret as correct model
    let (dev, info) = if info.model.starts_with("HS100") {
        let dev = HS100::from_raw(raw);
        let info = dev.sysinfo()?;
        (Device::HS100(dev), info)
    } else if info.model.starts_with("HS105") {
        let dev = HS105::from_raw(raw);
        let info = dev.sysinfo()?;
        (Device::HS105(dev), info)
    } else if info.model.starts_with("HS110") {
        let dev = HS110::from_raw(raw);
        let info = dev.sysinfo()?;
        (Device::HS110(dev), info)
    } else if info.model.starts_with("HS300") {
        let dev = HS300::from_raw(raw);
        let info = dev.sysinfo()?;
        (Device::HS300(dev), info)
    } else if info.model.starts_with("LB110") {
        let dev = LB110::from_raw(raw);
        let info = dev.sysinfo()?;
        (Device::LB110(dev), info)
    } else if info.model.starts_with("LB120") {
        let dev = LB120::from_raw(raw);
        let info = dev.sysinfo()?;
        (Device::LB120(dev), info)
    } else if info.model.starts_with("KL110") {
        let dev = KL110::from_raw(raw);
        let info = dev.sysinfo()?;
        (Device::KL110(dev), info)
    } else {
        (Device::Unknown(raw), info)
    };

    Ok((addr, dev, info))
}

fn pad(value: &str, padding: usize) -> String {
    let pad = " ".repeat(padding.saturating_sub(value.len()));
    format!("{}{}", value, pad)
}

fn device_is_on(device: &Device, index: Option<usize>) -> Option<bool> {
    match device {
        Device::HS100(device) => device.is_on().ok(),
        Device::HS105(device) => device.is_on().ok(),
        Device::HS110(device) => device.is_on().ok(),
        Device::HS300(device) if index.is_some() => device.is_on(index.unwrap()).ok(),
        Device::LB110(device) => device.is_on().ok(),
        Device::LB120(device) => device.is_on().ok(),
        Device::KL110(device) => device.is_on().ok(),
        _ => None,
    }
}

fn toggle_switch<S: Switch>(switch: &S, state: &str) -> TpResult<bool> {
    match state {
        "on" => switch.switch_on().and(Ok(true)),
        "off" => switch.switch_off().and(Ok(false)),
        "toggle" => switch.toggle(),
        _ => unreachable!(),
    }
}

fn toggle_multiswitch<S: MultiSwitch>(switch: &S, state: &str, index: usize) -> TpResult<bool> {
    match state {
        "on" => switch.switch_on(index).and(Ok(true)),
        "off" => switch.switch_off(index).and(Ok(false)),
        "toggle" => switch.toggle(index),
        _ => unreachable!(),
    }
}

fn human_stringify(value: &Value) -> String {
    match value {
        Value::Null => "-".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.to_string(),
        Value::Array(v) => v
            .iter()
            .map(human_stringify)
            .collect::<Vec<String>>()
            .join(", "),
        Value::Object(o) => stringify(o).unwrap(),
    }
}

#[derive(Clone, Copy, Debug)]
enum Format {
    Short,
    Long,
    #[allow(clippy::upper_case_acronyms)]
    JSON,
}

impl Format {
    fn output(self, rows: Vec<Value>) {
        use std::collections::HashMap;

        println!(
            "{}",
            match self {
                Format::JSON => stringify(&rows).unwrap(),
                Format::Short | Format::Long => {
                    // field title -> (ordering, field width)
                    let mut fields: HashMap<String, (usize, usize)> = HashMap::new();
                    let mut processed: Vec<HashMap<String, String>> =
                        Vec::with_capacity(rows.len());

                    for row in rows {
                        let arr = row.as_array().expect("(bug) row is not an array");
                        let mut proc = HashMap::with_capacity(arr.len());

                        for field in arr {
                            if let [key, value] = &field
                                .as_array()
                                .expect("(bug) row is not an array of arrays")[..]
                            {
                                let order_next = fields.len();
                                let k = key.as_str().unwrap().to_string();
                                let h = human_stringify(&value);
                                let hlen = h.len().max(k.len());
                                proc.insert(k.clone(), h);
                                fields
                                    .entry(k)
                                    .and_modify(|(_, len)| {
                                        if *len < hlen {
                                            *len = hlen;
                                        }
                                    })
                                    .or_insert((order_next, hlen));
                            }
                        }

                        processed.push(proc);
                    }

                    let mut fields: Vec<(String, (usize, usize))> = fields.into_iter().collect();
                    fields.sort_unstable_by(|(_, (a, _)), (_, (b, _))| a.cmp(b));
                    let fields: Vec<(String, usize)> = fields
                        .into_iter()
                        .map(|(name, (_, width))| (name, width))
                        .collect();
                    let mut lines = Vec::with_capacity(processed.len() + 2);

                    lines.push(format!(
                        " {} ",
                        fields
                            .iter()
                            .map(|(name, width)| pad(name, *width))
                            .collect::<Vec<String>>()
                            .join(" | ")
                    ));

                    lines.push(format!(
                        "-{}-",
                        fields
                            .iter()
                            .map(|(_, width)| "-".repeat(*width))
                            .collect::<Vec<String>>()
                            .join("-+-")
                    ));

                    lines.extend(processed.into_iter().map(|row| {
                        format!(
                            " {} ",
                            fields
                                .iter()
                                .map(|(name, width)| pad(row.get(name).unwrap(), *width))
                                .collect::<Vec<String>>()
                                .join(" | ")
                        )
                    }));

                    lines.join("\n")
                }
            }
        )
    }

    fn discover(self, addr: SocketAddr, device: Device, data: DeviceData) -> Value {
        match self {
            Format::JSON => json!({
                "addr": addr,
                "device": Self::device(device),
                "data": data,
            }),
            rest => rest.status(addr, device, data.sysinfo()),
        }
    }

    fn status(self, addr: SocketAddr, device: Device, sysinfo: &SysInfo) -> Value {
        match self {
            Format::Short => json!([
                ["Address", addr],
                ["Alias", sysinfo.alias],
                ["Product", sysinfo.dev_name],
                ["Model", sysinfo.model],
                ["Signal", format!("{} dB", sysinfo.rssi)],
                ["On?", device_is_on(&device, None)],
            ]),
            Format::Long => {
                let (lat, lon) = device
                    .location()
                    .map(|(lat, lon)| (Some(lat), Some(lon)))
                    .unwrap_or((None, None));

                json!([
                    ["Address", addr],
                    ["MAC", sysinfo.mac],
                    ["Alias", sysinfo.alias],
                    ["Product", sysinfo.dev_name],
                    ["Type", sysinfo.hw_type],
                    ["Model", sysinfo.model],
                    ["Version", sysinfo.sw_ver],
                    ["Signal", format!("{} dB", sysinfo.rssi)],
                    ["Latitude", lat],
                    ["Longitude", lon],
                    ["Mode", sysinfo.active_mode],
                    ["On?", device_is_on(&device, None)],
                ])
            }
            Format::JSON => {
                let location = device.location().ok();

                json!({
                    "addr": addr,
                    "device": Self::device(device),
                    "data": {
                        "system": sysinfo,
                        "location": location,
                    },
                })
            }
        }
    }

    fn actioned(
        self,
        addr: SocketAddr,
        device: Device,
        sysinfo: SysInfo,
        action: &'static str,
        result: Value,
    ) -> Value {
        match self {
            Format::Short => json!([
                ["Address", addr],
                ["Alias", sysinfo.alias],
                ["Product", sysinfo.dev_name],
                ["Model", sysinfo.model],
                [action, result],
            ]),
            Format::Long => json!([
                ["Address", addr],
                ["MAC", sysinfo.mac],
                ["Alias", sysinfo.alias],
                ["Product", sysinfo.dev_name],
                ["Type", sysinfo.hw_type],
                ["Model", sysinfo.model],
                ["Version", sysinfo.sw_ver],
                [action, result],
            ]),
            Format::JSON => json!({
                "addr": addr,
                "actioned": {
                    "action": action,
                    "result": result,
                },
                "device": Self::device(device),
                "data": {
                    "system": sysinfo,
                },
            }),
        }
    }

    fn device(device: Device) -> &'static str {
        match device {
            Device::HS100(_) => "HS100",
            Device::HS105(_) => "HS105",
            Device::HS110(_) => "HS110",
            Device::HS300(_) => "HS300",
            Device::LB110(_) => "LB110",
            Device::LB120(_) => "LB120",
            Device::KL110(_) => "KL110",
            Device::KP115(_) => "KP115",
            Device::Unknown(_) => "unknown",
        }
    }
}

fn main() {
    let matches = App::new("TPLink smart device CLI")
        .version("0.1")
        .author("Rob Young <rob@robyoung.digital>")
        .about("Discover and interact with TPLink smart devices on the local network")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(
            Arg::with_name("json")
                .long("json")
                .takes_value(false)
                .help("Respond with JSON"),
        )
        .arg(
            Arg::with_name("long")
                .long("long")
                .takes_value(false)
                .help("Display more information"),
        )
        .subcommand(
            SubCommand::with_name("discover")
                .about("Discover devices on the local network")
                .arg(
                    Arg::with_name("timeout")
                        .long("timeout")
                        .takes_value(true)
                        .help("Timeout after (seconds)")
                        .default_value("3"),
                ),
        )
        .subcommand(
            SubCommand::with_name("status")
                .about("Given device addresses, return info + status")
                .arg(Arg::with_name("address").multiple(true).required(true)),
        )
        .subcommand(
            SubCommand::with_name("reboot")
                .about("Reboot one or more device")
                .arg(
                    Arg::with_name("delay")
                        .long("delay")
                        .takes_value(true)
                        .help("Schedule the reboot (in seconds)")
                        .default_value("1"),
                )
                .arg(Arg::with_name("address").multiple(true).required(true)),
        )
        .subcommand(
            SubCommand::with_name("set-alias")
                .about("Rename a device")
                .arg(Arg::with_name("address").required(true))
                .arg(Arg::with_name("alias").required(true)),
        )
        .subcommand(
            SubCommand::with_name("switch")
                .about("Toggle a switchable device")
                .arg(Arg::with_name("address").required(true))
                .arg(
                    Arg::with_name("state")
                        .possible_values(&["on", "off", "toggle"])
                        .default_value("toggle")
                        .required(true),
                )
                .arg(Arg::with_name("index").default_value("0").required(false)),
        )
        .get_matches();

    let format = if matches.is_present("json") {
        Format::JSON
    } else if matches.is_present("long") {
        Format::Long
    } else {
        Format::Short
    };

    fn parse_seconds(value: &str, default: u64) -> Duration {
        match value.parse::<u64>() {
            Ok(n) => Duration::from_secs(n),
            Err(_) => Duration::from_secs(default),
        }
    }

    fn parse_address(addr: &str) -> SocketAddr {
        addr.parse()
            .map_err(|_| ())
            .or_else(|_| -> Result<_, ()> {
                Ok(SocketAddr::new(addr.parse().map_err(|_| ())?, 9999))
            })
            .unwrap_or_else(|_| panic!("not a valid address: {}", addr))
    }

    fn parse_addresses(matches: &clap::ArgMatches) -> Vec<SocketAddr> {
        matches
            .values_of("address")
            .unwrap()
            .map(parse_address)
            .collect()
    }

    format.output(match matches.subcommand() {
        ("discover", Some(matches)) => {
            let timeout = match matches.value_of("timeout").unwrap() {
                "never" => None,
                value => Some(parse_seconds(value, 3)),
            };

            command_discover(timeout, format)
        }
        ("status", Some(matches)) => command_status(parse_addresses(&matches), format),
        ("reboot", Some(matches)) => command_reboot(
            parse_addresses(&matches),
            parse_seconds(matches.value_of("delay").unwrap(), 1),
            format,
        ),
        ("set-alias", Some(matches)) => {
            let address = parse_address(matches.value_of("address").unwrap());
            let alias = matches.value_of("alias").unwrap();
            command_set_alias(address, alias, format)
        }
        ("switch", Some(matches)) => {
            let address = parse_address(matches.value_of("address").unwrap());
            let state = matches.value_of("state").unwrap();
            let index = matches
                .value_of("index")
                .and_then(|index| index.parse::<usize>().ok());
            command_switch_toggle(address, state, index, format)
        }
        _ => unreachable!(),
    })
}
