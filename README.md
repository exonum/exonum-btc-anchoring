# Exonum btc anchoring service &emsp; [![Build Status](https://travis-ci.com/exonum/exonum-btc-anchoring.svg?token=XsvDzZa3zu2eW4sVWuqN&branch=master)](https://travis-ci.com/exonum/exonum-btc-anchoring)

This crate implements a protocol for blockchain anchoring onto the `Bitcoin` blockchain that utilizes the native `Bitcoin` capabilities of creating multisig transactions.

## You may looking for:
* [Reference documentation](http://exonum.com/doc/crates/btc_anchoring_service/index.html)
* [Specification](http://exonum.com/doc/anchoring-spec/)
* [Implementation details](http://exonum.com/doc/anchoring-impl/)
* [Example code](examples/anchoring.rs)
* [Testnet tutorial](doc/tutorial.md)

# Usage
The anchoring service depends on bitcoind. For correct working of the service you need to launch bitcoind with specific configuration, see [tutorial](doc/tutorial.md) for details.

# Licence
Anchoring service licensed under [Apache License, Version 2.0](LICENSE).
