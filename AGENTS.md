# Hardware repository work

Start with the [current English documentation](docs/en/index.md). Update its
Vietnamese counterpart in the same documentation change.

Read the [repository overview](README.md) and [verification guide](docs/en/contributing/verification.md) before changes. Canonical Lab-First architecture, safety and migration authority remain in cirnetlab/cirnet-remote at the revision linked from README.md. Hardware owns local-only RPC, stations and private transport. Remote owns API orchestration, leases, quarantine and deployment. Contracts and Simulator are exact versioned wheel dependencies. Do not add sibling source paths, API implementation dependencies or a canonical Contracts generator.

Create raw diagnostic output outside every checkout/worktree/Git directory from the start through the installed canonical DEV allocator. Use explicit ownership/context for every participating repository, inspect evidence and clean only this session's owned directory. Keep only Markdown verification evidence in source. Preserve source assets, fixtures, protected history, licenses and existing evidence. Do not stage, commit, merge, push or publish without explicit authorization.
