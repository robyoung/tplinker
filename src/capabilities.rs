//! Features available on devices
//!
//! Different devices have different combinations of capabilities available to them. To
//! make these easier to work with in a type safe and consistent way sets of functions
//! are grouped together into capability traits that can be implemented on devices.
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::json;

use crate::{
    datatypes::{
        DeviceData, GetLightStateResult, LightState, SetLightState, SysInfo, LIGHT_SERVICE,
    },
    error::{Error, Result},
};

/// The basic set of functions available to all TPlink smart devices
///
/// All devices support this trait.
pub trait DeviceActions {
    /// Send a message to a device and return its parsed response
    fn send<T: DeserializeOwned>(&self, msg: &str) -> Result<T>;

    /// Get system information
    fn sysinfo(&self) -> Result<SysInfo> {
        Ok(self
            .send::<DeviceData>(r#"{"system":{"get_sysinfo":null}}"#)?
            .into_sysinfo())
    }

    /// Get the alias of the device
    ///
    /// This is a user defined name for the device.
    fn alias(&self) -> Result<String> {
        Ok(self.sysinfo()?.alias)
    }

    /// Set the alias of the device
    ///
    /// This is a user defined name for the device.
    fn set_alias(&self, alias: &str) -> Result<()> {
        let command = json!({
            "system": {"set_dev_alias": {"alias": alias}}
        })
        .to_string();
        check_command_error(self.send(&command)?, "/system/set_dev_alias/err_code")
    }

    /// Get the latitude and longitude coordinates
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

    /// Reboot the device in 1 second
    fn reboot(&self) -> Result<()> {
        self.reboot_with_delay(Duration::from_secs(1))
    }

    /// Reboot the device with a specified delay
    fn reboot_with_delay(&self, delay: Duration) -> Result<()> {
        let command = json!({
            "system": {"reboot": {"delay": delay.as_secs()}}
        })
        .to_string();

        check_command_error(self.send(&command)?, "/system/reboot/err_code")
    }
}

/// Devices that can be switched on and off
///
/// All devices support this trait.
pub trait Switch: DeviceActions {
    /// Check whether the device is on
    fn is_on(&self) -> Result<bool> {
        if let Some(relay_state) = self.sysinfo()?.relay_state {
            Ok(relay_state > 0)
        } else {
            Err(Error::Other(String::from("No relay state")))
        }
    }

    /// Check whether the device is off
    fn is_off(&self) -> Result<bool> {
        Ok(!self.is_on()?)
    }

    /// Switch the device on
    fn switch_on(&self) -> Result<()> {
        check_command_error(self.send(&r#"{"system":{"set_relay_state":{"state": 1}}}"#)?, "/system/set_relay_state/err_code")
    }

    /// Switch the device off
    fn switch_off(&self) -> Result<()> {
        check_command_error(self.send(&r#"{"system":{"set_relay_state":{"state": 0}}}"#)?, "/system/set_relay_state/err_code")
    }

    /// Toggle the device's on state
    ///
    /// If the device is on, switch it off.
    /// If the device is off, switch it on.
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

/// Smart light devices
///
/// The LB class of devices support this trait.
pub trait Light: DeviceActions {
    /// Get the current state of the light
    fn get_light_state(&self) -> Result<LightState> {
        let command = json!({
            LIGHT_SERVICE: {
                "get_light_state": null
            }
        })
        .to_string();
        let data: GetLightStateResult = self.send(&command)?;
        data.light_state()
    }

    /// Set the state of the light
    ///
    /// This is a low level method, and has no validation. You should use one of the
    /// higher level methods such as [`set_brightness`](./trait.Dimmer.html#method.set_brightness)
    /// or [`set_hsv`](./trait.Colour.html#method.set_hsv).
    fn set_light_state(&self, light_state: SetLightState) -> Result<LightState> {
        let command = json!({
            LIGHT_SERVICE: {
                "transition_light_state": light_state,
            },
        })
        .to_string();
        self.send::<GetLightStateResult>(&command)?.light_state()
    }
}

/// Dimmable smart light devices
pub trait Dimmer: Light {
    /// Get percentage brightness of bulb
    fn brightness(&self) -> Result<u16> {
        Ok(self.get_light_state()?.dft_on_state().brightness)
    }

    /// Set percentage brightness of bulb
    fn set_brightness(&self, brightness: u16) -> Result<()> {
        if brightness > 100 {
            Err(Error::Other(String::from(
                "Brightness must be between 0 and 100",
            )))
        } else {
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
}

/// Full colour smart light devices
pub trait Colour: Light {
    /// Get hue, saturation and value (brightness)
    fn get_hsv(&self) -> Result<(u16, u16, u16)> {
        let light_state = self.get_light_state()?;
        let dft_on_state = light_state.dft_on_state();

        Ok((
            dft_on_state.hue,
            dft_on_state.saturation,
            dft_on_state.brightness,
        ))
    }

    /// Get hue, saturation and value (brightness)
    ///
    /// Hue must be between 0 and 360.
    /// Saturation must be between 0 and 100.
    /// Brightness must be between 0 and 100.
    fn set_hsv(&self, hue: u16, saturation: u16, brightness: u16) -> Result<()> {
        if hue > 360 {
            return Err(Error::Other(String::from("Hue must be between 0 and 360")));
        }
        if saturation > 100 {
            return Err(Error::Other(String::from(
                "Saturation must be between 0 and 100",
            )));
        }
        if brightness > 100 {
            return Err(Error::Other(String::from(
                "Brightness must be between 0 and 100",
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

/// Smart devices with energy usage tracking.
pub trait Emeter: DeviceActions {
    /// Type of the emeter
    ///
    /// This is used by other Emeter methods. It is probably not useful to end users.
    fn emeter_type(&self) -> String {
        String::from("emeter")
    }

    /// Get the realtime energy usage
    // TODO: add proper return type
    fn get_emeter_realtime(&self) -> Result<serde_json::Value> {
        let command = json!({
            self.emeter_type(): {"get_realtime": null}
        })
        .to_string();
        Ok(self.send(&command)?)
    }

    /// Get the daily energy usage for a given month
    // TODO: add proper return type
    fn get_emeter_daily(&self, year: u16, month: u8) -> Result<serde_json::Value> {
        let command = json!({
            self.emeter_type(): {"get_daystat": {"month": month, "year": year}}
        })
        .to_string();
        Ok(self.send(&command)?)
    }

    /// Get the monthly energy usage for a given year
    // TODO: add proper return type
    fn get_emeter_monthly(&self, year: u16) -> Result<serde_json::Value> {
        let command = json!({
            self.emeter_type(): {"get_monthstat": {"year": year}}
        })
        .to_string();
        Ok(self.send(&command)?)
    }
}

/// Check the error code of a standard command
fn check_command_error(value: serde_json::Value, pointer: &str) -> Result<()> {
    if let Some(err_code) = value.pointer(pointer) {
        if err_code == 0 {
            Ok(())
        } else {
            Err(Error::Other(format!("Invalid error code {}", err_code)))
        }
    } else {
        Err(Error::Other(format!("Invalid response format: {}", value)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::tests::HS100_JSON;
    use std::cell::Cell;

    struct DummyDevice {
        msgs: Cell<Vec<String>>,
        resps: Cell<Vec<Result<String>>>,
    }

    impl DummyDevice {
        fn new(resp: Result<String>) -> Self {
            Self::multi(vec![resp])
        }

        fn multi(resps: Vec<Result<String>>) -> Self {
            Self {
                msgs: Cell::new(Vec::new()),
                resps: Cell::new(resps),
            }
        }

        fn reset(&self, resp: Result<String>) {
            self.msgs.set(Vec::new());
            self.resps.set(vec![resp]);
        }

        fn push_msg(&self, msg: &str) {
            let mut msgs = self.msgs.replace(vec![]);
            msgs.push(msg.to_string());
            self.msgs.set(msgs);
        }

        fn pop_resp(&self) -> Result<String> {
            let mut resps = self.resps.replace(vec![]);
            let resp = resps.remove(0);
            self.resps.set(resps);
            resp
        }
    }

    impl DeviceActions for DummyDevice {
        fn send<D: DeserializeOwned>(&self, msg: &str) -> Result<D> {
            self.push_msg(msg);
            match self.pop_resp() {
                Ok(resp) => Ok(serde_json::from_str::<D>(&resp)?),
                Err(err) => Err(err),
            }
        }
    }

    impl Switch for DummyDevice {}

    #[test]
    fn device_sysinfo() {
        let device = DummyDevice::new(Ok(HS100_JSON.to_string()));

        device.sysinfo().unwrap();
    }

    #[test]
    fn device_alias() {
        let device = DummyDevice::new(Ok(HS100_JSON.to_string()));

        assert_eq!(device.alias().unwrap(), "Switch Two".to_string());
    }

    #[test]
    fn device_set_alias() {
        let device = DummyDevice::new(Ok(
            r#"{"system":{"set_dev_alias":{"err_code":0}}}"#.to_string()
        ));

        device.set_alias("dave").unwrap();

        let inner = device.msgs.into_inner();

        assert_eq!(inner[0], r#"{"system":{"set_dev_alias":{"alias":"dave"}}}"#);
    }

    #[test]
    fn device_location() {
        let device = DummyDevice::new(Ok(HS100_JSON.to_string()));

        assert_eq!(device.location().unwrap(), (3456.0, 123.0));
    }

    #[test]
    fn device_reboot_with_delay() {
        let device = DummyDevice::new(Ok(r#"{"system":{"reboot":{"err_code":0}}}"#.to_string()));

        device.reboot_with_delay(Duration::from_secs(120)).unwrap();

        assert_eq!(
            device.msgs.into_inner()[0],
            r#"{"system":{"reboot":{"delay":120}}}"#
        );
    }

    #[test]
    fn device_reboot() {
        let device = DummyDevice::new(Ok(r#"{"system":{"reboot":{"err_code":0}}}"#.to_string()));

        device.reboot().unwrap();

        assert_eq!(
            device.msgs.into_inner()[0],
            r#"{"system":{"reboot":{"delay":1}}}"#
        );
    }

    #[test]
    fn switch_is_on_off() {
        let device = DummyDevice::new(Ok(HS100_JSON.to_string()));

        assert_eq!(device.is_on().unwrap(), false);

        device.reset(Ok(HS100_JSON.to_string()));
        assert_eq!(device.is_off().unwrap(), true);
    }

    #[test]
    fn switch_on() {
        let device = DummyDevice::new(Ok(
            r#"{"system":{"set_relay_state":{"err_code":0}}}"#.to_string()
        ));

        device.switch_on().unwrap();

        assert_eq!(
            device.msgs.into_inner()[0],
            r#"{"system":{"set_relay_state":{"state": 1}}}"#
        );
    }

    #[test]
    fn switch_off() {
        let device = DummyDevice::new(Ok(
            r#"{"system":{"set_relay_state":{"err_code":0}}}"#.to_string()
        ));

        device.switch_off().unwrap();

        assert_eq!(
            device.msgs.into_inner()[0],
            r#"{"system":{"set_relay_state":{"state": 0}}}"#
        );
    }

    #[test]
    fn switch_toggle() {
        let device = DummyDevice::multi(vec![
            Ok(HS100_JSON.to_string()),
            Ok(r#"{"system":{"set_relay_state":{"err_code":0}}}"#.to_string()),
        ]);

        assert_eq!(device.toggle().unwrap(), true);
        assert_eq!(device.msgs.into_inner(), vec![
            r#"{"system":{"get_sysinfo":null}}"#,
            r#"{"system":{"set_relay_state":{"state": 1}}}"#,
        ]);
    }

}
