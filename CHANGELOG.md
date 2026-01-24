# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.7](https://github.com/mladedav/json-subscriber/compare/json-subscriber-v0.2.6...json-subscriber-v0.2.7) - 2026-01-24

### Added

- support tracing-opentelemetry 0.32 ([#30](https://github.com/mladedav/json-subscriber/pull/30))

### Other

- Extract opentelemetry ID retrieval code into a macro ([#27](https://github.com/mladedav/json-subscriber/pull/27))

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
