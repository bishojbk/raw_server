use std::time::Duration;
use tokio::time::sleep;

async fn slow_task(name: &str) {
    println!("{} starting", name);
    sleep(Duration::from_secs(2)).await;
    println!("{} done", name);
}

#[tokio::main]
async fn main() {
    let a = slow_task("A");
    let b = slow_task("B");
    let c = slow_task("C");
    
}