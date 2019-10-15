# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking changes

- Ported `exonum-btc-anchoring` to a new version of Exonum with support
  of dynamic services. (#145)

  In this way, a large number of changes have occurred in service business logic:

  - Nodes performing anchoring are no longer strictly associated with the
  validator nodes. It means that there may exist a validator node that does not
  perform anchoring, and vice versa an anchoring node that is not a validator.
  But we strongly recommend to keep one to one relationship between the
  anchoring and validator nodes.
  - The bootstrapping procedure has been completely changed in accordance with
  the fact that the service can be started at any time during the blockchain
  life. The implementation of the service has become stateless and all logic
  that previously was performed in the `after_commit` method was taken out
  in a separate `btc_anchoring_sync` utility.
  TODO Reference to the description in the README.md.
  - "v1" prefix has been removed from API endpoints and introduced a lot of
  private API endpoints for the `btc_anchoring_sync` util.
  - Removed cryptographic proofs for Exonum blocks feature.
  It will be implemented as separate service.
  - Funding transaction entry in config has been replaced by the `add-funds`
  endpoint in the anchoring private API. This means is that you no longer need
  to use a configuration update procedure in order to add a new funding
  transaction. Now for this there is a simpler voting procedure using
  the `add-funds` endpoint. A qualified majority of nodes (`2/3n+1`) just have
  to send the same transaction so that it is used as a funding one.

### Internal improvements

- `exonum_bitcoinrpc` crate has been replaced in favor of `bitcoincore-rpc`. (#145)

## 0.12.0 - 2018-08-14

- Updated to the [Exonum 0.12.0](https://github.com/exonum/exonum/releases/tag/v0.12)
  release (#144).

## 0.11.0 - 2018-03-15

### Internal improvements

- Updated to the `Rust-bitcoin 0.17` release. (#142)
- Crate has been updated to Rust 2018 edition. This means that it is required
  to use Rust 1.31 or newer for compilation. (#142)

## 0.10.0 - 2018-12-14

### Internal improvements

- Updated to the `Rust-bitcoin 0.14` release (#134).

### Breaking changes

- New anchoring implementation is based on native segwit transactions. Acceptance of an
  anchoring transaction still requires the majority of votes of the validators -
  `2/3n+1`. As the votes are now separated from the transaction body, they do not
  affect transaction hash and, correspondingly, its ID. (#136)

  In this way:

  - Anchoring transactions have become fully deterministic. The problem of transaction
    [malleability]( https://en.wikipedia.org/wiki/Malleability_(cryptography)) has,
    thus, been solved. The validators do not have to agree on the latest Exonum
    transaction anchored to Bitcoin blockchain any more to continue the anchoring
    chain.
  - The service can extract the anchoring chain or information on a particular
    anchoring transaction from its database any time by a simple API request. It does
    not need to use a separate observer functionality any more to extract information
    on the latest Exonum anchoring transaction from Bitcoin blockchain and rebuild the
    anchoring chain from of this transaction.
  - There is no need to connect each Exonum node to the `bitcoind`. The anchoring
    transactions are generated deterministically and independently from the connection
    to the Bitcoin blockchain. New anchoring transactions are monitored by a separate
    method and forwarded to the Bitcoin blockchain whenever they occur.

  For more details on the updated anchoring service operation you can visit the
  [readme](README.md) page.

## 0.9.0 - 2018-07-20

### Internal improvements

- Anchoring transaction in memory pool now is considering as transaction with the `Some(0)`
  confirmations instead of `Null`. (#133)

- Log level for "Insufficient funds" errors reduced from `error` to `trace`. (#133)

### Breaking changes

- The anchoring chain observer logic has been moved to the `before_commit` stage. (#131)

  Thus additional thread in the public api handler has been no longer used.
  Thus now `anchoring-observer-check-interval` is measured in blocks instead of milliseconds.

- The anchoring API has been ported to the new `actix-web` backend. (#132)

  Some of API endpoints have been changed, you can see updated API description in
  the [documentation](https://exonum.com/doc/advanced/bitcoin-anchoring/#available-api).

### Internal improvements

- Added check that funding transaction in `anchoring-funding-txid` contains
  output to the anchoring address. (#130)

## 0.8.1 - 2018-06-06

### Internal improvements

- Changed `btc::Network` (de)serializing into/from string (#128).

- Updated to the `Rust-bitcoin 0.13.1` release (#128).

## 0.8 - 2018-06-01

### Breaking changes

- The anchoring service has been switched to using p2wsh address format (#123).

  It now uses segwit addresses....
  This change increases the limit on the number of validators and anchoring security
  as well as reduces fees for applying thereof.

  Note that the old format of anchoring transactions is incompatible with the new one.
  Hence, update of the existing blockchain to the new anchoring version is not possible.
  For use of a new anchoring format a new blockchain has to be launched.

### New features

- Introduced a new API method `/v1/block_header_proof/:height` that provides cryptographic
  proofs for Exonum blocks including those anchored to Bitcoin blockchain.
  The proof is an apparent evidence of availability of a certain Exonum block
  in the blockchain (#124).

### Internal improvements

- Updated to the [Rust-bitcoin 0.13](https://github.com/rust-bitcoin/rust-bitcoin/releases/tag/0.13)
  release (#123).

### Fixed

- Fixed bug with the `nearest_lect` endpoint that sometimes didn't return actual data [ECR-1387] (#125).

## 0.7 - 2018-04-11

### Internal improvements

- Updated to the [Rust-bitcoin 0.12](https://github.com/rust-bitcoin/rust-bitcoin/releases/tag/0.12)
  release (#122).

- Updated to the [Exonum 0.7](https://github.com/exonum/exonum/releases/tag/v0.7)
  release (#122).

### Fixed

- Fixed an issue with the identifiers of funding transactions with the witness data [ECR-1220]
  (#122).

## 0.6.1 - 2018-03-22

### Fixed

- Fix txid for transactions with the witness data [ECR-986] (#119).

  Txid for transactions should be always computed without witness data.

### Internal improvements

- Implement `Display` for the wrapped bitcoin types (#119).

## 0.6 - 2018-03-06

### Breaking changes

- The `network` parameter became named (#114).

  Now, to generate template config, run the following command:

  ```shell
  anchoring generate-template ...
  --anchoring-network <Network in which anchoring should work (testnet\bitcoin)>
  ```

### Internal improvements

- Error types now use `failure` instead of `derive-error`,
  which makes error messages more human-readable (#115).

- Implemented error codes for incorrect anchoring messages (#117).

- Updated to the [Exonum 0.6.0](https://github.com/exonum/exonum/releases/tag/v0.6)
  release (#117).

## 0.5 - 2018-01-30

### Changed

- Update to the [Exonum 0.5.0](https://github.com/exonum/exonum/releases/tag/v0.5)
  release (#112).

## 0.4 - 2017-12-08

### Added

- Added tests written on `exonum-testkit` (#101).

### Changed

- Update to the [Exonum 0.4.0](https://github.com/exonum/exonum/releases/tag/v0.4)
  release (#104).

### Removed

- Sandbox tests are removed (#101).

## 0.3.0 - 2017-11-03

- Update to the [Exonum 0.3.0](https://github.com/exonum/exonum/releases/tag/v0.3)
  release (#93).

## 0.2.1 - 2017-10-13

### Fixed

- Do not emit panic if lect does not found in bitcoin blockchain (#88).

## 0.2 - 2017-09-14

### Added

- Add `anchoring-observer-check-interval` to clap fabric (#85).

### Changed

- Run rpc tests only if the `rpc_tests` feature enabled (#84).
- Update anchoring chain observer configuration layout (#85).

### Fixed

- Fix typo in documentation (#83).

## 0.1 - 2017-07-17

The first release of Exonum btc anchoring service.
