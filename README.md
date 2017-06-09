# Exonum btc anchoring service &emsp; [![Build Status](https://travis-ci.com/exonum/exonum-btc-anchoring.svg?token=XsvDzZa3zu2eW4sVWuqN&branch=master)](https://travis-ci.com/exonum/exonum-btc-anchoring)

This crate implements a protocol for blockchain anchoring onto the `Bitcoin` blockchain that utilizes the native `Bitcoin` capabilities of creating multisig transactions.

## You may looking for:
* [Reference documentation](http://exonum.com/doc/crates/anchoring_btc_service/index.html)
* [Specification](http://exonum.com/doc/anchoring-spec/)
* [Implementation details](http://exonum.com/doc/anchoring-impl/)
* [Example code](examples/anchoring.rs)
* [Deployment guide](DEPLOY.md)
* [Contribution guide](https://github.com/exonum/exonum-core/blob/master/CONTRIBUTING.md)

# Installation guide

Just follow the installation guide of the 
[`exonum-core`](https://github.com/exonum/exonum-core/blob/master/INSTALL.md) to install dependencies.

# Usage
The anchoring service depends on bitcoind. For the correct work, you need to launch bitcoind with specific configuration, see [deployment guide](DEPLOY.md) for details.

To run tests you need to install `bitcoind` and specify following enviroment variables.
```shell
ANCHORING_RELAY_HOST=<bitcoind-rpc-listen-address>
ANCHORING_USER=<rpc-user>
ANCHORING_PASSWORD=<rpc-password>
```
Additional tests are situated in `sandbox_tests` subfolder.

# Licence
Anchoring service licensed under [Apache License, Version 2.0](LICENSE).
