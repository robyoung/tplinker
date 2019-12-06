use std::{
    net::{AddrParseError, SocketAddr},
    result,
    str::FromStr,
    time::Duration,
};

use serde::de::DeserializeOwned;
use serde_json::json;

use crate::{
    datatypes::{DeviceData, GetLightStateResult, LightState, SetLightState, SysInfo},
    error::{Error, Result},
    protocol::{DefaultProtocol, Protocol},
};

pub trait DeviceActions {
    /// Send a message to a device and return its parsed response
    fn send<T: DeserializeOwned>(&self, msg: &str) -> Result<T>;

    fn sysinfo(&self) -> Result<SysInfo> {
        Ok(self
            .send::<DeviceData>(r#"{"system":{"get_sysinfo":null}}"#)?
            .sysinfo())
    }

    fn alias(&self) -> Result<String> {
        Ok(self.sysinfo()?.alias)
    }

    fn set_alias(&self, alias: &str) -> Result<()> {
        let command = json!({
            "system": {"set_dev_alias": {"alias": alias}}
        })
        .to_string();
        self.send(&command)?;
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
        let command = json!({
            "system": {"reboot": {"delay": delay.as_secs()}}
        })
        .to_string();
        self.send(&command)?;
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
        self.send(&r#"{"system":{"set_relay_state":{"state": 1}}}"#)?;
        Ok(())
    }

    fn switch_off(&self) -> Result<()> {
        self.send(&r#"{"system":{"set_relay_state":{"state": 0}}}"#)?;
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

pub trait Light: DeviceActions {
    fn get_light_state(&self) -> Result<LightState> {
        let command = json!({
            "none.iot.smartbulb2lightingservice": {
                "get_light_state": null
            }
        })
        .to_string();
        let data: GetLightStateResult = self.send(&command)?;
        data.light_state()
    }

    fn set_light_state(&self, light_state: SetLightState) -> Result<()> {
        let command = json!({
            "none.iot.smartbulb2lightingservice": {
                "transition_light_state": light_state,
            },
        })
        .to_string();
        self.send(&command)?;
        Ok(())
    }
}

pub trait Dimmer: Light {
    fn brightness(&self) -> Result<u16> {
        Ok(self.get_light_state()?.dft_on_state().brightness)
    }

    fn set_brightness(&self, brightness: u16) -> Result<()> {
        // TODO: figure out how to not send nulls
        self.set_light_state(SetLightState {
            on_off: None,
            hue: None,
            saturation: None,
            brightness: Some(brightness),
            color_temp: None,
        })?;
        Ok(())
    }
}

pub trait Colour: Light {
    fn get_hsv(&self) -> Result<(u16, u16, u16)> {
        let light_state = self.get_light_state()?;
        let dft_on_state = light_state.dft_on_state();

        Ok((
            dft_on_state.hue,
            dft_on_state.saturation,
            dft_on_state.brightness,
        ))
    }

    fn set_hsv(&self, hue: u16, saturation: u16, brightness: u16) -> Result<()> {
        if hue > 360 {
            return Err(Error::Other(String::from(
                "Invalid hue; must be between 0 and 360",
            )));
        }
        if saturation > 100 {
            return Err(Error::Other(String::from(
                "Invalid saturation; must be between 0 and 100",
            )));
        }
        if brightness > 100 {
            return Err(Error::Other(String::from(
                "Invalid brightness; must be between 0 and 100",
            )));
        }
        self.set_light_state(SetLightState {
            on_off: None,
            hue: Some(hue),
            saturation: Some(saturation),
            brightness: Some(brightness),
            color_temp: None,
        })?;
        Ok(())
    }
}

pub trait Emeter: DeviceActions {
    fn emeter_type(&self) -> String {
        String::from("emeter")
    }

    // TODO: add proper return type
    fn get_emeter_realtime(&self) -> Result<serde_json::Value> {
        let command = json!({
            self.emeter_type(): {"get_realtime": null}
        }).to_string();
        Ok(self.send(&command)?)
    }

    // TODO: add proper return type
    fn get_emeter_daily(&self, year: u16, month: u8) -> Result<serde_json::Value> {
        let command = json!({
            self.emeter_type(): {"get_daystat": {"month": month, "year": year}}
        }).to_string();
        Ok(self.send(&command)?)
    }

    // TODO: add proper return type
    fn get_emeter_monthly(&self, year: u16) -> Result<serde_json::Value> {
        let command = json!({
            self.emeter_type(): {"get_monthstat": {"year": year}}
        }).to_string();
        Ok(self.send(&command)?)
    }
}

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
