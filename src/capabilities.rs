use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::json;

use crate::{
    datatypes::{DeviceData, GetLightStateResult, LightState, SetLightState, SysInfo, LIGHT_SERVICE},
    error::{Error, Result},
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
            LIGHT_SERVICE: {
                "get_light_state": null
            }
        })
        .to_string();
        let data: GetLightStateResult = self.send(&command)?;
        data.light_state()
    }

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

