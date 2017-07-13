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

use exonum_btc_anchoring::details::sandbox::{Request, SandboxClient};
use exonum_btc_anchoring::AnchoringRpc;

use {AnchoringSandbox, gen_sandbox_anchoring_config};

#[test]
fn test_rpc_getnewaddress() {
    let client = SandboxClient::default();
    client.expect(vec![
        request! {
            method: "getnewaddress",
            params: ["maintain"],
            response: "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY"
        },
    ]);
    let addr = client.getnewaddress("maintain").unwrap();
    assert_eq!(addr, "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY");
}

#[test]
#[should_panic(expected = "expected response for method=getnewaddress")]
fn test_rpc_expected_request() {
    let client = SandboxClient::default();
    client.getnewaddress("useroid").unwrap();
}

#[test]
#[should_panic(expected = "assertion failed")]
fn test_rpc_wrong_request() {
    let client = SandboxClient::default();
    client.expect(vec![
        request! {
            method: "getnewaddress",
            params: ["maintain"],
            response: "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY"
        },
    ]);
    client.getnewaddress("useroid").unwrap();
}

#[test]
#[should_panic(expected = "assertion failed")]
fn test_rpc_uneexpected_request() {
    let client = SandboxClient::default();
    client.expect(vec![
        request! {
            method: "getnewaddress",
            params: ["maintain"],
            response: "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY"
        },
        request! {
            method: "getnewaddress",
            params: ["maintain2"],
            response: "mmoXxKhBwnhtFiAMvxJ82CKCBia751mzfY"
        },
    ]);
    client.getnewaddress("useroid").unwrap();
    client.expect(vec![
        request! {
            method: "getnewaddress",
            params: ["maintain"],
            response: "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY"
        },
    ]);
}

#[test]
fn test_rpc_validateaddress() {
    let client = SandboxClient::default();
    client.expect(vec![
        request! {
            method: "validateaddress",
            params: ["n2cCRtaXxRAbmWYhH9sZUBBwqZc8mMV8tb"],
            response: {
                "account": "node_0",
                "address": "n2cCRtaXxRAbmWYhH9sZUBBwqZc8mMV8tb",
                "hdkeypath": "m/0'/0'/1023'",
                "hdmasterkeyid": "e2aabb596d105e11c1838c0b6bede91e1f2a95ee",
                "iscompressed": true,
                "ismine": true,
                "isscript": false,
                "isvalid": true,
                "iswatchonly": false,
                "pubkey": "0394a06ac465776c110cb43d530663d7e7df5684013075988917f02f\
                            f007edd364",
                "scriptPubKey": "76a914e7588549f0c4149e7949cd7ea933cfcdde45f8c888ac"
            }
        },
    ]);
    client
        .validateaddress("n2cCRtaXxRAbmWYhH9sZUBBwqZc8mMV8tb")
        .unwrap();
}

#[test]
fn test_generate_anchoring_config() {
    let mut client = AnchoringRpc(SandboxClient::default());
    gen_sandbox_anchoring_config(&mut client);
}

#[test]
fn test_anchoring_sandbox() {
    let _ = AnchoringSandbox::initialize(&[]);
}
