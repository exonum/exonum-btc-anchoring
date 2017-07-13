// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::ops::Drop;
use std::process::{Child, Command, Stdio};
use std::path::Path;
use std::fs;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::thread;
use std::time::Duration;

use bitcoinrpc;
use bitcoin::util::base58::ToBase58;
use tempdir::TempDir;
use rand;
use rand::Rng;

use details::rpc::{AnchoringRpc, AnchoringRpcConfig};
use details::btc::Address;

#[derive(Debug)]
pub struct RegTestNode {
    process: Child,
    client: AnchoringRpc,
}

impl RegTestNode {
    pub fn new<D, S>(dir: D, bind: S, rpc_port: u16) -> io::Result<RegTestNode>
    where
        D: AsRef<Path>,
        S: AsRef<str>,
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

        let process = Command::new("bitcoind")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg(format!("-datadir={}", dir.to_str().unwrap()))
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

    pub fn with_rand_ports<D: AsRef<Path>>(dir: D) -> io::Result<RegTestNode> {
        let mut rng = rand::thread_rng();
        let rpc_port = rng.gen();
        let listen_addr = format!("127.0.0.1:{}", rng.gen::<u16>());
        RegTestNode::new(dir, listen_addr, rpc_port)
    }

    pub fn generate_blocks(&self, n: u64) -> Result<Vec<String>, bitcoinrpc::Error> {
        self.client.generate(n, 99999)
    }

    pub fn generate_to_address(
        &self,
        n: u64,
        addr: &Address,
    ) -> Result<Vec<String>, bitcoinrpc::Error> {
        self.client.generatetoaddress(
            n,
            &addr.to_base58check(),
            99999,
        )
    }

    pub fn client(&self) -> &AnchoringRpc {
        &self.client
    }
}

impl Drop for RegTestNode {
    fn drop(&mut self) {
        trace!("stop regtest={:#?}", self.client.stop());
        thread::sleep(Duration::from_secs(3));
        trace!("kill regtest={:#?}", self.process.kill());
    }
}

pub fn temporary_regtest_node() -> io::Result<(TempDir, RegTestNode)> {
    let tmp_dir = TempDir::new("bitcoind").unwrap();
    let regtest_node = RegTestNode::with_rand_ports(&tmp_dir)?;
    Ok((tmp_dir, regtest_node))
}

#[cfg(test)]
mod tests {
    use exonum::helpers;

    use super::temporary_regtest_node;

    #[test]
    fn test_regtest_generate_blocks() {
        let _ = helpers::init_logger();

        let (_, regtest) = temporary_regtest_node().unwrap();
        regtest.generate_blocks(100).expect("Generate 100 blocks");
    }
}
