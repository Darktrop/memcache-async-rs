use memcache_async_rs::*;
use memcache_async_rs::client::*;
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
    let result2 = client2.get::<String>(String::from("some_key2"));
    dbg!(result.await);
    dbg!(result2.await);
    let mut client3 = client.clone();
    let result3 = client3.set::<String>(String::from("some_key2"), String::from("some_value2"), 3600);
    dbg!(result3.await);

    let mut client4 = client.clone();
    let result4 = client4.get::<String>(String::from("some_key2"));
    dbg!(result4.await);
}