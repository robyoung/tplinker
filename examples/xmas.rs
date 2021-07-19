use std::{thread, time};
use tplinker::{
    capabilities::Light, datatypes::SetLightState, devices::Device, discovery::discover,
};

fn main() {
    let devices = discover()
        .unwrap()
        .iter()
        .filter_map(|(addr, data)| match Device::from_data(*addr, &data) {
            Device::LB110(device) => Some(device),
            _ => None,
        })
        .collect::<Vec<_>>();

    let mut index = 0;
    loop {
        let device = &devices[index];
        let second = time::Duration::from_secs(1);

        let _ = device.set_light_state(SetLightState {
            on_off: Some(1),
            brightness: Some(100),
            ..Default::default()
        });
        thread::sleep(second);
        let _ = device.set_light_state(SetLightState {
            on_off: Some(0),
            brightness: Some(0),
            ..Default::default()
        });

        index = index + 1;
        index = index % devices.len()
    }
}
