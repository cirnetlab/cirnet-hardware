# Hardware verification and external artifacts

## 2026-11-05 Hardware 0.1.2 candidate

Native Windows verification at UTC+07:00 used an external export of the
uncommitted candidate based on `2a41c0c00ed862621877952ae4b6d84ba66d790d`,
CPython 3.14.4, and the exact canonical Contracts and Simulator 0.1.2 wheels.
The wheel built from the candidate sdist matched a separate source wheel build
byte for byte. The exact installed wheel provenance, `pip check`, and all 126
Hardware tests passed, including real loopback RPC and spawned workers.
Actionlint 1.7.12 passed the workflow. Raw output used the external session
owned by `cirnet-release-014-20261105`. The pytest log SHA256 is
`edface42357f3d18671483f9232599c3667bde4014cbc90001131e21fd4941d5`.
These results cover the local candidate. Release artifacts require a separate
build from the actual committed source.

## 2026-11-05 Lowercase repository paths

The pinned Contracts and Simulator commits are unchanged. GitHub regenerated
their source archives with lowercase root directories. Both archive inventories
and all file bytes matched the Git commits (266 Contracts files and 45 Simulator
files). The CI helper downloaded both lowercase URLs, verified the updated
archive SHA256 values and extracted the sources in the owned external session.
All 11 CI dependency profile tests passed. This check stopped before the wheel
build. Published release artifacts retain their existing pins.

## 2026-11-05 Python 3.14 compatibility

Primary CI and local verification now use normal-GIL CPython 3.14; reusable
package metadata retains its 3.11+ compatibility. A fresh Windows AMD64 3.14.6
environment passes all 114 installed-wheel tests, including actual loopback RPC
and spawned simulator workers, plus original archive/RECORD provenance checks.
Its independently extracted sdist rebuilds to identical candidate wheel bytes.
The existing binary-only hash closure already admits the selected 3.14 Windows
and glibc Linux wheels; explicit `spawn` preserves worker behavior across the
Linux 3.14 default-start-method change.

Independent native Linux 3.14.6 verification passes all 115 installed tests,
including the new CI selection guard, actual DEV bootstrap and real RPC/worker
coverage. Fresh wheel/sdist builds, selected provenance and `pip check` pass.
The original pinned Contracts build helper also passes end-to-end on 3.14.

The source CI helper adds only reviewed 3.14 wheel hashes to the pinned
Contracts build copy, checking original lock digests and exact versions before
any write. Eleven profile tests pass, including unsafe paths, altered sources
and invalid version/hash rejection. Release pins and runtime behavior are
unchanged. Raw diagnostics start in the external session owned by
`cirnet-python314-compat-20261105`. Hosted Actions, publication and physical
hardware/FPGA acceptance are NOT_RUN for this upgrade.

## 2026-11-05 Node 26 LTS CI baseline

Current CI selects Node 26 LTS before allocator bootstrap. Both tests in
`tests/test_bootstrap_environment.py` pass with portable Node 26.13.0 on PATH;
actionlint 1.7.12 passes the workflow with `-shellcheck= -pyflakes=`. This changes
the verification toolchain only. Runtime Python source, dependency locks and
published artifacts are unchanged. The full installed-wheel/RPC gate, hosted
Actions and physical hardware verification are NOT_RUN for this policy change.
Historical results below retain their recorded Node versions. Evidence was
inspected in the shared external session owned by `cirnet-node26-compat-20261105`.

Current repository identity is [cirnetlab/cirnet-hardware](https://github.com/cirnetlab/cirnet-hardware). Dated extraction and provenance records remain valid. Current-source CI selects exact Contracts and Simulator 0.1.2 wheels by immutable URL and SHA256. The Git archive pins in `integration/ci-source-dependencies.json` reproduce the recorded 0.1.1 dependency builds. Keep immutable v0.1.1 source/tag/artifact provenance separate from candidates rebuilt after documentation or package metadata changes.

Create logs, raw JSON, screenshots, traces, exports, environments and diagnostic output outside every checkout, worktree and Git administration directory from the start. Only Markdown verification evidence belongs in this repository. Preserve approved source assets, fixtures, historical evidence and licenses. Build caches/egg-info are compilation outputs, not diagnostic evidence; do not package them.

Current verification selects `cirnet-artifact-paths==1.0.0a1` and its `cirnet_artifact_paths` module. `scripts/bootstrap_contract_tools.py` verifies wheel/Node digests from `integration/contracts-version.json` before the first allocation, publishes the owning session/context and cleanup CLI before pip, and materializes only the verified allocator as a compilation cache. No consumer owns another path-validation implementation. Node and Git are required; CI selects Node 26 LTS.

Before a cross-repository allocation, set `CIRNET_ARTIFACTS_CONTEXT` to JSON containing an explicit owner and all participating absolute Git roots. Keep that exact context/session through children and cleanup. Standalone verification needs only this repository. An exported source product inside a session remains an export, not a Git root. Denied output or Git enumeration fails closed; never fall back into source. Bootstrap pip and later child processes use session-owned TEMP/TMP/TMPDIR and caches. Set external defaults before any application import in integration.

Use a fresh external environment, the SHA256-locked complete third-party closure, and reviewed wheel manifests for `cirnet-contracts==0.1.2` and a Simulator 0.1.2 build that pins that distribution. No sibling sources, editable installation, unpinned latest or root codegen is supported. Build `--no-isolation` after installing locked build requirements. Inspect wheel/sdist members and licenses, then force-install the exact candidate wheel with `--no-deps`, run `pip check`, and bind `scripts/check_installed.py --wheel ABSOLUTE_WHEEL` to its archive digest, metadata, payload and RECORD. Test from an external CWD with `python -I -B -m pytest ABSOLUTE_PRODUCT/tests --basetemp SESSION/hw-leaf-pytest`; test helpers remain spawn-importable without exposing product source. Cacheprovider stays disabled. Pass `-B` explicitly because isolated mode ignores the `PYTHONDONTWRITEBYTECODE` environment variable.

For release acceptance, export the reviewed source into the owned session, prove the sdist builds without sibling source, prove repeat wheel bytes with the same `SOURCE_DATE_EPOCH`, and install the full closure offline using `--require-hashes --only-binary=:all: --no-index --find-links ABSOLUTE_WHEELHOUSE`. Distinguish fresh uncommitted exports from committed clean checkout evidence. Retain exact first-party hashes/version/source fingerprints separately for release review. Test no-driver unavailable, guarded lifecycle/freshness/revocation, real loopback RPC/NCAP and actual spawned execution; Remote additionally verifies API leases/quarantine and integration against the same installed wheel.

Record revision/dirty state, time/timezone, environment, commands, PASS/FAIL/NOT_RUN/BLOCKED, evidence inspection and limits in Markdown. Tests do not certify electrical safety, FPGA ABI/timing or real hardware. Workflow changes require actionlint 1.7.12 with `-shellcheck= -pyflakes=`. Compare status, staged diff and physical ignored-output inventory before/after; keep original history/source unchanged.

After evidence inspection and worker shutdown, clean only the owning session with `node scripts/artifact-paths.mjs --cleanup SESSION`. CI uses the published canonical cleanup CLI with `always()` even after bootstrap/install failure. Never clean shared parent directories, another session, or historical outputs. When sharing an integration session, its owner performs final cleanup; leaf tasks do not remove it.


## 2026-11-04 canonical naming verification

PASS on Windows, Asia/Saigon (+07:00), with Python 3.14.4, Node 24.20.0,
pytest 9.1.1 and actionlint 1.7.12. CI continues to pin Python 3.12 and Node 22;
this local run does not claim a Linux CI result. Verification used fresh external
source exports and a fresh virtual environment with the complete hash-locked
Hardware verification closure. The exports were uncommitted inputs based on
`d58f96a051ea5940b46cf2e661bd200f4c275521`; they are distinct from a future committed checkout and immutable
published releases. `SOURCE_DATE_EPOCH=1793782800` was explicit.

The selected `cirnet-contracts==0.1.1` wheel SHA256 was
`2ba8cd1a3c1aeb02c10c154a5e3d8b169e96f4caee4c332d4a1bbc88d9ff2e1a`.
The immutable DEV wheel remained selected by its existing SHA256
`ff76532c69b2115338edb641660c966c5459dd40cb806f52a1e2e8c379c70a1f`;
actual canonical-environment bootstrap, installation and allocator integrity
checks passed with that legacy artifact. No published artifact or historical
provenance lock was changed.

`cirnet_hardware_service-0.1.1-py3-none-any.whl` SHA256:
`9bf5b632fa2313272fc7389f3b72b2932715b55f0b3d84443aee6ab392d46f00`.
The wheel contains nine `hardware_service` modules and 14 total archive members
including distribution metadata. It pins `cirnet-simulator==0.1.1`, selected by
SHA256 `8865ba5c120ccf6c41bb7f2f7927d092dcccbf5c79146118353b0a542b583cb2`.
All **111 tests passed** in the final installed-wheel run. Coverage includes actual
loopback calls through canonical and legacy RPC routes with identical safety
behavior, legacy worker-setting fallback and canonical precedence, 100,000-sample
NCAP transport, guarded lifecycle, no-driver behavior and actual spawned workers.

The initial sandboxed run passed 100 tests and could not connect its 11 loopback
RPC cases because Windows returned WSA10013. Those same cases passed with local
socket access; the final complete run also passed with that access. No RPC test
was skipped or replaced with a mocked transport.

`python -m pip check` and the newly selected wheel's
`scripts/check_installed.py --wheel ABSOLUTE_WHEEL` passed after force reinstall.
Tests ran from the external working directory using
`python -I -B -m pytest ABSOLUTE_TESTS --basetemp OWNED_SESSION/PRODUCT-pytest-final-triplet`.
The changed workflow passed actionlint 1.7.12 with `-shellcheck= -pyflakes=`;
canonical CI secrets/variables retain explicit legacy fallbacks, including SHA256.
The official Windows actionlint archive SHA256 was
`6e7241b51e6817ea6a047693d8e6fed13b31819c9a0dd6c5a726e1592d22f6e9`.

Raw build/install/test diagnostics began under the allocator-owned shared session
`C:/Users/trann/.codex/visualizations/2026/10/28/01a184f1-bcd8-7171-94bd-3a3cef062860/netcircuit-zw1kOX/python-leaves`.
The session owner retains it for integrated review and cleanup. Frozen fixtures,
historical Markdown and published release inputs were preserved. These checks do
not establish physical hardware availability, electrical safety certification,
native SPICE integration or analog capability.

### Final namespace and allocator follow-up (2026-11-04)

All 113 tests pass with final Contracts wheel `e129eb9075083dccbce9a37a595abe04f405197c2c06a86e651cb47b9a8acead`, Simulator wheel `8865ba5c120ccf6c41bb7f2f7927d092dcccbf5c79146118353b0a542b583cb2` and Hardware wheel `e22fed2ce416dbf69a7c2d808266bd1298cea2189073bdc445d5928389ef7669`. This includes real native RPC/future cases and both allocator alias regressions. The source CI selects the pushed Contracts and Simulator commits by full Git revision and an archive SHA256 verified against every Git blob. Current raw diagnostics are external in the `netcircuit-cKRGky` session; the earlier shared diagnostic directory was partially removed by a corrected Remote test-fixture isolation issue. Source, Git history and developer data were preserved.
