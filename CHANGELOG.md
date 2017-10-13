# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

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