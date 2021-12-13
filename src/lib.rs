//! A library to query and control `TPLink` smart devices on the local network.
//!
//! Supported devices include HS100, HS110, LB110, LB120, KL110.
//!
//! Inspired and influenced by [`pyHS100`](https://github.com/GadgetReactor/pyHS100) and
//! [hs100api](https://github.com/abronan/hs100-rust-api).
//!
//! # Usage
//!
//! There are two main entrypoints. If you know the IP address and device type you can
//! instantiate directly. Alternatively you can discover devices on the local network.
//!
//! In order to do things with devices you must bring in the capabiliy traits from
//! [`capabilities`](./capabilities/index.html).
//!
//! ## Discovery
//!
//! To see all `TPLink` smart devices on the local network use
//! [`discovery::discover`](./discovery/fn.discover.html).
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
//!
//! ## Direct device
//!
//! To connect to a specific TPLink device use the specific device struct from
//! [`devices`](./devices/index.html).
//!
//! ```no_run
//! use tplinker::{
//!   devices::LB110,
//!   capabilities::{Switch, Dimmer},
//! };
//!
//! let device = LB110::new("192.168.0.99:9999").unwrap();
//! if device.is_on().unwrap() {
//!   let brightness = device.brightness().unwrap();
//!   if brightness < 50 {
//!     device.set_brightness(brightness + 20).unwrap();
//!   }
//! }
//! ```
//!
//! ## Capabilities
//!
//! In order to do things with devices you must bring in the relevant capability
//! traits from [`capabilities`](./capabilities/index.html).

#![deny(missing_docs)]

extern crate byteorder;

#[macro_use]
extern crate serde_derive;

pub mod capabilities;
pub mod datatypes;
pub mod devices;
pub mod discovery;
pub mod error;
mod protocol;

pub use discovery::discover;
