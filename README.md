# CIRNET-Hardware

[English documentation](docs/en/index.md) · [Tài liệu tiếng Việt](docs/vi/index.md)

Repository: [cirnetlab/cirnet-hardware](https://github.com/cirnetlab/cirnet-hardware). Current distributions and settings use CIRNET names. Legacy `NETCIRCUIT_*` settings and RPC routes remain compatible; canonical settings take precedence.

Independent `cirnet-hardware-service==0.1.2`, Python >=3.11. The `hardware_service` namespace owns local-only Hardware RPC 2.0, virtual/physical stations and private FPGA transport. The canonical service route is `cirnet.hardware.v2.HardwareService`; the generated Contracts bindings also register the legacy `netcircuit.hardware.v2.HardwareService` route. New clients retry the legacy route only on `UNIMPLEMENTED`. Runtime consumes exact `cirnet-contracts==0.1.2` and `cirnet-simulator==0.1.2` wheels. No API implementation or sibling checkout is needed. Revalidate the existing immutable `v0.1.1` artifacts under the current repository URL while retaining their original source revisions and SHA256. Compare candidates from changed source separately; changed published bytes or tags require an explicit release decision and must respect immutability. Record the selected artifacts and consumer access in the system manifest.

Canonical [Lab-First v3.1 architecture](https://github.com/cirnetlab/cirnet-remote/blob/0d038c7aed15798696223558b4e58f5ffe926aa9/docs/ARCHITECTURE.md), [physical safety authority](https://github.com/cirnetlab/cirnet-remote/blob/0d038c7aed15798696223558b4e58f5ffe926aa9/docs/PHYSICAL_LAB_SAFETY.md) and approved M0 remain owned by Remote. The pinned authority revision is unchanged by the repository rename. Safety, topology and correctness are independent. Safe unknown or mismatched components do not become unsafe by identity alone.

Physical Start requires fresh scan/safety, a matching executable station/experiment plan, observed endpoint/resource coverage, applied routing, guarded power and armed capture. Missing driver reports OFFLINE/unavailable, zero slots/channels and `hardware_applied=false`. Serial/USB-SPI adapters remain unavailable placeholders. Test drivers, software/RTL gates and synthetic captures establish no electrical readiness, hardware ABI, ADC/DAC/AFE, SDRAM or physical timing guarantee. Remote owns admission, persisted leases/quarantine, capture storage and safe-release acknowledgment.

Virtual legacy digital execution uses independent per-experiment sessions and bounded spawned workers. Current Logic Analyzer has four channels; Scope/Generator have zero. `CIRNET_SIMULATION_BACKEND=legacy` is the default. Each station snapshots its backend; compiled identity survives environment changes and worker boundaries. Reserved `ngspice` remains unavailable without fallback. Set `CIRNET_SIMULATION_WORKERS` identically in API and Hardware (default 4, positive integer). RPC reserves four additional control threads. Stop/reset invalidates pending generation results; shutdown waits for owned workers.

Run the independently installed service with `python -m hardware_service.server`, default `127.0.0.1:50051`. `serve()` permits loopback/localhost or Unix sockets only. The browser never accesses it directly. Bindings come from installed Contracts; this product performs no generation. Genuine bounded digital NCAP is carried in `OperationReply.capture_payload`, separate from metadata-only `result_json`.

## Build and verify

Build the current source in a fresh environment with reviewed `cirnet-contracts==0.1.2` and `cirnet-simulator==0.1.2` wheels.
The immutable Hardware release keeps its original `netcircuit-contracts==0.1.1`
dependency and uses a separate environment. Both Contracts distributions install
the `contracts` namespace.

Use Node 26 and Python 3.14 in CI; Python >=3.11 is supported. Begin with an external Python environment and reviewed Contracts/DEV and Simulator artifact manifests. See [verification policy](docs/VERIFICATION.md). The complete third-party runtime/test/build closure is SHA256 locked in `requirements-verification.lock`; first-party wheels have separately reviewed version/digest manifests. The former `dev` code-generation extra is replaced by `test`: consumer tests import installed RPC bindings and never compile canonical Contracts.

```powershell
$env:CIRNET_CONTRACTS_ARTIFACT_MANIFEST = 'C:/absolute/contracts/artifacts.json'
$env:CIRNET_SIMULATOR_ARTIFACT_MANIFEST = 'C:/absolute/simulator/artifacts.json'
$bootstrap = python -c "from scripts.bootstrap_contract_tools import bootstrap; import json; print(json.dumps(bootstrap()))" | Select-Object -Last 1 | ConvertFrom-Json
$env:CIRNET_ARTIFACTS_SESSION = $bootstrap.session
$env:CIRNET_ARTIFACTS_CONTEXT = $bootstrap.context | ConvertTo-Json -Compress -Depth 10
$env:TMP = $env:TEMP = $env:TMPDIR = $bootstrap.session
$env:PIP_CACHE_DIR = Join-Path $bootstrap.session 'hw-leaf-pip-cache'
$env:PYTHONDONTWRITEBYTECODE = '1'
python -m pip download --require-hashes --only-binary=:all: -r requirements-verification.lock --dest "$env:CIRNET_ARTIFACTS_SESSION/hw-leaf-wheelhouse"
python -m pip install --require-hashes --only-binary=:all: --no-index --find-links "$env:CIRNET_ARTIFACTS_SESSION/hw-leaf-wheelhouse" -r requirements-verification.lock
python scripts/install_contracts.py
python scripts/install_simulator.py --manifest $env:CIRNET_SIMULATOR_ARTIFACT_MANIFEST
$env:SOURCE_DATE_EPOCH = git log -1 --format=%ct
python -m build --no-isolation --outdir "$env:CIRNET_ARTIFACTS_SESSION/hw-leaf-dist"
python -m pip install --no-index --no-deps --force-reinstall "$env:CIRNET_ARTIFACTS_SESSION/hw-leaf-dist/cirnet_hardware_service-0.1.2-py3-none-any.whl"
python -m pip check
python scripts/check_installed.py --wheel "$env:CIRNET_ARTIFACTS_SESSION/hw-leaf-dist/cirnet_hardware_service-0.1.2-py3-none-any.whl"
$product = (Get-Location).Path
Push-Location $env:CIRNET_ARTIFACTS_SESSION
python -I -B -m pytest "$product/tests" --basetemp "$env:CIRNET_ARTIFACTS_SESSION/hw-leaf-pytest"
Pop-Location
# Inspect evidence and preserve separately selected release artifacts before cleanup.
node scripts/artifact-paths.mjs --cleanup $env:CIRNET_ARTIFACTS_SESSION
```

CI selects the reviewed Contracts and Simulator 0.1.2 wheels with `CIRNET_CONTRACTS_WHEEL_URL`, `CIRNET_CONTRACTS_WHEEL_SHA256`, `CIRNET_SIMULATOR_WHEEL_URL` and `CIRNET_SIMULATOR_WHEEL_SHA256`. The prior Git archive pins in `integration/ci-source-dependencies.json` remain available to reproduce their recorded 0.1.1 builds. Local installs use explicit reviewed wheel selections. The independent DEV wheel still uses its configured URL and SHA256. Public published assets support anonymous access; authenticated CI/draft access still uses the selected-repository read token. Only GitHub release-asset API requests receive credentials; redirects do not. Missing pins fail closed. It downloads the locked closure, installs it offline, builds wheel/sdist without PEP 517 network isolation, force-installs the selected Hardware archive, checks version/RECORD/digest/import provenance and runs tests from an external CWD. The pytest prepend mode exposes only test-local helpers for real spawn imports; it does not expose product source. No automatic publication occurs. Hosted CI access and execution are separate evidence.

The existing immutable [v0.1.1 release](https://github.com/cirnetlab/cirnet-hardware/releases/tag/v0.1.1) retains its original tag, artifact bytes and source provenance. The 0.1.2 wheel and sdist belong to `cirnet-hardware-v0.1.2`, tagged `v0.1.2` after release acceptance. See [release verification work](docs/M6_RELEASE_WORK.md).

## Compatibility and rollback

The eight non-version runtime modules preserve Remote revision `0d038c7aed15798696223558b4e58f5ffe926aa9` byte-for-byte; package `__version__` is 0.1.2. Graph 1.0/1.1/1.2, RPC 2.0, NCAP v1, station/instrument semantics and Simulator backend identity remain unchanged. Logical FPGA scaffold and physical ABI are independent version domains.

Rollback stops admission, drains/Stops through acknowledged safe cleanup or quarantine, restores the prior compatible Hardware/Contracts/Simulator wheels and consumer pins, then restarts the service. Keep database, captures, leases and quarantine; never replay Start. Package rollback in an isolated environment does not prove electrical shutdown or live deployment. Original source and history remain preserved in Remote; the integrated release process verifies target ancestry, exact artifact/source revisions and publication.

Current-source builds produce CIRNET-named distributions. Existing published `netcircuit-*` wheels, their source revisions and hashes remain immutable; use separate environments for those historical artifacts. DEV tooling accepts the distribution/resource name selected by its exact release lock.
