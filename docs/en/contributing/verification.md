# Verify CIRNET Hardware

English · [Tiếng Việt](../../vi/contributing/verification.md)

[Documentation index](../index.md)

The producer [CI workflow](../../../.github/workflows/hardware-service.yml) runs
on Ubuntu for pushes to `master`, pull requests, and manual dispatch. It uses
the pinned DEV tool, Contracts and Simulator wheels, builds a Python wheel
and sdist, verifies the installed wheel, runs `tests/` from an external
directory, and uploads the archives for review. The native Rust build, the
opt-in Stage 1 lifecycle tests, and the Stage 3 suites run separately.

## Verify the installed Python wheel

Use Node 26 and Python 3.14 to reproduce the current CI environment. Activate a fresh Python environment outside all checkouts. Select reviewed Contracts and Simulator 0.1.2 manifests and the pinned DEV allocator. Before saving diagnostics, bootstrap the allocator and inherit the session and context. For a shared check, the context lists every participating repository and an explicit owner.

Run the following PowerShell commands from this repository. Replace both manifest paths with the reviewed absolute paths.

Run the commands in order and stop at the first failure.

~~~powershell
$env:CIRNET_CONTRACTS_ARTIFACT_MANIFEST = 'C:/absolute/contracts/artifacts.json'
$env:CIRNET_SIMULATOR_ARTIFACT_MANIFEST = 'C:/absolute/simulator/artifacts.json'
$bootstrap = python -B -c "from scripts.bootstrap_contract_tools import bootstrap; import json; print(json.dumps(bootstrap()))" | Select-Object -Last 1 | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or -not $bootstrap.session -or -not $bootstrap.context) {
    throw 'Could not allocate a verification session.'
}
$env:CIRNET_ARTIFACTS_SESSION = $bootstrap.session
$env:CIRNET_ARTIFACTS_CONTEXT = $bootstrap.context | ConvertTo-Json -Compress -Depth 10
$env:TMP = $env:TEMP = $env:TMPDIR = $bootstrap.session
$env:PIP_CACHE_DIR = Join-Path $bootstrap.session 'pip-cache'
$env:PYTHONDONTWRITEBYTECODE = '1'
$wheelhouse = Join-Path $bootstrap.session 'wheelhouse'
$dist = Join-Path $bootstrap.session 'dist'
python -B -m pip download --require-hashes --only-binary=:all: -r requirements-verification.lock --dest $wheelhouse
python -B -m pip install --require-hashes --only-binary=:all: --no-index --find-links $wheelhouse -r requirements-verification.lock
python -B scripts/install_contracts.py
python -B scripts/install_simulator.py --manifest $env:CIRNET_SIMULATOR_ARTIFACT_MANIFEST
$env:SOURCE_DATE_EPOCH = git log -1 --format=%ct
python -B -m build --no-isolation --outdir $dist
$wheel = Join-Path $dist 'cirnet_hardware_service-0.1.2-py3-none-any.whl'
python -B -m pip install --no-index --no-deps --force-reinstall $wheel
python -B -m pip check
python -B scripts/check_installed.py --wheel $wheel
$tests = Join-Path (Get-Location).Path 'tests'
Push-Location $bootstrap.session
try {
    python -I -B -m pytest $tests --basetemp (Join-Path $bootstrap.session 'pytest')
} finally { Pop-Location }
~~~

The wheel check binds the installed files, metadata and RECORD to the selected archive. The isolated tests exercise loopback RPC, NCAP, unavailable physical drivers, guarded lifecycle and spawned virtual workers. Check the sdist can build independently with the locked closure, and compare wheel bytes at the same `SOURCE_DATE_EPOCH` before release. A local source export and a committed release build have separate provenance.

## Stage 1 native worker

Use a candidate environment with the manifest-selected Hardware and Simulator
wheels, the pinned Contracts wheel, and pytest and psutil from the Remote
verification toolchain. Set `CIRNET_NATIVE_STAGE1_CANDIDATE` to the absolute
candidate manifest path. Run the following command from the owned session.

~~~powershell
python -I -B -m pytest -q C:/absolute/hardware/tests/native_cpp_lifecycle_cases.py --basetemp "$env:CIRNET_ARTIFACTS_SESSION/hardware-native-lifecycle"
~~~

The gate checks artifact hashes and installed payloads, then sends real
loopback gRPC requests to Hardware with a C++ child. It controls the child
process or reply timing to cover saturated Stop, reset, crash, timeout and a
reply held during Stop. For each case it checks worker exit, empty stale
capture, recovery with identical NCAP bytes and shutdown of the replacement
worker. These controlled fault timings are separate from performance samples.

## Rust Hardware candidate

Use Rust 1.90 or later and the locked Cargo dependency cache. The measured
Windows build used Rust 1.90.0. Prepare the cache outside the checkout before
running offline. The native build consumes `cirnet-contracts-native==0.1.2`
from revision `e640f80b1199779659d3db7817fee16d13cc24c1`. Set
`CIRNET_CONTRACTS_ROOT`, `CIRNET_SIM_CPP_WORKER`, and
`CIRNET_SIM_CPP_WORKER_SHA256` as in the [candidate setup](../guides/getting-started.md#run-the-rust-hardware-candidate).
Run from the Hardware repository with an active allocator session.

~~~powershell
$env:CARGO_TARGET_DIR = Join-Path $env:CIRNET_ARTIFACTS_SESSION 'hardware-cargo'
cargo build --release --locked --offline
cargo test --locked --offline --lib --bins
cargo test --locked --offline --test simulator_parity -- --ignored
~~~

The unit tests cover the wire protocol, worker supervision, station fencing,
Start rendezvous, and loopback binding. The four opt-in
[parity cases](../../../native/tests/simulator_parity.rs) need the real pinned
worker and Contracts resources. They check digital samples and exact NCAP,
powered DC preview and unsupported parts, clock and vector ordering and the
event budget, and physical mounting from
`contracts/design-models/fixtures/physical-mounting-parity.json`.

For the separate loopback RPC suite, use a verification environment with
pytest, psutil, and the pinned Python Contracts, Hardware, Simulator, and
Remote wheels. Use the Stage 1 reference wheels and
`CIRNET_NATIVE_STAGE1_CANDIDATE` as above. Set
`CIRNET_NATIVE_STAGE3_HARDWARE` to the absolute Rust executable path and
`CIRNET_NATIVE_STAGE3_HARDWARE_SHA256` to the selected digest. If the installed
Remote wheel differs from Stage 1, set `CIRNET_NATIVE_STAGE3_REMOTE_SHA256` to
the selected Remote digest. Keep `rustc` on PATH for the stale reply fixture
that the suite builds under its external test directory.

~~~powershell
$rpcTests = Join-Path (Get-Location).Path 'native/tests/test_rpc_lifecycle.py'
Push-Location $env:CIRNET_ARTIFACTS_SESSION
try {
    python -I -B -m pytest -q $rpcTests --basetemp (Join-Path $env:CIRNET_ARTIFACTS_SESSION 'hardware-native-rpc')
} finally { Pop-Location }
~~~

The [RPC suite](../../../native/tests/test_rpc_lifecycle.py) covers both RPC names,
NCAP parity, unavailable physical stations, cancellation and quarantine,
worker faults and stale replies. Its QPC cases run on Windows.
The [paired diagnostic suite](../../../native/tests/test_rpc_stop_diagnostic.py)
uses the same artifact inputs and is opt-in through
`CIRNET_NATIVE_STAGE3_STOP_DIAGNOSTIC=1` or
`CIRNET_NATIVE_STAGE3_RPC_DIAGNOSTIC=1`. The formal loaded Stop procedure
belongs to Remote. [Windows Gate 3 evidence](../history/stage-3-windows-verification.md) ·
[Remote Gate 3 policy](https://github.com/cirnetlab/cirnet-remote/blob/13a35145b6d15644c7be10d95a5410bf69e11867/docs/en/architecture/native-migration.md#gate-3-acceptance-policy-by-host)

## Inspect and clean the session

Record the result and scope in Markdown. Inspect raw evidence and confirm owned workers have stopped before cleaning only the session you own. Preserve release candidates selected for handoff separately.

~~~powershell
node scripts/artifact-paths.mjs --cleanup $env:CIRNET_ARTIFACTS_SESSION
~~~

See the [workflow](../../../.github/workflows/hardware-service.yml) and the [dated verification record](../history/verification-ledger.md). These checks establish installed software behavior within their recorded scope.
