# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

## 0.5 - 2018-01-30

### Changed
- Update to the [Exonum 0.5.0](https://github.com/exonum/exonum/releases/tag/v0.5) release (#112).

## 0.4 - 2017-12-08

### Added
- Added tests written on `exonum-testkit` (#101).

### Changed
- Update to the [Exonum 0.4.0](https://github.com/exonum/exonum/releases/tag/v0.4) release (#104).

### Removed
- Sandbox tests are removed (#101).

## 0.3.0 - 2017-11-03

- Update to the [Exonum 0.3.0](https://github.com/exonum/exonum/releases/tag/v0.3) release (#93).

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