# Verify the Hardware wheel

English · [Tiếng Việt](../vi/verification.md)

[Documentation index](index.md)

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

Record the result and scope in Markdown. Inspect raw evidence and confirm owned workers have stopped before cleaning only the session you own. Preserve release candidates selected for handoff separately.

~~~powershell
node scripts/artifact-paths.mjs --cleanup $env:CIRNET_ARTIFACTS_SESSION
~~~

See the [workflow](../../.github/workflows/hardware-service.yml) and the [dated verification record](../VERIFICATION.md). These checks establish installed software behavior within their recorded scope.
