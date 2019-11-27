# TPLinker

A rust library to query and control TPLink smart plugs and smart lights.

Inspired and influenced by [pyHS100](https://github.com/GadgetReactor/pyHS100) and
[hs100api](https://github.com/abronan/hs100-rust-api).

**Work in progress**

## Usage

Discovery:
```rust
use tplinker::discovery::discover;

fn main() {
  for (addr, device) in discover().unwrap() {
    let sysinfo = device.sysinfo();
    println!("{}\t{}\t{}", addr, sysinfo.alias, sysinfo.hw_type);
  }
}
```

Devices
```
let plug = HS100::new("192.1.1.10:9999")?;
plug.is_on()
plug.switch_on()
```
