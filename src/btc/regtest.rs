use std::ops::Drop;
use std::process::{Command, Child};
use std::path::Path;
use std::fs;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::thread;
use std::time::Duration;

use bitcoinrpc;
use bitcoin::util::base58::ToBase58;

use client::AnchoringRpc;
use config::AnchoringRpcConfig;
use btc::Address;

#[derive(Debug)]
pub struct RegTestNode {
    process: Child,
    client: AnchoringRpc,
}

impl RegTestNode {
    pub fn new<D, S>(dir: D, bind: S, rpc_port: u16) -> io::Result<RegTestNode>
        where D: AsRef<Path>,
              S: AsRef<str>
    {
        let dir = dir.as_ref();
        // create config dir
        fs::create_dir_all(dir)?;
        // write configuration
        let mut file = File::create(dir.join("bitcoin.conf"))?;
        writeln!(file, "regtest=1")?;
        writeln!(file, "txindex=1")?;
        writeln!(file, "server=1")?;
        writeln!(file, "bind={}", bind.as_ref())?;
        writeln!(file, "rpcport={}", rpc_port)?;
        writeln!(file, "rpcuser=regtest")?;
        writeln!(file, "rpcpassword=regtest")?;

        let rpc = AnchoringRpcConfig {
            host: format!("http://127.0.0.1:{}", rpc_port),
            username: Some("regtest".to_string()),
            password: Some("regtest".to_string()),
        };

        let process = Command::new("bitcoind").arg(format!("-datadir={}", dir.to_str().unwrap()))
            .spawn()?;
        let client = AnchoringRpc::new(rpc);
        // Wait for bitcoind node
        for _ in 0..30 {
            thread::sleep(Duration::from_secs(2));
            if client.getinfo().is_ok() {
                return Ok(RegTestNode {
                    process: process,
                    client: client,
                });
            }
        }
        Err(io::ErrorKind::TimedOut.into())
    }

    pub fn generate_blocks(&self, n: u64) -> Result<Vec<String>, bitcoinrpc::Error> {
        self.client.generate(n, 99999)
    }

    pub fn generate_to_address(&self,
                               n: u64,
                               addr: &Address)
                               -> Result<Vec<String>, bitcoinrpc::Error> {
        self.client.generatetoaddress(n, &addr.to_base58check(), 99999)
    }

    pub fn client(&self) -> &AnchoringRpc {
        &self.client
    }
}

impl Drop for RegTestNode {
    fn drop(&mut self) {
        trace!("stop regtest={:#?}", self.client.stop());
        trace!("kill regtest={:#?}", self.process.kill());
        trace!("wait regtest={:#?}", self.process.wait());
        trace!("stderr regtest={:#?}", self.process.stderr);
        trace!("stdout regtest={:#?}", self.process.stdout);
    }
}

#[cfg(test)]
mod tests {
    extern crate blockchain_explorer;

    use tempdir::TempDir;

    use super::RegTestNode;

    #[test]
    fn test_regtest_generate_blocks() {
        let _ = blockchain_explorer::helpers::init_logger();

        let tmp_dir = TempDir::new("bitcoind").unwrap();
        let regtest = RegTestNode::new(&tmp_dir, "127.0.0.1:20000", 16000).unwrap();
        regtest.generate_blocks(100).expect("Generate 100 blocks");
    }
}