# M4 Hardware extraction verification

Date: 2026-10-27, Asia/Saigon. Source Remote revision `0d038c7aed15798696223558b4e58f5ffe926aa9`; target base `2b0005f40dc826bc7940cd0af28c57ebb063fcb7`, branch `codex/m4-hardware-extract`. The product is an uncommitted reviewed extraction, LOCAL_UNPUBLISHED. No staging, authored commit, merge, push, source deletion, license change or M5 is included.

Environment: Windows AMD64, Python 3.14.4, Node 24.20.0, setuptools 84.0.0. Contracts 1.0.1 and Simulator 0.2.0 are the exact reviewed independent wheels. CI specifies Linux/Python 3.12/Node 22; hosted execution and minimum Python 3.11 are NOT_RUN locally.

| Verification | Result and scope |
| --- | --- |
| Original runtime and tests | PASS: eight non-version modules byte-identical to retained Remote; package initializer changes version only. Every assert/raise AST node in the seven original test files is preserved. Source-path injection and root codegen setup are removed. |
| Installed package RED | PASS: fresh private environment with Contracts/Simulator but no Hardware fails the installed-distribution test with missing hardware_service. |
| Initial standalone wheel suite | PASS: 106 tests, 19.67 seconds, external CWD and installed Hardware wheel. Includes actual spawned workers, deterministic capture, owned pool shutdown, control threads, physical lifecycle/freshness/profile limits, unavailable driver/transport and genuine 100k-event NCAP RPC. |
| Fresh export and clean sdist | PASS: independent uncommitted source export; wheel/sdist built with no sibling source or build-network isolation. Clean sdist rebuild and fresh export reproduce candidate wheel bytes with SOURCE_DATE_EPOCH=1793107095. Wheel contains nine runtime modules plus five dist-info members, including unchanged LICENSE. |
| Offline complete closure | PASS: 22 SHA256-locked third-party Windows wheels downloaded and installed into a second fresh external environment using --require-hashes --only-binary=:all: --no-index. Reviewed first-party wheels installed with --no-deps. pip check and exact version/RECORD/archive-digest/import provenance pass. |
| Clean installed tests | PASS: 106 tests in 21.00 seconds from the extracted sdist against installed artifacts. |
| Isolated package rollback | PASS: candidate 0.1.1 to retained-source baseline 0.1.0 to candidate 0.1.1. Exact old version/no-driver capability and pip check pass; all 86 adapted original tests pass on 0.1.0 in 6.53 seconds. Restored 0.1.1 provenance/pip check and 106 tests pass in 23.02 seconds. This is package compatibility evidence, not a deployed electrical rollback. |
| Workflow syntax | PASS: actionlint 1.7.12 with -shellcheck= -pyflakes=. Hosted CI and artifact-download ACL remain unverified. |

The initial negative provenance tests verify source shadowing, changed payloads and stale same-version archive rejection; an additional unowned installed-file case is included in the final source. Final exported artifacts, exact source inventory/fingerprint and latest gate results are recorded in the external reviewed artifact manifest and Remote's canonical M4 verification record. No sdist hash is embedded here because this document is itself an sdist source input.

Ruling: replace the former grpcio-tools dev extra with a pytest test extra. Consumers use packaged Contracts bindings; this removes unnecessary canonical generation tooling without changing runtime dependencies. Ruling: pytest prepend mode exposes test-local helpers, including module-level spawn targets, while commands run outside the checkout and runtime imports are wheel-owned. Only deliberate negative provenance probes introduce attacked source paths.

Inspection found that Python -I ignores PYTHONDONTWRITEBYTECODE. Standard commands and negative subprocess probes now explicitly use -B. Only this task's initially created target __pycache__ compilation caches were removed after absolute path checks; existing historical outputs were untouched. The canonical allocator uses the shared M4 owner/context across Remote, Hardware, Contracts and Simulator. Logs, environments, exports, builds, wheelhouse and raw diagnostics were created externally from the start and inspected. No visual evidence was generated; no electrical hardware was inspected. The integration owner retains and cleans the shared session.

Publication/download ACL, connected target ancestry, hosted CI, deployment, physical shutdown/electrical safety, FPGA ABI/freshness and actual acquisition/output quality remain separate open gates. Safety and topology/correctness stay independent; no driver or capability has been invented. Operational rollback still stops admission, requires acknowledged safe cleanup or quarantine, preserves leases/captures/data and never replays Start.
