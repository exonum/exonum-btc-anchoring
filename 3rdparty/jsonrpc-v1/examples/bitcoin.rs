extern crate jsonrpc_v1;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

use std::env;

use jsonrpc_v1::client::Client as RpcClient;

#[derive(Deserialize, Debug)]
struct MiningInfo {
    blocks: u64,
    currentblocksize: u64,
    currentblockweight: u64,
    currentblocktx: u64,
    difficulty: f64,
    errors: String,
    networkhashps: f64,
    pooledtx: u64,
    testnet: bool,
    chain: String,
}

fn main() {
    let mut args = env::args();
    let url = args.nth(1).unwrap_or("http://localhost:18332".into());
    let user = args.next();
    let passwd = args.next();

    println!("Send request to: {:?} user={:?}, passwd={:?}",
             url,
             user,
             passwd);
    let client = RpcClient::new(url, user, passwd);

    let method_name = String::from("getmininginfo");
    let request = client.build_request(method_name, vec![]);

    match client.send_request(&request).and_then(|res| res.into_result::<MiningInfo>()) {
        Ok(mining_info) => {
            println!("mining_info: {:?}", mining_info);
            return;
        }
        Err(e) => panic!("error: {}", e),
    }
}
