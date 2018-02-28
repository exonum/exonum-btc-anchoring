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

## Installation guide

Just follow the installation guide of the [`exonum`][exonum:install] to
install dependencies.

## Usage

The anchoring service depends on bitcoind. For the correct work, you need to
launch bitcoind with specific configuration, see [deployment guide](DEPLOY.md)
for details.

If you want to run rpc-tests, do the following:

* Install and configure `bitcoind`.
* Specify following environment variables.

  ```shell
  ANCHORING_RELAY_HOST=<bitcoind-rpc-listen-address>
  ANCHORING_USER=<rpc-user>
  ANCHORING_PASSWORD=<rpc-password>
  ```

* Enable feature `rpc_tests` in cargo.

Additional tests are situated in [tests](tests) subfolder.

## Licence

Exonum core library is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[travis:image]: https://travis-ci.org/exonum/exonum-btc-anchoring.svg?branch=master
[travis:url]: https://travis-ci.org/exonum/exonum-btc-anchoring
[Exonum]: https://github.com/exonum/exonum
[exonum:reference]: https://docs.rs/exonum-btc-anchoring
[anchoring:specification]: https://exonum.com/doc/advanced/bitcoin-anchoring/
[exonum:contribution]: https://exonum.com/doc/contributing/
[exonum:install]: https://exonum.com/doc/get-started/install/
