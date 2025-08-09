# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.7-alpha.2](https://github.com/mladedav/json-subscriber/compare/json-subscriber-v0.2.7-alpha.1...json-subscriber-v0.2.7-alpha.2) - 2025-08-09

### Other

- add automatic benchmark runs ([#29](https://github.com/mladedav/json-subscriber/pull/29))
- Extract opentelemetry ID retrieval code into a macro ([#27](https://github.com/mladedav/json-subscriber/pull/27))

### Other

- use trusted publishing to crates.io

## [0.2.6](https://github.com/mladedav/json-subscriber/compare/json-subscriber-v0.2.5...json-subscriber-v0.2.6) - 2025-07-01

### Fixed

- enable needed opentelemetry feature
- correctly turn on optional opentelemetry dependencies

## [0.2.5](https://github.com/mladedav/json-subscriber/compare/json-subscriber-v0.2.4...json-subscriber-v0.2.5) - 2025-06-02

### Added

- allow for multiple opentelemetry dependencies

### Fixed

- do not serialize flattened empty objects

### Other

- bump MSRV to 1.75
