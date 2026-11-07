# CIRNET Hardware changelog for major version 0

[Changelog index](README.md) · [English history](../en/history/index.md) ·
[Lịch sử tiếng Việt](../vi/history/index.md)

## Contents

- [Unreleased](#unreleased)
- [v0.1.2](#v012)
- [v0.1.1](#v011)

## Unreleased

### Added

- Added an opt-in path for Python Hardware to supervise the C++ Simulator
  worker. Stop and reset fence stale results and wait for worker cleanup before
  acknowledging safe release. [Lifecycle supervision source](https://github.com/cirnetlab/cirnet-hardware/commit/c41027d35788b861392851ab523d995670c3fbaa)
- Added the separate virtual-only Rust Hardware candidate with both RPC 2.0
  service names. The r8-R2 combination passed Gate 3 functionality,
  compatibility and safety on Windows x64. Linux/Xeon performance, physical
  HIL and production/release acceptance remain `NOT RUN`. [Rust source](https://github.com/cirnetlab/cirnet-hardware/commit/3ad140f24ae5f3f2e6b735b5cff4beec7bffda54) · [Gate 3 record](../en/history/stage-3-windows-verification.md)

### Changed

- Moved canonical parser and Protobuf tests to Contracts. Hardware retains its
  installed package and runtime-boundary checks. [Test ownership change](https://github.com/cirnetlab/cirnet-hardware/commit/65dbf0a55d29026817ddf11458259ce312e94bc6)
- Organized the release changelog by major and documented how to maintain
  paired guidance and preserve historical records. [Documentation conventions](../en/contributing/writing-documentation.md)

## v0.1.2

Published **2026-11-05** at `15:59:16 +07:00`. Lightweight tag target
`9a8935b` at `15:57:37 +07:00`.
[GitHub release](https://github.com/cirnetlab/cirnet-hardware/releases/tag/v0.1.2) · [Tagged source](https://github.com/cirnetlab/cirnet-hardware/tree/9a8935b6a596abe2d21ce90416ba3b914e9f36be) · [System selection](https://github.com/cirnetlab/cirnet-remote/blob/38aea6120920f48cea817827ac2277cce4bef25e/integration/release-set.json)

### Changed

- Published `cirnet-hardware-service==0.1.2` with exact
  `cirnet-contracts==0.1.2` and `cirnet-simulator==0.1.2` dependencies. The
  `hardware_service` namespace remains stable. [Tagged package metadata](https://github.com/cirnetlab/cirnet-hardware/blob/9a8935b6a596abe2d21ce90416ba3b914e9f36be/pyproject.toml)
- Added the canonical `cirnet.hardware.v2.HardwareService` route and
  `CIRNET_*` settings. Legacy route and settings remain supported, and
  canonical settings take precedence. [Compatibility change](https://github.com/cirnetlab/cirnet-hardware/commit/61f02a482d22bb005fc21c79272505095b8ead18)
- Moved CI to Node.js 26 and Python 3.14. The package still supports
  Python 3.11 or later. [Verification record](../en/history/verification-ledger.md#2026-11-05-python-314-compatibility)

## v0.1.1

Published **2026-10-28** at `22:51:32 +07:00`. Annotated tag
`bd8f3ae` at `22:50:21 +07:00`, targeting `de65db8` from
`2026-10-27 at 23:46:35 +07:00`.
[GitHub release](https://github.com/cirnetlab/cirnet-hardware/releases/tag/v0.1.1) · [Tagged source](https://github.com/cirnetlab/cirnet-hardware/tree/de65db8b89d0dfa6dd4c430a41397555bc77111d) · [System selection](https://github.com/cirnetlab/cirnet-remote/blob/065179ead7c7cf4f44483b74edca99bb84f8f2a5/integration/release-set.json)

### Added

- Published the first immutable Hardware wheel and source distribution as
  `netcircuit-hardware-service==0.1.1`. The independent service provides
  local-only RPC 2.0, safety-gated stations and virtual execution with the
  exact 0.1.1 Contracts and Simulator wheels. [Tagged README](https://github.com/cirnetlab/cirnet-hardware/blob/de65db8b89d0dfa6dd4c430a41397555bc77111d/README.md) · [M4 extraction record](../en/history/2026-10-27-m4-verification.md)
