use crate::{
    datatypes::{DeviceData, SysInfo},
    error::{Error, Result},
    protocol::{DefaultProtocol, Protocol},
};
use std::{net::SocketAddr, time::Duration};

// CAPABILITIES

pub trait Device {
    /// Send a message to a device and return its parsed response
    fn submit(&self, msg: &str) -> Result<DeviceData>;

    fn capabilities(&self) -> Vec<String> {
        vec![String::from("Device")]
    }

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

pub trait DeviceSwitch: Device {
    fn is_on(&self) -> Result<bool>;

    fn is_off(&self) -> Result<bool> {
        Ok(!self.is_on()?)
    }

    fn switch_on(&self) -> Result<()>;
    fn switch_off(&self) -> Result<()>;

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

// DEVICES

struct RawDevice {
    ip: SocketAddr,
    protocol: Box<dyn Protocol>,
}

impl RawDevice {
    pub fn new(ip: SocketAddr) -> RawDevice {
        RawDevice {
            ip,
            protocol: Box::new(DefaultProtocol::new()),
        }
    }
}

impl Device for RawDevice {
    fn submit(&self, msg: &str) -> Result<DeviceData> {
        Ok(serde_json::from_str::<DeviceData>(
            &self.protocol.send(self.ip, msg)?,
        )?)
    }
}

// TODO: should it be HS110 and HS100 or simply SmartPlug like in pyhs100?
pub struct HS100 {
    raw: RawDevice,
}

impl HS100 {
    pub fn new(ip: SocketAddr) -> HS100 {
        HS100 {
            raw: RawDevice::new(ip),
        }
    }
}

impl Device for HS100 {
    fn submit(&self, msg: &str) -> Result<DeviceData> {
        self.raw.submit(msg)
    }
}

impl DeviceSwitch for HS100 {
    fn is_on(&self) -> Result<bool> {
        if let Some(relay_state) = self.sysinfo()?.relay_state {
            Ok(relay_state > 0)
        } else {
            Err(Error::Other(String::from("No relay state")))
        }
    }

    fn switch_off(&self) -> Result<()> {
        // TODO: investigate a command helper
        let command = r#"{{"system":{{"set_relay_state":{{"state": 0}}}}}}"#;
        self.submit(&command)?;
        Ok(())
    }

    fn switch_on(&self) -> Result<()> {
        // TODO: investigate a command helper
        let command = r#"{{"system":{{"set_relay_state":{{"state": 1}}}}}}"#;
        self.submit(&command)?;
        Ok(())
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
            ip: "0.0.0.0:9999".parse().unwrap(),
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
            ip: "0.0.0.0:9999".parse().unwrap(),
            protocol: Box::new(protocol),
        };

        assert!(device.submit("{}").is_err());
    }

    #[test]
    fn test_raw_device_location() {
        let protocol = ProtocolMock::new();
        protocol.set_send_return_value(Ok(String::from(HS100_JSON)));
        let device = RawDevice {
            ip: "0.0.0.0:9999".parse().unwrap(),
            protocol: Box::new(protocol),
        };

        assert_eq!((3456.0, 123.0), device.location().unwrap());
    }
}
