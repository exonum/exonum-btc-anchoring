# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

 - Changed `btc::Network` serializing into string 

## 0.8 - 2018-06-01

### Breaking changes

- The anchoring service has been switched to using p2wsh address format. (#123)
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
  in the blockchain. (#124)

### Internal improvements

- Updated to the [Rust-bitcoin 0.13](https://github.com/rust-bitcoin/rust-bitcoin/releases/tag/0.13)
  release (#123).

### Fixed

- Fixed bug with the `nearest_lect` endpoint that sometimes didn't return actual data [ECR-1387]. (#125)

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

- Fix txid for transactions with the witness data [ECR-986]. (#119)
  Txid for transactions should be always computed without witness data.

### Internal improvements

- Implement `Display` for the wrapped bitcoin types. (#119)

## 0.6 - 2018-03-06

### Breaking changes

- The `network` parameter became named. (#114)
  Now, to generate template config, run the following command:

  ```shell
  anchoring generate-template ...
  --anchoring-network <Network in which anchoring should work (testnet\bitcoin)>
  ```

### Internal improvements

- Error types now use `failure` instead of `derive-error`,
  which makes error messages more human-readable. (#115)

- Implemented error codes for incorrect anchoring messages. (#117)

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

- Do not emit panic if lect does not found in bitcoin blockchain. (#88)

## 0.2 - 2017-09-14

### Added

- Add `anchoring-observer-check-interval` to clap fabric (#85)

### Changed

- Run rpc tests only if the `rpc_tests` feature enabled. (#84)
- Update anchoring chain observer configuration layout. (#85)

### Fixed

- Fix typo in documentation (#83)

## 0.1 - 2017-07-17

The first release of Exonum btc anchoring service.
