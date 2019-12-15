//! Structs for specific device models.
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
use std::{
    net::{AddrParseError, SocketAddr},
    result,
    str::FromStr,
};

use serde::de::DeserializeOwned;

use crate::{
    capabilities::{DeviceActions, Dimmer, Emeter, Light, Switch},
    datatypes::DeviceData,
    error::Result,
    protocol::{DefaultProtocol, Protocol},
};

// DEVICES

pub struct RawDevice {
    addr: SocketAddr,
    protocol: Box<dyn Protocol>,
}

impl RawDevice {
    pub fn new(addr: &str) -> result::Result<RawDevice, AddrParseError> {
        Ok(Self {
            addr: SocketAddr::from_str(addr)?,
            protocol: Box::new(DefaultProtocol::new()),
        })
    }

    pub fn from_addr(addr: SocketAddr) -> Self {
        Self {
            addr,
            protocol: Box::new(DefaultProtocol::new()),
        }
    }
}

impl DeviceActions for RawDevice {
    fn send<'a, T: DeserializeOwned>(&self, msg: &str) -> Result<T> {
        Ok(serde_json::from_str::<T>(
            &self.protocol.send(self.addr, msg)?,
        )?)
    }
}

macro_rules! new_device {
    ( $x:ident ) => {
        pub struct $x {
            raw: RawDevice,
        }

        impl $x {
            pub fn new(addr: &str) -> std::result::Result<Self, AddrParseError> {
                Ok(Self {
                    raw: RawDevice::new(addr)?,
                })
            }

            pub unsafe fn from_raw(raw: RawDevice) -> Self {
                Self { raw }
            }

            pub fn from_addr(addr: SocketAddr) -> Self {
                Self {
                    raw: RawDevice::from_addr(addr),
                }
            }
        }

        impl DeviceActions for $x {
            fn send<T: DeserializeOwned>(&self, msg: &str) -> Result<T> {
                self.raw.send(msg)
            }
        }
    };
}

new_device!(HS100);

impl Switch for HS100 {}

new_device!(HS110);

impl Switch for HS110 {}
impl Emeter for HS110 {}

new_device!(LB110);

impl Switch for LB110 {
    fn is_on(&self) -> Result<bool> {
        Ok(self.get_light_state()?.on_off == 1)
    }

    fn switch_on(&self) -> Result<()> {
        self.send(&r#"{"smartlife.iot.smartbulb.lightingservice":{"transition_light_state":{"on_off":1}}}"#)?;
        Ok(())
    }

    fn switch_off(&self) -> Result<()> {
        self.send(&r#"{"smartlife.iot.smartbulb.lightingservice":{"transition_light_state":{"on_off":0}}}"#)?;
        Ok(())
    }
}
impl Light for LB110 {}
impl Dimmer for LB110 {}
impl Emeter for LB110 {
    fn emeter_type(&self) -> String {
        String::from("smartlife.iot.common.emeter")
    }
}

/// An enum of the available device types.
///
/// This is returned from [`discover`](../discovery/fn.discover.html).
/// If the device type is not recognised but we can parse the response the
/// `Unknown` variant is returned.
pub enum Device {
    HS100(HS100),
    HS110(HS110),
    LB110(LB110),
    Unknown(RawDevice),
}

impl Device {
    pub fn from_data(addr: SocketAddr, device_data: &DeviceData) -> Device {
        let model = device_data.clone().sysinfo().model;
        if model.contains("HS100") {
            Device::HS100(HS100::from_addr(addr))
        } else if model.contains("HS110") {
            Device::HS110(HS110::from_addr(addr))
        } else if model.contains("LB110") {
            Device::LB110(LB110::from_addr(addr))
        } else {
            Device::Unknown(RawDevice::from_addr(addr))
        }
    }
}

impl DeviceActions for Device {
    fn send<T: DeserializeOwned>(&self, msg: &str) -> Result<T> {
        match self {
            Device::HS100(d) => d.send(msg),
            Device::HS110(d) => d.send(msg),
            Device::LB110(d) => d.send(msg),
            Device::Unknown(d) => d.send(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::tests::HS100_JSON;
    use crate::protocol::ProtocolMock;

    #[test]
    fn test_raw_device_submit_success() {
        // arrange
        let protocol = ProtocolMock::new();
        protocol.set_send_return_value(Ok(String::from(HS100_JSON)));
        let device = RawDevice {
            addr: "0.0.0.0:9999".parse().unwrap(),
            protocol: Box::new(protocol),
        };

        // act
        let device_data: DeviceData = device.send("{}").unwrap();

        // assert
        assert_eq!("Switch Two", device_data.sysinfo().alias);
    }

    #[test]
    fn test_raw_device_submit_failure() {
        let protocol = ProtocolMock::new();
        protocol.set_send_return_value(Ok(String::from("invalid")));
        let device = RawDevice {
            addr: "0.0.0.0:9999".parse().unwrap(),
            protocol: Box::new(protocol),
        };

        assert!(device.send::<DeviceData>("{}").is_err());
    }

    #[test]
    fn test_raw_device_location() {
        let protocol = ProtocolMock::new();
        protocol.set_send_return_value(Ok(String::from(HS100_JSON)));
        let device = RawDevice {
            addr: "0.0.0.0:9999".parse().unwrap(),
            protocol: Box::new(protocol),
        };

        assert_eq!((3456.0, 123.0), device.location().unwrap());
    }
}
