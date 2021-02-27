use tplinker::{devices::Device, discovery::discover};

fn main() {
    for (addr, data) in discover().unwrap() {
        let _ = Device::from_data(addr, &data);
        let sysinfo = data.sysinfo();
        println!("{}\t{}\t{}", addr, sysinfo.alias, sysinfo.hw_type);
    }
}
