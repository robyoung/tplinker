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
    capabilities::{ColorTemperature, DeviceActions, Dimmer, Emeter, Light, MultiSwitch, Switch},
    datatypes::{DeviceData, GetLightStateResult},
    error::Result,
    protocol::{DefaultProtocol, Protocol},
};

// DEVICES

/// A raw, generic smart device
#[derive(Clone, Debug)]
pub struct RawDevice<T: Protocol> {
    addr: SocketAddr,
    protocol: T,
}

impl RawDevice<DefaultProtocol> {
    /// Make a raw device from an address string
    pub fn new(addr: &str) -> result::Result<RawDevice<DefaultProtocol>, AddrParseError> {
        Ok(Self {
            addr: SocketAddr::from_str(addr)?,
            protocol: DefaultProtocol::default(),
        })
    }

    /// Make a raw device from an address struct
    pub fn from_addr(addr: SocketAddr) -> Self {
        Self {
            addr,
            protocol: DefaultProtocol::default(),
        }
    }
}

impl<T: Protocol> DeviceActions for RawDevice<T> {
    fn send<'a, D: DeserializeOwned>(&self, msg: &str) -> Result<D> {
        Ok(serde_json::from_str::<D>(
            &self.protocol.send(self.addr, msg)?,
        )?)
    }
}

macro_rules! new_device {
    ( $x:ident, $description:expr ) => {
        new_device! {
            $x
            => main # concat!("A ", stringify!($x), " ", $description, "\n\nWhen directly creating a device using the `from_*` methods below, you must make sure that the address you pass is indeed that of a ", stringify!($x), ", as there is no further checking.")
            => new # concat!("Make a ", stringify!($x), " device from an address string")
            => raw # concat!("Make a ", stringify!($x), " device from an already constructed raw device")
            => addr # concat!("Make a ", stringify!($x), " device from an address struct")
        }
    };
    ( $x:ident
      => main # $docmain:expr
      => new # $docnew:expr
      => raw # $docraw:expr
      => addr # $docaddr:expr ) => {
        #[doc = $docmain]
        #[derive(Clone, Debug)]
        pub struct $x<T: Protocol> {
            raw: RawDevice<T>,
        }

        impl $x<DefaultProtocol> {
            #[doc = $docnew]
            pub fn new(addr: &str) -> std::result::Result<Self, AddrParseError> {
                Ok(Self {
                    raw: RawDevice::new(addr)?,
                })
            }

            #[doc = $docaddr]
            pub fn from_addr(addr: SocketAddr) -> Self {
                Self {
                    raw: RawDevice::from_addr(addr),
                }
            }
        }

        impl<T: Protocol> $x<T> {
            #[doc = $docraw]
            pub fn from_raw(raw: RawDevice<T>) -> Self {
                Self { raw }
            }
        }

        impl<T: Protocol> DeviceActions for $x<T> {
            fn send<D: DeserializeOwned>(&self, msg: &str) -> Result<D> {
                self.raw.send(msg)
            }
        }
    };
}

new_device!(HS100, "smart plug");

impl<T: Protocol> Switch for HS100<T> {}

new_device!(HS103, "smart plug");

impl<T: Protocol> Switch for HS103<T> {}

new_device!(HS105, "smart plug mini");

impl<T: Protocol> Switch for HS105<T> {}

new_device!(HS110, "smart plug with energy monitoring");

impl<T: Protocol> Switch for HS110<T> {}
impl<T: Protocol> Emeter for HS110<T> {}

new_device!(KP115, "smart plug mini with energy monitoring");

impl<T: Protocol> Switch for KP115<T> {}
impl<T: Protocol> Emeter for KP115<T> {}

new_device!(HS300, "smart power strip with energy monitoring");

impl<T: Protocol> MultiSwitch for HS300<T> {}
impl<T: Protocol> Emeter for HS300<T> {}

new_device!(LB110, "dimmable smart lightbulb");

impl<T: Protocol> Switch for LB110<T> {
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
impl<T: Protocol> Light for LB110<T> {}
impl<T: Protocol> Dimmer for LB110<T> {}
impl<T: Protocol> Emeter for LB110<T> {
    fn emeter_type(&self) -> String {
        String::from("smartlife.iot.common.emeter")
    }
}

new_device!(LB120, "tunable white color smart lightbulb");

impl<T: Protocol> Switch for LB120<T> {
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
impl<T: Protocol> Light for LB120<T> {}
impl<T: Protocol> Dimmer for LB120<T> {}
impl<T: Protocol> ColorTemperature for LB120<T> {}
impl<T: Protocol> Emeter for LB120<T> {
    fn emeter_type(&self) -> String {
        String::from("smartlife.iot.common.emeter")
    }
}

new_device!(KL110, "dimmable smart lightbulb");

impl<T: Protocol> Switch for KL110<T> {
    fn is_on(&self) -> Result<bool> {
        Ok(self.get_light_state()?.on_off == 1)
    }

    fn switch_on(&self) -> Result<()> {
        self.send::<GetLightStateResult>(&r#"{"smartlife.iot.smartbulb.lightingservice":{"transition_light_state":{"on_off":1}}}"#)?;
        Ok(())
    }

    fn switch_off(&self) -> Result<()> {
        self.send::<GetLightStateResult>(&r#"{"smartlife.iot.smartbulb.lightingservice":{"transition_light_state":{"on_off":0}}}"#)?;
        Ok(())
    }
}
impl<T: Protocol> Light for KL110<T> {}
impl<T: Protocol> Dimmer for KL110<T> {}
impl<T: Protocol> Emeter for KL110<T> {
    fn emeter_type(&self) -> String {
        String::from("smartlife.iot.common.emeter")
    }
}

/// An enum of the available device types.
///
/// This is returned from [`discover`](../discovery/fn.discover.html).
/// If the device type is not recognised but we can parse the response the
/// `Unknown` variant is returned.
#[derive(Clone, Debug)]
pub enum Device {
    /// Device variant for an HS100 smart plug
    HS100(HS100<DefaultProtocol>),
    /// Device variant for an HS103 smart plug
    HS103(HS103<DefaultProtocol>),
    /// Device variant for an HS105 smart plug
    HS105(HS105<DefaultProtocol>),
    /// Device variant for an HS110 smart plug
    HS110(HS110<DefaultProtocol>),
    /// Device variant for an HS300 smart power strip
    HS300(HS300<DefaultProtocol>),
    /// Device variant for an LB110 smart light
    LB110(LB110<DefaultProtocol>),
    /// Device variant for an LB120 smart light
    LB120(LB120<DefaultProtocol>),
    /// Device variant for an KL110 smart light
    KL110(KL110<DefaultProtocol>),
    /// Device variant for an KP115 smart plug
    KP115(KP115<DefaultProtocol>),
    /// Device variant for an unknown device
    Unknown(RawDevice<DefaultProtocol>),
}

impl Device {
    /// Create a device from a raw `DeviceData`. Usually used from
    /// [`discover`](../discovery/fn.discover.html).
    pub fn from_data(addr: SocketAddr, device_data: &DeviceData) -> Device {
        let model = &device_data.sysinfo().model;
        if model.contains("HS100") {
            Device::HS100(HS100::from_addr(addr))
        } else if model.contains("HS103") {
            Device::HS103(HS103::from_addr(addr))
        } else if model.contains("HS105") {
            Device::HS105(HS105::from_addr(addr))
        } else if model.contains("HS110") {
            Device::HS110(HS110::from_addr(addr))
        } else if model.contains("HS300") {
            Device::HS300(HS300::from_addr(addr))
        } else if model.contains("LB110") {
            Device::LB110(LB110::from_addr(addr))
        } else if model.contains("LB120") {
            Device::LB120(LB120::from_addr(addr))
        } else if model.contains("KL110") {
            Device::KL110(KL110::from_addr(addr))
        } else if model.contains("KP115") {
            Device::KP115(KP115::from_addr(addr))
        } else {
            Device::Unknown(RawDevice::from_addr(addr))
        }
    }
}

impl DeviceActions for Device {
    fn send<D: DeserializeOwned>(&self, msg: &str) -> Result<D> {
        match self {
            Device::HS100(d) => d.send(msg),
            Device::HS103(d) => d.send(msg),
            Device::HS105(d) => d.send(msg),
            Device::HS110(d) => d.send(msg),
            Device::HS300(d) => d.send(msg),
            Device::LB110(d) => d.send(msg),
            Device::LB120(d) => d.send(msg),
            Device::KL110(d) => d.send(msg),
            Device::KP115(d) => d.send(msg),
            Device::Unknown(d) => d.send(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::tests::HS100_JSON_OFF;
    use crate::protocol::mock::ProtocolMock;

    #[test]
    fn test_raw_device_submit_success() {
        // arrange
        let protocol = ProtocolMock::default();
        protocol.set_send_return_value(Ok(String::from(HS100_JSON_OFF)));
        let device = RawDevice {
            addr: "0.0.0.0:9999".parse().unwrap(),
            protocol,
        };

        // act
        let device_data: DeviceData = device.send("{}").unwrap();

        // assert
        assert_eq!("Switch Two", device_data.sysinfo().alias);
    }

    #[test]
    fn test_raw_device_submit_failure() {
        let protocol = ProtocolMock::default();
        protocol.set_send_return_value(Ok(String::from("invalid")));
        let device = RawDevice {
            addr: "0.0.0.0:9999".parse().unwrap(),
            protocol,
        };

        assert!(device.send::<DeviceData>("{}").is_err());
    }

    #[test]
    fn test_raw_device_location() {
        let protocol = ProtocolMock::default();
        protocol.set_send_return_value(Ok(String::from(HS100_JSON_OFF)));
        let device = RawDevice {
            addr: "0.0.0.0:9999".parse().unwrap(),
            protocol,
        };

        assert_eq!((3456.0, 123.0), device.location().unwrap());
    }
}
