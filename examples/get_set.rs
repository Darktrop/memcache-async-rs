use memcache_async_rs::*;
use memcache_async_rs::client::*;
use tokio::prelude::*;
#[macro_use] extern crate log;
use env_logger::Env;

#[tokio::main]
async fn main() {
    env_logger::from_env(Env::default().default_filter_or("debug")).init();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut transport = MemcacheTransport::new(rt.executor());
    let mut client = transport.connect(String::from("127.0.0.1:11211")).await.expect("not connected");
    info!("connected");
    let mut client2 = client.clone();
    let result = client.get::<String>(String::from("some_key"));
    let result2 = client2.get::<String>(String::from("some_key"));
    dbg!(result.await);
    dbg!(result2.await);
}