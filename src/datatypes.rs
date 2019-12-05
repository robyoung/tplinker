use crate::error::{Error, Result, SectionError};

type ErrCode = i16;

#[derive(Debug, Deserialize, Clone)]
pub struct DeviceData {
    pub system: System,
    // TODO: remove these in favour of action specific types
    pub emeter: Option<SectionResult<Emeter>>,
    #[serde(flatten)]
    pub smartlife: Smartlife,
}

impl DeviceData {
    pub fn sysinfo(self) -> SysInfo {
        self.system.sysinfo
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum SectionResult<T> {
    Ok(T),
    Err(SectionError),
}

impl<T> SectionResult<T> {
    pub fn unwrap(self) -> T {
        match self {
            Self::Ok(section) => section,
            Self::Err(_) => panic!("expecting section"),
        }
    }
}

#[derive(Clone, Deserialize, Debug)]
pub struct GetLightStateResult {
    #[serde(rename = "smartlife.iot.smartbulb.lightingservice")]
    pub lightingservice: SectionResult<GetLightState>,
}

impl GetLightStateResult {
    pub fn light_state(self) -> Result<LightState> {
        match self.lightingservice {
            SectionResult::Ok(light_state) => Ok(light_state.light_state),
            SectionResult::Err(err) => Err(Error::from(err.clone())),
        }
    }
}

#[derive(Clone, Deserialize, Debug)]
pub struct GetLightState {
    #[serde(rename = "get_light_state")]
    #[serde(alias = "transition_light_state")]
    pub light_state: LightState,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Smartlife {
    #[serde(rename = "smartlife.iot.dimmer")]
    pub dimmer: Option<SectionError>,
    #[serde(rename = "smartlife.iot.common.emeter")]
    pub emeter: Option<SectionResult<SmartlifeEmeter>>,
    #[serde(rename = "smartlife.iot.smartbulb.lightingservice")]
    pub lightingservice: Option<SectionResult<SmartlifeLightingService>>,
}

impl Smartlife {
    pub fn emeter(&self) -> Result<&SmartlifeEmeter> {
        if let Some(emeter) = &self.emeter {
            match emeter {
                SectionResult::Ok(emeter) => Ok(emeter),
                SectionResult::Err(err) => Err(Error::from(err.clone())),
            }
        } else {
            Err(Error::Other(String::from("No emeter present")))
        }
    }

    pub fn lightingservice(&self) -> Result<&SmartlifeLightingService> {
        if let Some(l) = &self.lightingservice {
            match l {
                SectionResult::Ok(l) => Ok(l),
                SectionResult::Err(err) => Err(Error::from(err.clone())),
            }
        } else {
            Err(Error::Other(String::from("No lighting service present")))
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct SmartlifeEmeter {
    #[serde(rename = "get_realtime")]
    pub realtime: SmartlifeEmeterRealtime,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SmartlifeEmeterRealtime {
    pub power_mw: u32,
    pub err_code: ErrCode,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SmartlifeLightingService {
    #[serde(rename = "get_light_state")]
    pub light_state: LightState,
}

#[derive(Debug, Deserialize, Clone)]
pub struct System {
    #[serde(rename = "get_sysinfo")]
    pub sysinfo: SysInfo,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SysInfo {
    // COMMON
    pub sw_ver: String,
    pub hw_ver: String,
    #[serde(alias = "type")]
    #[serde(alias = "mic_type")]
    pub hw_type: String,
    pub model: String,
    #[serde(alias = "mic_mac")]
    pub mac: String,
    #[serde(rename = "deviceId")]
    pub device_id: String,
    #[serde(rename = "hwId")]
    pub hw_id: String,
    #[serde(rename = "oemId")]
    pub oem_id: String,
    pub alias: String,
    #[serde(alias = "description")]
    pub dev_name: String,
    pub err_code: ErrCode,
    pub rssi: i32,
    pub active_mode: String, // TODO: Could be enum

    // TODO: group fields together
    // HS..
    #[serde(rename = "fwId")]
    pub fw_id: Option<String>,
    pub relay_state: Option<u8>,
    pub on_time: Option<i64>,
    pub feature: Option<String>, // TODO: Could be enum
    pub updating: Option<u8>,
    pub icon_hash: Option<String>,
    pub led_off: Option<u8>,

    // HS100
    pub longitude_i: Option<i32>,
    pub latitude_i: Option<i32>,
    pub ntc_state: Option<u8>,    // TODO: what is this?

    // HS110
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,

    // LB110
    pub light_state: Option<LightState>,
    pub is_dimmable: Option<u8>,
    pub is_color: Option<u8>,
    pub is_variable_color_temp: Option<u8>,
    pub heapsize: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LightState {
    pub on_off: u8,
    #[serde(rename = "dft_on_state")]
    dft_off_on_state: Option<DftOnState>,
    #[serde(flatten)]
    dft_on_on_state: Option<DftOnState>,
    pub err_code: Option<ErrCode>,
}

impl LightState {
    pub fn dft_on_state(&self) -> &DftOnState {
        if let Some(dft_on_state) = &self.dft_off_on_state {
            dft_on_state
        } else if let Some(dft_on_state) = &self.dft_on_on_state {
            dft_on_state
        } else {
            panic!("dft_on_state must be present in some way");
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct DftOnState {
    pub mode: String,
    pub hue: i32,
    pub saturation: i32,
    pub color_temp: i32,
    pub brightness: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Emeter {
    #[serde(rename = "get_realtime")]
    pub realtime: SectionResult<EmeterRealtime>,
    // TODO: add other stats aggregations
}

#[derive(Debug, Deserialize, Clone)]
pub struct EmeterRealtime {
    pub current: f64,
    pub voltage: f64,
    pub power: f64,
    pub total: f64,
    pub err_code: ErrCode,
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub const HS100_JSON: &'static str = r#"{
      "system": {
        "get_sysinfo": {
          "sw_ver": "1.5.8 Build 180815 Rel.135935",
          "hw_ver": "2.1",
          "type": "IOT.SMARTPLUGSWITCH",
          "model": "HS100(UK)",
          "mac": "00:00:00:00:00:00",
          "dev_name": "Smart Wi-Fi Plug",
          "alias": "Switch Two",
          "relay_state": 0,
          "on_time": 0,
          "active_mode": "none",
          "feature": "TIM",
          "updating": 0,
          "icon_hash": "",
          "rssi": -53,
          "led_off": 0,
          "longitude_i": 123,
          "latitude_i": 3456,
          "hwId": "00000000000000000000000000000000",
          "fwId": "00000000000000000000000000000000",
          "deviceId": "0000000000000000000000000000000000000000",
          "oemId": "FDD18403D5E8DB3613009C820963E018",
          "next_action": {
            "type": -1
          },
          "ntc_state": 0,
          "err_code": 0
        }
      },
      "emeter": {
        "get_realtime": {
          "err_code": -1,
          "err_msg": "module not support"
        }
      },
      "smartlife.iot.dimmer": {
        "err_code": -1,
        "err_msg": "module not support"
      },
      "smartlife.iot.common.emeter": {
        "err_code": -1,
        "err_msg": "module not support"
      },
      "smartlife.iot.smartbulb.lightingservice": {
        "err_code": -1,
        "err_msg": "module not support"
      }
    }"#;

    const HS110_JSON: &'static str = r#"{
      "system": {
        "get_sysinfo": {
          "err_code": 0,
          "sw_ver": "1.2.5 Build 171213 Rel.095335",
          "hw_ver": "1.0",
          "type": "IOT.SMARTPLUGSWITCH",
          "model": "HS110(UK)",
          "mac": "00:00:00:00:00:00",
          "deviceId": "0000000000000000000000000000000000000000",
          "hwId": "00000000000000000000000000000000",
          "fwId": "00000000000000000000000000000000",
          "oemId": "90AEEA7AECBF1A879FCA3C104C58C4D8",
          "alias": "Switch One",
          "dev_name": "Wi-Fi Smart Plug With Energy Monitoring",
          "icon_hash": "",
          "relay_state": 1,
          "on_time": 12521,
          "active_mode": "schedule",
          "feature": "TIM:ENE",
          "updating": 0,
          "rssi": -40,
          "led_off": 0,
          "latitude": 0.0,
          "longitude": 0.0
        }
      },
      "emeter": {
        "get_realtime": {
          "current": 0.0,
          "voltage": 300.00,
          "power": 1.0,
          "total": 1.0,
          "err_code": 0
        }
      },
      "smartlife.iot.dimmer": {
        "err_code": -1,
        "err_msg": "module not support"
      },
      "smartlife.iot.common.emeter": {
        "err_code": -1,
        "err_msg": "module not support"
      },
      "smartlife.iot.smartbulb.lightingservice": {
        "err_code": -1,
        "err_msg": "module not support"
      }
    }"#;

    const LB110_JSON_OFF: &'static str = r#"{
      "system": {
        "get_sysinfo": {
          "sw_ver": "1.8.6 Build 180809 Rel.091659",
          "hw_ver": "1.0",
          "model": "LB110(EU)",
          "description": "Smart Wi-Fi LED Bulb with Dimmable Light",
          "alias": "Lamp",
          "mic_type": "IOT.SMARTBULB",
          "dev_state": "normal",
          "mic_mac": "000000000000",
          "deviceId": "0000000000000000000000000000000000000000",
          "oemId": "A68E15472071CB761E5CCFB388A1D8AE",
          "hwId": "00000000000000000000000000000000",
          "is_factory": false,
          "disco_ver": "1.0",
          "ctrl_protocols": {
            "name": "Linkie",
            "version": "1.0"
          },
          "light_state": {
            "on_off": 0,
            "dft_on_state": {
              "mode": "normal",
              "hue": 0,
              "saturation": 0,
              "color_temp": 2700,
              "brightness": 1
            }
          },
          "is_dimmable": 1,
          "is_color": 0,
          "is_variable_color_temp": 0,
          "preferred_state": [
            {
              "index": 0,
              "hue": 0,
              "saturation": 0,
              "color_temp": 2700,
              "brightness": 100
            },
            {
              "index": 1,
              "hue": 0,
              "saturation": 0,
              "color_temp": 2700,
              "brightness": 80
            },
            {
              "index": 2,
              "hue": 0,
              "saturation": 0,
              "color_temp": 2700,
              "brightness": 10
            },
            {
              "index": 3,
              "hue": 0,
              "saturation": 0,
              "color_temp": 2700,
              "brightness": 1
            }
          ],
          "rssi": -51,
          "active_mode": "none",
          "heapsize": 290056,
          "err_code": 0
        }
      },
      "emeter": {
        "err_code": -2001,
        "err_msg": "Module not support"
      },
      "smartlife.iot.dimmer": {
        "err_code": -2001,
        "err_msg": "Module not support"
      },
      "smartlife.iot.common.emeter": {
        "get_realtime": {
          "power_mw": 0,
          "err_code": 0
        }
      },
      "smartlife.iot.smartbulb.lightingservice": {
        "get_light_state": {
          "on_off": 0,
          "dft_on_state": {
            "mode": "normal",
            "hue": 0,
            "saturation": 0,
            "color_temp": 2700,
            "brightness": 1
          },
          "err_code": 0
        }
      }
    }"#;

    const LB110_JSON_ON: &'static str = r#"{
      "system": {
        "get_sysinfo": {
          "sw_ver": "1.8.6 Build 180809 Rel.091659",
          "hw_ver": "1.0",
          "model": "LB110(EU)",
          "description": "Smart Wi-Fi LED Bulb with Dimmable Light",
          "alias": "Lamp",
          "mic_type": "IOT.SMARTBULB",
          "dev_state": "normal",
          "mic_mac": "000000000000",
          "deviceId": "0000000000000000000000000000000000000000",
          "oemId": "A68E15472071CB761E5CCFB388A1D8AE",
          "hwId": "00000000000000000000000000000000",
          "is_factory": false,
          "disco_ver": "1.0",
          "ctrl_protocols": {
            "name": "Linkie",
            "version": "1.0"
          },
          "light_state": {
            "on_off": 1,
            "mode": "normal",
            "hue": 0,
            "saturation": 0,
            "color_temp": 2700,
            "brightness": 10
          },
          "is_dimmable": 1,
          "is_color": 0,
          "is_variable_color_temp": 0,
          "preferred_state": [
            {
              "index": 0,
              "hue": 0,
              "saturation": 0,
              "color_temp": 2700,
              "brightness": 100
            },
            {
              "index": 1,
              "hue": 0,
              "saturation": 0,
              "color_temp": 2700,
              "brightness": 80
            },
            {
              "index": 2,
              "hue": 0,
              "saturation": 0,
              "color_temp": 2700,
              "brightness": 10
            },
            {
              "index": 3,
              "hue": 0,
              "saturation": 0,
              "color_temp": 2700,
              "brightness": 1
            }
          ],
          "rssi": -48,
          "active_mode": "none",
          "heapsize": 290152,
          "err_code": 0
        }
      },
      "emeter": {
        "err_code": -2001,
        "err_msg": "Module not support"
      },
      "smartlife.iot.dimmer": {
        "err_code": -2001,
        "err_msg": "Module not support"
      },
      "smartlife.iot.common.emeter": {
        "get_realtime": {
          "power_mw": 1800,
          "err_code": 0
        }
      },
      "smartlife.iot.smartbulb.lightingservice": {
        "get_light_state": {
          "on_off": 1,
          "mode": "normal",
          "hue": 0,
          "saturation": 0,
          "color_temp": 2700,
          "brightness": 10,
          "err_code": 0
        }
      }
    }"#;

    #[test]
    fn deserialise_hs100() {
        let result = serde_json::from_str::<DeviceData>(&HS100_JSON).unwrap();

        let sysinfo = result.sysinfo();
        assert_eq!(sysinfo.hw_ver, "2.1");
        assert_eq!(sysinfo.model, "HS100(UK)");
    }

    #[test]
    fn deserialise_hs110() {
        let result = serde_json::from_str::<DeviceData>(&HS110_JSON).unwrap();

        let sysinfo = result.sysinfo();
        assert_eq!(sysinfo.hw_ver, "1.0");
        assert_eq!(sysinfo.model, "HS110(UK)");
    }

    #[test]
    fn deserialise_lb110_off() {
        let result = serde_json::from_str::<DeviceData>(&LB110_JSON_OFF).unwrap();

        let sysinfo = result.clone().sysinfo();
        assert_eq!(sysinfo.hw_ver, "1.0");
        assert_eq!(sysinfo.model, "LB110(EU)");
        assert_eq!(
            sysinfo
                .light_state
                .as_ref()
                .unwrap()
                .dft_on_state()
                .color_temp,
            2700
        );
        let smartlife = result.smartlife;
        assert_eq!(smartlife.emeter().unwrap().realtime.power_mw, 0);
        assert_eq!(
            smartlife
                .lightingservice()
                .unwrap()
                .light_state
                .dft_on_state()
                .color_temp,
            2700
        );
    }

    #[test]
    fn deserialise_lb110_on() {
        let result = serde_json::from_str::<DeviceData>(&LB110_JSON_ON).unwrap();

        let sysinfo = result.clone().sysinfo();
        assert_eq!(sysinfo.hw_ver, "1.0");
        assert_eq!(sysinfo.model, "LB110(EU)");
        assert_eq!(
            sysinfo
                .light_state
                .as_ref()
                .unwrap()
                .dft_on_state()
                .color_temp,
            2700
        );
        let smartlife = result.smartlife;
        assert_eq!(smartlife.emeter().unwrap().realtime.power_mw, 1800);
        assert_eq!(
            smartlife
                .lightingservice()
                .unwrap()
                .light_state
                .dft_on_state()
                .color_temp,
            2700
        );
    }
}
