# CIRNET Hardware version 0 changelog

[Changelog index](README.md)

## Unreleased

### Changed

- Standardize English major-version changelog navigation.

## v0.3.0-dev.1 — Development

This is a development version used in the Windows x64 Native Candidate workflow. The available Git tags and GitHub Releases provide no publication date for this version.

### Changed

- Co-locate private RPC field projections with the handlers and document the service, station and Simulator module ownership.
- Align repository references with the canonical CIRNET names.
- Migrate the native Rust service, private Preview definition and tests into conventional Cargo paths.
- Build against Contracts Next and verify the Simulator Next worker digest at the runtime boundary.
- Preserve local Hardware RPC 2.0, station lifecycle and protected Stop behavior.
- Reference CIRNET Lab-First for the station safety boundary.
- Run Windows component CI on master pushes with pinned private dependencies.
- Report the exact private dependency permissions required by Hardware CI.
- Verify plan and capture isolation, malformed NCAP evidence and worker digest rejection.
- Verify native Hardware RPC with the selected Contracts schema, checked-in reference replies and C++ worker.
- Documented a single owned verification session for Cargo, generated RPC bindings, fixtures and pytest output.

### Fixed

- Compile the stale-generation RPC test worker before pytest on Windows CI.

**Sources**

- [Version manifest](../../Cargo.toml)
- [Native implementation baseline](https://github.com/cirnetlab/cirnet-hardware/commit/2288cc44233b537752fb40d251e7b01d51004337)
- [Latest source milestone recorded here](https://github.com/cirnetlab/cirnet-hardware/commit/15056907c680989223daafc964a5500be5fa4eaa)
- [Earlier version index placeholder](https://github.com/cirnetlab/cirnet-hardware/blob/13a5888de6bc35a52f1117768483024e8a1b114a/docs/changelogs/CHANGELOG_V0.md)
