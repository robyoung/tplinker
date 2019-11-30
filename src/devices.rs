use crate::{
    datatypes::{DeviceData, SysInfo},
    error::{Error, Result},
    protocol::{DefaultProtocol, Protocol},
};
use std::{
    net::{AddrParseError, SocketAddr},
    result,
    str::FromStr,
    time::Duration,
};

// TODO: move things around as private items are visible to sub-modules
pub trait DeviceActions {
    /// Send a message to a device and return its parsed response
    fn submit(&self, msg: &str) -> Result<DeviceData>;

    fn sysinfo(&self) -> Result<SysInfo> {
        let device_data = self.submit(r#"{"system":{"get_sysinfo":null}}"#)?;
        Ok(device_data.sysinfo())
    }

    fn alias(&self) -> Result<String> {
        Ok(self.sysinfo()?.alias)
    }

    fn set_alias(&self, alias: &str) -> Result<()> {
        // TODO: investigate a command helper
        let command = format!(
            r#"{{"system":{{"set_dev_alias": {{"alias": {}}}}}}}"#,
            alias
        );
        self.submit(&command)?;
        Ok(())
    }

    fn location(&self) -> Result<(f64, f64)> {
        let sysinfo = self.sysinfo()?;
        if let (Some(latitude), Some(longitude)) = (sysinfo.latitude, sysinfo.longitude) {
            Ok((latitude, longitude))
        } else if let (Some(latitude_i), Some(longitude_i)) =
            (sysinfo.latitude_i, sysinfo.longitude_i)
        {
            Ok((f64::from(latitude_i), f64::from(longitude_i)))
        } else {
            Err(Error::Other(String::from("Complete coordinates not found")))
        }
    }

    fn reboot(&self) -> Result<()> {
        self.reboot_with_delay(Duration::from_secs(1))
    }

    fn reboot_with_delay(&self, delay: Duration) -> Result<()> {
        let command = format!(
            r#"{{"system":{{"reboot":{{"delay": {}}}}}}}"#,
            delay.as_secs()
        );
        self.submit(&command)?;
        Ok(())
    }
}

pub trait Switch: DeviceActions {
    fn is_on(&self) -> Result<bool> {
        if let Some(relay_state) = self.sysinfo()?.relay_state {
            Ok(relay_state > 0)
        } else {
            Err(Error::Other(String::from("No relay state")))
        }
    }

    fn is_off(&self) -> Result<bool> {
        Ok(!self.is_on()?)
    }

    fn switch_on(&self) -> Result<()> {
        self.submit(&r#"{"system":{"set_relay_state":{"state": 1}}}"#)?;
        Ok(())
    }

    fn switch_off(&self) -> Result<()> {
        self.submit(&r#"{{"system":{{"set_relay_state":{{"state": 0}}}}}}"#)?;
        Ok(())
    }

    fn toggle(&self) -> Result<bool> {
        if self.is_on()? {
            self.switch_off()?;
            Ok(false)
        } else {
            self.switch_on()?;
            Ok(true)
        }
    }
}

pub trait Light: DeviceActions {}

pub trait Dimmer: Light {}

pub trait Colour: Light {}

pub trait Emeter: DeviceActions {}

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
    fn submit(&self, msg: &str) -> Result<DeviceData> {
        Ok(serde_json::from_str::<DeviceData>(
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

            pub fn from_addr(addr: SocketAddr) -> Self {
                Self {
                    raw: RawDevice::from_addr(addr),
                }
            }
        }

        impl DeviceActions for $x {
            fn submit(&self, msg: &str) -> Result<DeviceData> {
                self.raw.submit(msg)
            }
        }
    };
}

// TODO: should it be HS110 and HS100 or simply SmartPlug like in pyhs100?
// TODO: create a declarative macro to generate device structs with constructors
new_device!(HS100);

impl Switch for HS100 {}

new_device!(HS110);

impl Switch for HS110 {}
impl Emeter for HS110 {}

new_device!(LB110);

impl Switch for LB110 {}
impl Light for LB110 {}
impl Dimmer for LB110 {}

pub enum Device {
    HS100(HS100),
    HS110(HS110),
    LB110(LB110),
    Unknown(RawDevice),
}

impl Device {
    pub fn from_data(addr: SocketAddr, device_data: &DeviceData) -> Device {
        let model = device_data.system.sysinfo.model.clone();
        if model.contains("HS100") {
            Device::HS100(HS100::from_addr(addr))
        } else if model.contains("HS100") {
            Device::HS110(HS110::from_addr(addr))
        } else if model.contains("LB110") {
            Device::LB110(LB110::from_addr(addr))
        } else {
            Device::Unknown(RawDevice::from_addr(addr))
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
        let device_data = device.submit("{}").unwrap();

        // assert
        assert_eq!("Switch Two", device_data.system.sysinfo.alias);
    }

    #[test]
    fn test_raw_device_submit_failure() {
        let protocol = ProtocolMock::new();
        protocol.set_send_return_value(Ok(String::from("invalid")));
        let device = RawDevice {
            addr: "0.0.0.0:9999".parse().unwrap(),
            protocol: Box::new(protocol),
        };

        assert!(device.submit("{}").is_err());
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
