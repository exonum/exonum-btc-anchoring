# Exonum btc anchoring service &emsp; [![Build Status](https://travis-ci.com/exonum/exonum-btc-anchoring.svg?token=XsvDzZa3zu2eW4sVWuqN&branch=master)](https://travis-ci.com/exonum/exonum-btc-anchoring)

This crate implements a protocol for blockchain anchoring onto the `Bitcoin` blockchain that utilizes the native `Bitcoin` capabilities of creating multisig transactions.

## You may looking for:
* [Reference documentation](http://exonum.com/doc/crates/anchoring_btc_service/index.html)
* [Specification](http://exonum.com/doc/anchoring-spec/)
* [Implementation details](http://exonum.com/doc/anchoring-impl/)
* [Example code](examples/anchoring.rs)

# Usage
The anchoring service depends on bitcoind. For correct working of the service you need to launch bitcoind with specific configuration.

## Bitcoin full node deploy
### Configuration
Here the sample bitcoin.conf file.
```ini
# Run on the test network instead of the real bitcoin network.
testnet=1
# server=1 tells Bitcoin-Qt and bitcoind to accept JSON-RPC commands
server=1
# Maintain a full transaction index, used by the getrawtransaction rpc call
txindex=1

# Bind to given address to listen for JSON-RPC connections. Use [host]:port notation for IPv6.
# This option can be specified multiple times (default: bind to all interfaces)
#rpcbind=<addr>

# You must set rpcuser and rpcpassword to secure the JSON-RPC api
#rpcuser=<username>
#rpcpassword=YourSuperGreatPasswordNumber_DO_NOT_USE_THIS_OR_YOU_WILL_GET_ROBBED_385593
```
Detailed documentation you can find [here](https://en.bitcoin.it/wiki/Running_Bitcoin#Bitcoin.conf_Configuration_File).

### Launching
You can start the node with the command
```
$ bitcoind --reindex --daemon
```
*note 1: Be sure to wait for the full load of the bitcoin blockchain.*

*note 2: If node deploy for exists configuration be sure that current anchoring address is imported by `importaddress` rpc call.*

## Anchoring testnet example
For quick anchoring demonstration you can install built-in anchoring example.
```
$ cargo install --example anchoring
```

### Generate testnet config
After installation you need to generate testnet configuration
```
$ anchoring generate \
    --output-dir <destdir> <n> \
    --anchoring-host <bitcoin full node host> \
    --anchoring-user <username> \
    --anchoring-password <password> \
    --anchoring-funds <initial funds>
```
Which create the configuration of N nodes using given `bitcoind`.

*warning! It is important that the full node have some bitcoin amount greater  than `<initial_finds>, since the initial funding transaction will create during the testnet generation.*

### Launching testnet
You need to launch the whole testnet nodes. 
The command to launch 'm' node look such this:
```
$ anchoring run --node-config <destdir>/<m>.toml --leveldb-path <destdir>/db/<m>
```
In addition you may to set http port for configuration update service. More information you can find by invoke `anchoring help`

**Important warning! Do not use this example in production. Secret keys are stored in the single directory on the single machine and can be stolen.*

## Usage in your blockchain
See [example](http://exonum.com/doc/crates/anchoring_btc_service/index.html#examples) in a reference documentation.

# Next steps
You can learn the reference [documentation](http://exonum.com/doc/crates/anchoring_btc_service/index.html) or full [specification](http://exonum.com/doc/anchoring-spec).

# Licence
Anchoring service licensed under [Apache License, Version 2.0](https://github.com/serde-rs/serde/blob/master/LICENSE-APACHE).
