# Exonum Anchoring Service to Bitcoin

[![Build Status](https://travis-ci.org/exonum/exonum-btc-anchoring.svg?token=XsvDzZa3zu2eW4sVWuqN&branch=master)](https://travis-ci.org/exonum/exonum-btc-anchoring)

This crate implements a service for [Exonum] blockchain that provides a protocol for anchoring onto
the Bitcoin blockchain that utilizes the native Bitcoin capabilities of creating multisig
transactions.

* [Reference documentation][exonum:reference]
* [Specification][anchoring:specification]
* [Example code](examples/anchoring.rs)
* [Deployment guide](DEPLOY.md)
* [Contribution guide][exonum:contribution]

## Installation guide

Just follow the installation guide of the [`exonum`][exonum:install] to install dependencies.

## Usage

The anchoring service depends on bitcoind. For the correct work, you need to launch bitcoind with
specific configuration, see [deployment guide](DEPLOY.md) for details.

To run tests you need to install `bitcoind` and specify following enviroment variables.
```shell
ANCHORING_RELAY_HOST=<bitcoind-rpc-listen-address>
ANCHORING_USER=<rpc-user>
ANCHORING_PASSWORD=<rpc-password>
```
Additional tests are situated in `sandbox_tests` subfolder.

## Licence

Exonum core library is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[Exonum]: https://github.com/exonum/exonum
[exonum:reference]: https://docs.rs/exonum-btc-anchoring
[anchoring:specification]: https://exonum.com/doc/advanced/bitcoin-anchoring/
[exonum:contribution]: https://exonum.com/doc/contributing/
[exonum:install]: https://exonum.com/doc/get-started/install/
