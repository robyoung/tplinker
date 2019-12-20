use std::{
    net::SocketAddr,
    rc::Rc,
    str::FromStr,
    time::Duration,
};

use clap::{App, AppSettings, Arg, SubCommand};
use serde_json::{json, to_string as stringify, Value};

use tplinker::{
    capabilities::{DeviceActions, Emeter, RealtimeEnergy, Switch},
    datatypes::{DeviceData, SysInfo},
    devices::{Device, RawDevice, HS100, HS110, LB110},
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
                .map(|(addr, dev, info)| format.status(addr, dev, info))
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

fn command_switch_toggle(addr: SocketAddr, state: &str, format: Format) -> Vec<Value> {
    let (expected, statename) = match state {
        "toggle" => (None, "Toggled"),
        "on" => (Some(true), "Switched on"),
        "off" => (Some(false), "Switched off"),
        _ => unreachable!(),
    };

    device_from_addr(addr)
        .and_then(|(addr, dev, _info)| {
            let actual = device_is_on(&dev).unwrap();
            let expected = match expected {
                None => !actual,
                Some(e) => e,
            };

            let done = if expected == actual {
                Value::Bool(false)
            } else {
                match &dev {
                    Device::HS100(s) => toggle_switch(s, state),
                    Device::HS110(s) => toggle_switch(s, state),
                    Device::LB110(s) => toggle_switch(s, state),
                    _ => panic!("not a switchable device: {}", addr),
                }
                .map(|_| Value::Bool(true))
                .unwrap_or_else(|err| {
                    // In case it errors but has actually succeeded
                    let current = device_is_on(&dev).unwrap();
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

fn command_energy(addresses: Vec<SocketAddr>, times: Vec<TimeRequest>, format: Format) -> Vec<Value> {
    #[derive(Debug)]
    struct Tables {
        pub errors: Vec<(Rc<SocketAddr>, Option<TimeRequest>, String)>,
        pub realtime: Vec<(Rc<SocketAddr>, Rc<SysInfo>, RealtimeEnergy)>,
        //                                           y    m   d
        pub daily: Vec<(Rc<SocketAddr>, Rc<SysInfo>, u16, u8, u8, usize)>,
        //                                             y    m
        pub monthly: Vec<(Rc<SocketAddr>, Rc<SysInfo>, u16, u8, usize)>,
    }

    let mut tables = Tables {
        errors: Vec::new(),
        realtime: Vec::new(),
        daily: Vec::new(),
        monthly: Vec::new(),
    };

    fn harvest_energy_info<D: Emeter>(tables: &mut Tables, addr: Rc<SocketAddr>, device: D, sys: Rc<SysInfo>, times: &[TimeRequest]) {
        for time in times {
            match time {
                TimeRequest::Realtime => {
                    match device.get_emeter_realtime() {
                        Err(err) => {
                            tables.errors.push((addr.clone(), Some(time.clone()), err.to_string()));
                        },
                        Ok(energy) => {
                            tables.realtime.push((addr.clone(), sys.clone(), energy));
                        }
                    }

                }
                TimeRequest::Daily { year, month } => {
                    match device.get_emeter_daily(*year, *month) {
                        Err(err) => {
                            tables.errors.push((addr.clone(), Some(time.clone()), err.to_string()));
                        },
                        Ok(days) => {
                            for d in days {
                                tables.daily.push((addr.clone(), sys.clone(), d.year, d.month, d.day, d.energy));
                            }
                        }
                    }
                }
                TimeRequest::Monthly { year } => {
                    match device.get_emeter_monthly(*year) {
                        Err(err) => {
                            tables.errors.push((addr.clone(), Some(time.clone()), err.to_string()));
                        },
                        Ok(months) => {
                            for m in months {
                                tables.monthly.push((addr.clone(), sys.clone(), m.year, m.month, m.energy));
                            }
                        }
                    }
                }
            }
        }
    }

    for addr in addresses {
        if let Ok((addr, dev, sys)) = device_from_addr(addr).map_err(|err| err.to_string()) {
            let addr = Rc::new(addr);
            let sys = Rc::new(sys);
            match dev {
                Device::HS110(d) => {
                    harvest_energy_info(&mut tables, addr, d, sys, &times);
                },
                _ => {
                    tables.errors.push((addr, None, "Not an energy-monitoring device".into()));
                }
            }
        }
    }

    for (addr, time, err) in tables.errors {
        eprintln!("Error retrieving {:?} energy for {}: {}", time, addr, err);
    }

    if format != Format::JSON && !tables.realtime.is_empty() { println!("\n== Current energy use:"); }
    format.output(tables.realtime.into_iter().map(|(addr, sys, energy)|
        format.energy_realtime(addr, sys, energy)
    ).collect());

    if format != Format::JSON && !tables.monthly.is_empty() { println!("\n== Monthly energy use:"); }
    format.output(tables.monthly.into_iter().map(|(addr, sys, year, month, energy)|
        format.energy_monthly(addr, sys, year, month, energy)
    ).collect());

    if format != Format::JSON && !tables.daily.is_empty() { println!("\n== Daily energy use:"); }
    format.output(tables.daily.into_iter().map(|(addr, sys, year, month, day, energy)|
        format.energy_daily(addr, sys, year, month, day, energy)
    ).collect());

    Vec::new()
}

fn device_from_addr(addr: SocketAddr) -> TpResult<(SocketAddr, Device, SysInfo)> {
    let raw = RawDevice::from_addr(addr);
    let info = raw.sysinfo()?;

    // Re-interpret as correct model
    let (dev, info) = if info.model.starts_with("HS100") {
        let dev = HS100::from_raw(raw);
        let info = dev.sysinfo()?;
        (Device::HS100(dev), info)
    } else if info.model.starts_with("HS110") {
        let dev = HS110::from_raw(raw);
        let info = dev.sysinfo()?;
        (Device::HS110(dev), info)
    } else if info.model.starts_with("LB110") {
        let dev = LB110::from_raw(raw);
        let info = dev.sysinfo()?;
        (Device::LB110(dev), info)
    } else {
        (Device::Unknown(raw), info)
    };

    Ok((addr, dev, info))
}

fn lpad(value: &str, padding: usize) -> String {
    let pad = " ".repeat(padding.saturating_sub(value.len()));
    format!("{}{}", value, pad)
}

fn rpad(value: &str, padding: usize) -> String {
    let pad = " ".repeat(padding.saturating_sub(value.len()));
    format!("{}{}", pad, value)
}

fn device_is_on(device: &Device) -> Option<bool> {
    match device {
        Device::HS100(device) => device.is_on().ok(),
        Device::HS110(device) => device.is_on().ok(),
        Device::LB110(device) => device.is_on().ok(),
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

fn human_stringify(value: &Value) -> (String, bool) {
    match value {
        Value::Null => ("-".to_string(), false),
        Value::Bool(b) => (b.to_string(), false),
        Value::Number(n) => (n.to_string(), true),
        Value::String(s) => (s.to_string(), false),
        Value::Array(v) => (v
            .iter()
            .map(|v| human_stringify(v).0)
            .collect::<Vec<String>>()
            .join(", "), false),
        Value::Object(o) => (stringify(o).unwrap(), false),
    }
}

#[derive(Clone, Debug)]
enum TimeRequest {
    Realtime,
    Daily { year: u16, month: u8 },
    Monthly { year: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Format {
    Short,
    Long,
    JSON,
}

impl Format {
    fn output(self, rows: Vec<Value>) {
        use std::collections::HashMap;

        if rows.is_empty() { return; }

        println!(
            "{}",
            match self {
                Format::JSON => stringify(&rows).unwrap(),
                Format::Short | Format::Long => {
                    // field title -> (ordering, field width)
                    let mut fields: HashMap<String, (usize, usize)> = HashMap::new();
                    let mut processed: Vec<HashMap<String, (String, bool)>> =
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
                                let (h, align_right) = human_stringify(&value);
                                let hlen = h.len().max(k.len());
                                proc.insert(k.clone(), (h, align_right));
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
                            .map(|(name, width)| lpad(name, *width))
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
                                .map(|(name, width)| {
                                    let (val, align_right) = row.get(name).unwrap();
                                    if *align_right { rpad(val, *width) }
                                    else { lpad(val, *width) }
                                })
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
                "device": Self::device(&device),
                "data": data,
            }),
            rest => rest.status(addr, device, data.sysinfo()),
        }
    }

    fn status(self, addr: SocketAddr, device: Device, sysinfo: SysInfo) -> Value {
        match self {
            Format::Short => json!([
                ["Address", addr],
                ["Alias", sysinfo.alias],
                ["Product", sysinfo.dev_name],
                ["Model", sysinfo.model],
                ["Signal", format!("{} dB", sysinfo.rssi)],
                ["On?", device_is_on(&device)],
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
                    ["On?", device_is_on(&device)],
                ])
            }
            Format::JSON => {
                let location = device.location().ok();

                json!({
                    "addr": addr,
                    "device": Self::device(&device),
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
                "device": Self::device(&device),
                "data": {
                    "system": sysinfo,
                },
            }),
        }
    }

    fn energy_realtime(self, addr: Rc<SocketAddr>, sysinfo: Rc<SysInfo>, energy: RealtimeEnergy) -> Value {
        match self {
            Format::Short => json!([
                ["Address", addr.to_string()],
                ["Alias", sysinfo.alias],
                ["Power (mW)", energy.power],
            ]),
            Format::Long => json!([
                ["Address", addr.to_string()],
                ["Alias", sysinfo.alias],
                ["Current (mA)", energy.current],
                ["Voltage (mV)", energy.voltage],
                ["Power (mW)", energy.power],
                ["This week (Wh)", energy.total_energy],
            ]),
            Format::JSON => {
                json!({
                    "addr": addr.to_string(),
                    "alias": sysinfo.alias,
                    "energy": {
                        "date": "current",
                        "current_ma": energy.current,
                        "voltage_mv": energy.voltage,
                        "power_mw": energy.power,
                        "week_so_far_power_wh": energy.total_energy,
                    },
                })
            }
        }
    }

    fn energy_daily(self, addr: Rc<SocketAddr>, sysinfo: Rc<SysInfo>, year: u16, month: u8, day: u8, energy: usize) -> Value {
        let date = format!("{:04}-{:02}-{:02}", year, month, day);
        match self {
            Format::Short | Format::Long => json!([
                ["Address", addr.to_string()],
                ["Alias", sysinfo.alias],
                ["Date", date],
                ["Energy (Wh)", energy],
            ]),
            Format::JSON => {
                json!({
                    "addr": addr.to_string(),
                    "alias": sysinfo.alias,
                    "energy": {
                        "date": date,
                        "wh": energy,
                    },
                })
            }
        }
    }

    fn energy_monthly(self, addr: Rc<SocketAddr>, sysinfo: Rc<SysInfo>, year: u16, month: u8, energy: usize) -> Value {
        let date = format!("{:04}-{:02}", year, month);
        match self {
            Format::Short | Format::Long => json!([
                ["Address", addr.to_string()],
                ["Alias", sysinfo.alias],
                ["Month", date],
                ["Energy (Wh)", energy],
            ]),
            Format::JSON => {
                json!({
                    "addr": addr.to_string(),
                    "alias": sysinfo.alias,
                    "energy": {
                        "month": date,
                        "wh": energy,
                    },
                })
            }
        }
    }


    fn device(device: &Device) -> &'static str {
        match device {
            Device::HS100(_) => "HS100",
            Device::HS110(_) => "HS110",
            Device::LB110(_) => "LB110",
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
                .arg(
                    Arg::with_name("address")
                        .multiple(true)
                        .required(true)
                        .validator(|val| check_address(&val).and(Ok(())))
                ),
        )
        .subcommand(
            SubCommand::with_name("reboot")
                .about("Reboot one or more device")
                .arg(
                    Arg::with_name("delay")
                        .long("delay")
                        .takes_value(true)
                        .help("Schedule the reboot (in seconds)")
                        .default_value("1")
                        .validator(|val| u64::from_str(&val).and(Ok(())).map_err(|err| err.to_string())),
                )
                .arg(
                    Arg::with_name("address")
                        .multiple(true)
                        .required(true)
                        .validator(|val| check_address(&val).and(Ok(())))
                ),
        )
        .subcommand(
            SubCommand::with_name("set-alias")
                .about("Rename a device")
                .arg(
                    Arg::with_name("address")
                        .required(true)
                        .validator(|val| check_address(&val).and(Ok(())))
                )
                .arg(Arg::with_name("alias").required(true)),
        )
        .subcommand(
            SubCommand::with_name("switch")
                .about("Toggle a switchable device")
                .arg(
                    Arg::with_name("address")
                        .required(true)
                        .validator(|val| check_address(&val).and(Ok(())))
                )
                .arg(
                    Arg::with_name("state")
                        .possible_values(&["on", "off", "toggle"])
                        .default_value("toggle")
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("energy")
                .about("Query energy-monitoring devices")
                .arg(Arg::with_name("now")
                    .long("now")
                    .takes_value(false)
                    .help("Retrieve the current realtime energy use (default if no other option provided)")
                )
                .arg(Arg::with_name("daily")
                    .long("daily")
                    .takes_value(true)
                    .multiple(true)
                    .help("Retrieve the daily energy use for the given month (YYYY-MM)")
                    .validator(|val| parse_year_month(&val).and(Ok(())))
                )
                .arg(Arg::with_name("monthly")
                    .long("monthly")
                    .takes_value(true)
                    .multiple(true)
                    .help("Retrieve the monthly energy use for the given year (YYYY)")
                    .validator(|val| parse_year(&val).and(Ok(())))
                )
                .arg(
                    Arg::with_name("address")
                        .multiple(true)
                        .required(true)
                        .validator(|val| check_address(&val).and(Ok(())))
                ),
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

    fn check_address(addr: &str) -> Result<SocketAddr, String> {
        addr.parse()
            .map_err(|_| ())
            .or_else(|_| -> Result<_, ()> {
                Ok(SocketAddr::new(addr.parse().map_err(|_| ())?, 9999))
            })
            .map_err(|_| "not an IP address or IP:port pair".into())
    }

    fn parse_address(addr: &str) -> SocketAddr {
        // okay to unwrap as all will have been checked by clap
        check_address(addr).unwrap()
    }

    fn parse_addresses(matches: &clap::ArgMatches) -> Vec<SocketAddr> {
        matches
            .values_of("address")
            .unwrap()
            .into_iter()
            .map(parse_address)
            .collect()
    }

    fn parse_year_month(val: &str) -> Result<(u16, u8), String> {
        let ym = val.split("-").collect::<Vec<&str>>();
        if ym.len() != 2 { return Err("cannot split in two by '-'".into()); }
        let year = u16::from_str(ym[0]).map_err(|err| format!("year: {}", err))?;
        let month = u8::from_str(ym[1]).map_err(|err| format!("month: {}", err))?;
        if year < 2000 || year > 2100 { return Err("year out of bounds".into()); }
        if month < 1 || month > 12 { return Err("month out of bounds".into()); }
        Ok((year, month))
    }

    fn parse_year(val: &str) -> Result<u16, String> {
        let year = u16::from_str(&val).map_err(|err| err.to_string())?;
        if year < 2000 || year > 2100 { return Err("out of bounds".into()); }
        Ok(year)
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
            command_switch_toggle(address, state, format)
        }
        ("energy", Some(matches)) => {
            let addresses = parse_addresses(&matches);

            let mut timerequests = Vec::new();

            if let Some(dailies) = matches.values_of("daily") {
                for daily in dailies {
                    let (year, month) = parse_year_month(daily).unwrap();
                    timerequests.push(TimeRequest::Daily { year, month });
                }
            }

            if let Some(monthlies) = matches.values_of("monthly") {
                for monthly in monthlies {
                    let year = parse_year(monthly).unwrap();
                    timerequests.push(TimeRequest::Monthly { year });
                }
            }

            if matches.is_present("now") || timerequests.is_empty() {
                timerequests.push(TimeRequest::Realtime);
            }

            command_energy(addresses, timerequests, format)
        }
        _ => unreachable!(),
    })
}
