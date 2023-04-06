# TPLinker


[![GitHub Actions][gh-image]][gh-checks]
[![crates.io][cratesio-image]][cratesio]
[![docs.rs][docsrs-image]][docsrs]

[gh-image]: https://github.com/robyoung/tplinker/workflows/Test/badge.svg
[gh-checks]: https://github.com/robyoung/tplinker/actions?query=workflow%3ATest
[cratesio-image]: https://img.shields.io/crates/v/tplinker.svg
[cratesio]: https://crates.io/crates/tplinker
[docsrs-image]: https://docs.rs/tplinker/badge.svg
[docsrs]: https://docs.rs/tplinker

A rust library to query and control TPLink smart plugs and smart lights.

Supported devices include HS100, HS110, LB110, LB120, KL110.

Inspired and influenced by [pyHS100](https://github.com/GadgetReactor/pyHS100) and
[hs100api](https://github.com/abronan/hs100-rust-api).

There are two main entrypoints. If you know the IP address and device type you can
instantiate directly. Alternatively you can discover devices on the local network.

In order to do things with devices you must bring in the capabiliy traits from
`capabilities`.

## Discovery

To see all TPLink smart devices on the local network use `discovery::discover`.

```rust
use tplinker::{
  discovery::discover,
  devices::Device,
  capabilities::Switch,
};

fn main() {
  for (addr, data) in discover().unwrap() {
    let device = Device::from_data(addr, &data);
    let sysinfo = data.sysinfo();
    println!("{}\t{}\t{}", addr, sysinfo.alias, sysinfo.hw_type);
    match device {
      Device::HS110(device) => { device.switch_on().unwrap(); },
      _ => {},
    }
  }
}
```

## Direct device

To connect to a specific TPLink device use the specific device struct from `devices`.

```rust
use tplinker::{
  devices::LB110,
  capabilities::{Switch, Dimmer},
};

let device = LB110::new("192.168.0.99:9999").unwrap();
if device.is_on().unwrap() {
  let brightness = device.brightness().unwrap();
  if brightness < 50 {
    device.set_brightness(brightness + 20).unwrap();
  }
}
```

## Capabilities

In order to do things with devices you must bring in the relevant capability
traits from `capabilities`.


## CLI

There is a basic CLI:

```
tplinker discover

tplinker status 192.168.1.2 192.168.1.3

tplinker switch 192.168.1.4 on
```
