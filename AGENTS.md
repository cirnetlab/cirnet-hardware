# Repository work

- Start with the [English index](docs/en/index.md). After verification, update affected English and [Vietnamese](docs/vi/index.md) pages and the [changelog](docs/changelogs/README.md).
- Keep each task plan only in `.superpowers/plans/` and its matching spec, acceptance criteria and progress checklist only in `.superpowers/specs/`. Keep open items as `- [ ]` and use `- [x]` only after verification.
- Hardware owns local-only RPC 2.0, station lifecycles and private FPGA transport. Follow Remote’s Lab-First and safety authority linked from [station boundaries](docs/en/architecture/station-boundaries.md). Preserve guarded Start, safe Stop and quarantine handoff.
- Use exact Contracts and Simulator wheels for Python and the pinned Contracts crate and worker for native verification. Keep API orchestration, durable leases, quarantine and deployment in Remote, and canonical generation in Contracts. Avoid sibling source paths.
- Follow the [verification guide](docs/en/contributing/verification.md). Generate raw evidence with the pinned DEV allocator outside every checkout, worktree and Git directory. List the owner and all participating roots. Keep Markdown evidence in the repo. Inspect evidence and clean only owned sessions.
- Verify claims against source, Contracts and tests. Preserve assets, fixtures, licenses and historical evidence. Review Git diff, status and ignored outputs. Stage, commit, merge, push or publish only with explicit authorization.
