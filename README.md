# Exonum Anchoring Service to Bitcoin

[![Build Status][travis:image]][travis:url]

This crate implements a service for [Exonum] blockchain that provides
a protocol for anchoring onto the Bitcoin blockchain that utilizes the
native Bitcoin capabilities of creating multisig transactions.

* [Reference documentation][exonum:reference]
* [Specification][anchoring:specification]
* [Example code](examples/anchoring.rs)
* [Deployment guide](DEPLOY.md)
* [Contribution guide][exonum:contribution]

## Prerequisites

### Installation

Just follow the installation guide of the [`exonum`][exonum:install] to
install dependencies.

### Bitcoin node deployment

First of all install `bitcoind` via your package manager and ensure that you
use the latest stable version. You may visit official bitcoin [site][bitcoin:install]
for more information about installation.

Then create bitcoind configuration file in according to this [tutorial][bitcoin_wiki:configuration].

For correct work of the service, the `bitcoind` configuration file
should contain the following settings:

```ini
# Run on the test network instead of the real bitcoin network.
# If you want to use main network comment line bellow:
testnet=1
# server=1 tells Bitcoin-Qt and bitcoind to accept JSON-RPC commands.
server=1
# Maintain a full transaction index, used by the getrawtransaction rpc call.
# txindex=1

# Bind to given address to listen for JSON-RPC connections.
# Use [host]:port notation for IPv6.
# This option can be specified multiple times (default: bind to all interfaces)
#rpcbind=<addr>
# You must specify rpcuser and rpcpassword to secure the JSON-RPC API
#rpcuser=<username>
#rpcpassword=YourSuperGreatPasswordNumber_DO_NOT_USE_THIS_OR_YOU_WILL_GET_ROBBED_385593
```

These rpc settings will be used by the service.

After creating configuration file, launch `bitcoind` daemon via command:

```shell
bitcoind --daemon
```

Downloading and indexing of the bitcoin blockchain may take a lot of time,
especially for the mainnet.

## Usage

Include `exonum-btc-anchoring` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum-btc-oracle = "*"
```

Add the BTC anchoring service to the blockchain in the main project file:

```rust
extern crate exonum;
extern crate exonum_btc_anchoring as anchoring;

use exonum::helpers;
use exonum::helpers::fabric::NodeBuilder;

fn main() {
    exonum::crypto::init();
    helpers::init_logger().unwrap();
    let node = NodeBuilder::new()
        .with_service(Box::new(anchoring::ServiceFactory));
    node.run();
}

```

## Configuration parameters

### For the `generate-template` subcommand

* `btc-anchoring-network` - bitcoin network type used for downloading Bitcoin blocks headers. 
  
  Possible values: [mainnet, testnet, regtest]

* `btc-anchoring-interval` - interval in blocks between anchored blocks.
* `btc-anchoring-fee` - transaction fee per byte in satoshi that anchoring nodes should use.
* `btc-anchoring-utxo-confirmations` - the minimum number of confirmations for the first funding transactions.

### For the `generate-config` subcommand

* `btc-anchoring-rpc-host` - Bitcoin rpc url.
* `btc-anchoring-rpc-user` - User to login into bitcoind.
* `btc-anchoring-rpc-password` - Password to login into bitcoind.

### For the `finalize` subcommand

* `btc-anchoring-create-funding-tx` - if this option is set, node will create an initial funding 
  transaction with the given amount in satoshis and return it identifier.
* `btc-anchoring-funding-txid` - Identifier of the initial funding transaction which was created 
  previously using the option above. 

### For adjusting the running blockchain configuration

Variables that you can modify

* `transaction_fee` - the amount of the fee per byte in satoshis for the anchoring transactions.
* `anchoring_interval` - the interval in blocks between anchored blocks.
* `funding_transaction` - the hex representation of current funding transaction, 
  node would use it as input if it did not spent.
* `public_keys` - the list of hex-encoded compressed bitcoin public keys of
  exonum validators that collects into the current anchoring address.

***The option is not to be used for changing the applied Bitcoin network, otherwise the service will come to a halt.***

## Deployment

## Maintaince

### Add funds

### Modify list of validators 

## Licence

Exonum core library is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[bitcoin:install]: https://bitcoin.org/en/full-node#what-is-a-full-node
[bitcoin:faucet]: https://testnet.manu.backend.hamburg/faucet
[bitcoin:base58check]: https://en.bitcoin.it/wiki/Base58Check_encoding
[bitcoin:wif]: https://en.bitcoin.it/wiki/Wallet_import_format
[bitcoin_wiki:configuration]: https://en.bitcoin.it/wiki/Running_Bitcoin#Bitcoin.conf_Configuration_File
[travis:image]: https://travis-ci.org/exonum/exonum-btc-anchoring.svg?branch=master
[travis:url]: https://travis-ci.org/exonum/exonum-btc-anchoring
[Exonum]: https://github.com/exonum/exonum
[exonum:reference]: https://docs.rs/exonum-btc-anchoring
[anchoring:specification]: https://exonum.com/doc/advanced/bitcoin-anchoring/
[exonum:contribution]: https://exonum.com/doc/contributing/
[exonum:install]: https://exonum.com/doc/get-started/install/