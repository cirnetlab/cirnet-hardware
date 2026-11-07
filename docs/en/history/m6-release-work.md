# CIRNET repository rename verification

Date: 2026-10-29, Asia/Saigon. Current repository is [CIRNETLAB/CIRNET-Hardware](https://github.com/CIRNETLAB/CIRNET-Hardware). Local checkout names remain unchanged. README authority links retain their original source revisions; package metadata, repository ownership instructions and the workflow display name now use CIRNET. Python/npm identifiers, exact dependency versions, environment names, RPC/NCAP, all nine runtime modules and LICENSE are unchanged. Legacy runtime wording remains part of its preserved source provenance.

Local verification used fresh external tracked-file exports of uncommitted changes over `de65db8b89d0dfa6dd4c430a41397555bc77111d`, Windows/Python 3.12.12/Node 24.20.0. Export fingerprint before adding this record: `b98ca3563ceb2b66ff51fe197715b8652a6ef66cf9f80c74cf7f6435040a89f6`. This is not evidence of a committed clean checkout or hosted CI for the eventual rename commit. The README release instruction was clarified after this candidate build; the final committed-source build must verify that later documentation input as well.

| Gate | Result |
| --- | --- |
| Direct final-name anonymous downloads | PASS: exact released Contracts/DEV/Simulator/Hardware numeric asset URLs and selected digests, without credentials. |
| Fresh standalone environment | PASS: SHA256-locked third-party closure downloaded, then installed offline; first-party wheels selected by digest, no sibling/editable/runtime source imports. |
| Published-wheel baseline | PASS: 108 tests, 30.30 seconds, external CWD. |
| Candidate wheel/sdist | PASS: built without network isolation from the independent export; fresh sdist rebuild reproduces wheel bytes with `SOURCE_DATE_EPOCH=1793119595`. |
| Installed candidate | PASS: forced exact-wheel reinstall, `pip check`, archive/RECORD/resource/import provenance; 108 tests, 27.43 seconds, `python -I -B -m pytest`. |
| Byte comparison | PASS: wheel inventory, all runtime payloads, dependencies, version and LICENSE unchanged; only METADATA/RECORD change. Sdist changes are current README, verification policy, ownership instructions, workflow and package metadata. |
| Workflow validation | PASS: official actionlint 1.7.12, `-shellcheck= -pyflakes=`. |
| Committed-source/master hosted CI | NOT_RUN in this local work unit; release owner verifies after integration. |

Published wheel SHA256 remains `f6fc643bc40130a0f8b4136db8db4f8abe1b0b366efaffa8c1866786429d9e9b`; candidate wheel is `878b0a338ee2c1f6412dc39ce80bd704bcd184f193a2313332b0dc6352389aad`. Candidate sdist is `03c7ce3bd4bfc10d1f29940c41463ac593d46eb0b03e9c8e74951ac4f84acf8a`. These are distinct same-version archives. The immutable v0.1.1 tag/assets retain original provenance and were not replaced or retagged. Current metadata does not imply that published bytes were rebuilt from a rename commit.

Inspected external evidence: `current-name-hardware-simulator-b9fa2760/VERIFICATION_RECEIPT.json` SHA256 `bb6a6b38f0c06e8ea6d96a7c806dfe78066e07a68699706f96eaa52da89d36c4`, and `ARTIFACT_COMPARISON_RECEIPT.json` SHA256 `a2384262a4dd9f13fe8745f274fe5d433377565afa6f72ec50148b29b4e92b84`, under the migration owner's canonical external session. Historical file and LICENSE physical hashes/normalized Git blobs match; original ignored-output lists are unchanged. This leaf task did not stage, commit, push or publish; shared-session inspection/cleanup remains with its owner. Hardware tests include unavailable-driver, lifecycle/freshness/revocation, actual loopback RPC/NCAP and spawned execution; they do not certify physical electrical safety, FPGA timing or deployment readiness. Remote owns API integration and quarantine acceptance.

The dated work record below is preserved as historical evidence.

---

# M6 product version work

Date: 2026-10-27. This is current work-unit evidence, separate from historical migration reports.

Hardware product remains exactly 0.1.1. Contracts and Simulator exact dependencies, artifact verification scripts, installed-resource tests, selected wheel checks, CI and current documentation now select 0.1.1. DEV remains 1.0.0a1. RPC 2.0, NCAP, firmware/protocol/register-map versions, lifecycle and unavailable physical capabilities are unchanged. Historical M4 evidence remains intact.

Changed Python/JSON/TOML syntax checks PASS. Installed-artifact/real-loopback/spawn/Stop-reset/quarantine acceptance is NOT_RUN in this work unit until committed dependency artifacts are rebuilt. No shared API environment installations were performed.

Follow README locked/offline bootstrap, build wheel/sdist to an allocator-owned external output, force-install `netcircuit_hardware_service-0.1.1-py3-none-any.whl`, run `pip check` and `scripts/check_installed.py --wheel ABSOLUTE_WHEEL`, then external-CWD `python -I -B -m pytest ABSOLUTE_TESTS --basetemp ABSOLUTE_SESSION/pytest`. Remote owns integrated API quarantine/lifecycle coverage.

All generated regression fixtures/archives used the shared external allocator session with all six Git roots. Shared-session cleanup remains with the migration owner. No stage/commit/merge/push was performed by this work unit.
