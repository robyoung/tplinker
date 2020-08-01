use std::time::Duration;

use tplinker::tokio::discovery;

#[tokio::main]
async fn main() {
    let duration = Duration::from_secs(2);
    discovery::with_timeout(duration)
        .await
        .unwrap()
        .into_iter()
        .for_each(|(addr, _)| {
            println!("{:?}", addr);
        });
        
}
