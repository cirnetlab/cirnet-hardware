# CIRNET-Hardware

Repository: [CIRNETLAB/CIRNET-Hardware](https://github.com/CIRNETLAB/CIRNET-Hardware). Existing local directories may retain the `net-Circuit-Hardware` name. Python distributions/imports, `NETCIRCUIT_*` settings and protocol identifiers retain their compatible names.

Independent `netcircuit-hardware-service==0.1.1`, Python >=3.11. The `hardware_service` namespace owns local-only Hardware RPC 2.0, virtual/physical stations and private FPGA transport. Runtime consumes exact `cirnet-contracts==0.1.1` and `netcircuit-simulator==0.1.1` wheels. No API implementation or sibling checkout is needed. Revalidate the existing immutable `v0.1.1` artifacts under the current repository URL while retaining their original source revisions and SHA256. Compare candidates from changed source separately; changed published bytes or tags require an explicit release decision and must respect immutability. Record the selected artifacts and consumer access in the system manifest.

Canonical [Lab-First v3.1 architecture](https://github.com/CIRNETLAB/CIRNET-Remote/blob/0d038c7aed15798696223558b4e58f5ffe926aa9/docs/ARCHITECTURE.md), [physical safety authority](https://github.com/CIRNETLAB/CIRNET-Remote/blob/0d038c7aed15798696223558b4e58f5ffe926aa9/docs/PHYSICAL_LAB_SAFETY.md) and approved M0 remain owned by Remote. The pinned authority revision is unchanged by the repository rename. Safety, topology and correctness are independent. Safe unknown or mismatched components do not become unsafe by identity alone.

Physical Start requires fresh scan/safety, a matching executable station/experiment plan, observed endpoint/resource coverage, applied routing, guarded power and armed capture. Missing driver reports OFFLINE/unavailable, zero slots/channels and `hardware_applied=false`. Serial/USB-SPI adapters remain unavailable placeholders. Test drivers, software/RTL gates and synthetic captures establish no electrical readiness, hardware ABI, ADC/DAC/AFE, SDRAM or physical timing guarantee. Remote owns admission, persisted leases/quarantine, capture storage and safe-release acknowledgment.

Virtual legacy digital execution uses independent per-experiment sessions and bounded spawned workers. Current Logic Analyzer has four channels; Scope/Generator have zero. `NETCIRCUIT_SIMULATION_BACKEND=legacy` is the default. Each station snapshots its backend; compiled identity survives environment changes and worker boundaries. Reserved `ngspice` remains unavailable without fallback. Set `NETCIRCUIT_SIMULATION_WORKERS` identically in API and Hardware (default 4, positive integer). RPC reserves four additional control threads. Stop/reset invalidates pending generation results; shutdown waits for owned workers.

Run the independently installed service with `python -m hardware_service.server`, default `127.0.0.1:50051`. `serve()` permits loopback/localhost or Unix sockets only. The browser never accesses it directly. Bindings come from installed Contracts; this product performs no generation. Genuine bounded digital NCAP is carried in `OperationReply.capture_payload`, separate from metadata-only `result_json`.

## Build and verify

Build the current source in a fresh environment with reviewed
`cirnet-contracts==0.1.1` and Simulator wheels built against that dependency.
The immutable Hardware release keeps its original `netcircuit-contracts==0.1.1`
dependency and uses a separate environment. Both Contracts distributions install
the `contracts` namespace.

Use Node 22 and Python 3.12 in CI; Python >=3.11 is supported. Begin with an external Python environment and reviewed Contracts/DEV and Simulator artifact manifests. See [verification policy](docs/VERIFICATION.md). The complete third-party runtime/test/build closure is SHA256 locked in `requirements-verification.lock`; first-party wheels have separately reviewed version/digest manifests. The former `dev` code-generation extra is replaced by `test`: consumer tests import installed RPC bindings and never compile canonical Contracts.

```powershell
$env:NETCIRCUIT_CONTRACTS_ARTIFACT_MANIFEST = 'C:/absolute/contracts/artifacts.json'
$env:NETCIRCUIT_SIMULATOR_ARTIFACT_MANIFEST = 'C:/absolute/simulator/artifacts.json'
$bootstrap = python -c "from scripts.bootstrap_contract_tools import bootstrap; import json; print(json.dumps(bootstrap()))" | Select-Object -Last 1 | ConvertFrom-Json
$env:NETCIRCUIT_ARTIFACTS_SESSION = $bootstrap.session
$env:NETCIRCUIT_ARTIFACTS_CONTEXT = $bootstrap.context | ConvertTo-Json -Compress -Depth 10
$env:TMP = $env:TEMP = $env:TMPDIR = $bootstrap.session
$env:PIP_CACHE_DIR = Join-Path $bootstrap.session 'hw-leaf-pip-cache'
$env:PYTHONDONTWRITEBYTECODE = '1'
python -m pip download --require-hashes --only-binary=:all: -r requirements-verification.lock --dest "$env:NETCIRCUIT_ARTIFACTS_SESSION/hw-leaf-wheelhouse"
python -m pip install --require-hashes --only-binary=:all: --no-index --find-links "$env:NETCIRCUIT_ARTIFACTS_SESSION/hw-leaf-wheelhouse" -r requirements-verification.lock
python scripts/install_contracts.py
python scripts/install_simulator.py --manifest $env:NETCIRCUIT_SIMULATOR_ARTIFACT_MANIFEST
$env:SOURCE_DATE_EPOCH = git log -1 --format=%ct
python -m build --no-isolation --outdir "$env:NETCIRCUIT_ARTIFACTS_SESSION/hw-leaf-dist"
python -m pip install --no-index --no-deps --force-reinstall "$env:NETCIRCUIT_ARTIFACTS_SESSION/hw-leaf-dist/netcircuit_hardware_service-0.1.1-py3-none-any.whl"
python -m pip check
python scripts/check_installed.py --wheel "$env:NETCIRCUIT_ARTIFACTS_SESSION/hw-leaf-dist/netcircuit_hardware_service-0.1.1-py3-none-any.whl"
$product = (Get-Location).Path
Push-Location $env:NETCIRCUIT_ARTIFACTS_SESSION
python -I -B -m pytest "$product/tests" --basetemp "$env:NETCIRCUIT_ARTIFACTS_SESSION/hw-leaf-pytest"
Pop-Location
# Inspect evidence and preserve separately selected release artifacts before cleanup.
node scripts/artifact-paths.mjs --cleanup $env:NETCIRCUIT_ARTIFACTS_SESSION
```

CI accepts exact immutable HTTPS artifact URLs plus SHA256 repository variables and optional `NETCIRCUIT_ARTIFACTS_READ_TOKEN`. Set the Contracts and Simulator wheel URL and SHA256 variables to the matching reviewed candidates before running current-source CI. DEV selection remains independently pinned. Public published assets support anonymous access; authenticated CI/draft access still uses the selected-repository read token. Only GitHub release-asset API requests receive credentials; redirects do not. Missing pins fail closed. It downloads the locked closure, installs it offline, builds wheel/sdist without PEP 517 network isolation, force-installs the selected Hardware archive, checks version/RECORD/digest/import provenance and runs tests from an external CWD. The pytest prepend mode exposes only test-local helpers for real spawn imports; it does not expose product source. No automatic publication occurs. Hosted CI access and execution are separate evidence.

The existing immutable [v0.1.1 release](https://github.com/CIRNETLAB/CIRNET-Hardware/releases/tag/v0.1.1) retains its original tag, artifact bytes and source provenance. A build from renamed current-source documentation/metadata is a separate candidate even when its package version is 0.1.1; it must not replace those published bytes or be attributed to the original tag. See [release verification work](docs/M6_RELEASE_WORK.md).

## Compatibility and rollback

The eight non-version runtime modules preserve Remote revision `0d038c7aed15798696223558b4e58f5ffe926aa9` byte-for-byte; only package `__version__` becomes 0.1.1. Graph 1.0/1.1/1.2, RPC 2.0, NCAP v1, station/instrument semantics and Simulator backend identity remain unchanged. Logical FPGA scaffold and physical ABI are independent version domains.

Rollback stops admission, drains/Stops through acknowledged safe cleanup or quarantine, restores the prior compatible Hardware/Contracts/Simulator wheels and consumer pins, then restarts the service. Keep database, captures, leases and quarantine; never replay Start. Package rollback in an isolated environment does not prove electrical shutdown or live deployment. Original source and history remain preserved in Remote; the integrated release process verifies target ancestry, exact artifact/source revisions and publication.
