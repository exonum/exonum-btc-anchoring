# Exonum Anchoring Service to Bitcoin

[![Build Status][travis:image]][travis:url]

This crate implements a service for [Exonum] blockchain that provides
a protocol for anchoring onto the Bitcoin blockchain that utilizes the
native Bitcoin capabilities of creating multisig transactions.

* [Reference documentation][anchoring:reference]
* [Specification][anchoring:specification]
* [Example code](examples/btc_anchoring.rs)
* [Newbie guide](guides/newbie.md)
* [Maintenance guide](guides/maintenance.md)
* [Contribution guide][exonum:contribution]

## Licence

Exonum core library is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[anchoring:reference]: https://docs.rs/exonum-btc-anchoring
[anchoring:specification]: https://exonum.com/doc/version/latest/advanced/bitcoin-anchoring/
[exonum:contribution]: https://exonum.com/doc/contributing/
[exonum:install]: https://exonum.com/doc/get-started/install/
[Exonum]: https://github.com/exonum/exonum
[travis:image]: https://travis-ci.org/exonum/exonum-btc-anchoring.svg?branch=master
[travis:url]: https://travis-ci.org/exonum/exonum-btc-anchoring
