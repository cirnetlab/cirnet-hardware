# Write and maintain CIRNET Hardware documentation

[Tiếng Việt](../../vi/contributing/writing-documentation.md) · [Documentation](../index.md)

Follow the [repository instructions](../../../AGENTS.md) and the
[shared CIRNET conventions](https://github.com/cirnetlab/cirnet-rules/blob/master/en/policies/documentation.md).
Keep the root README in English with links to both language indexes. Give
current English and Vietnamese pages the same relative path and update
them together. Preserve commands, identifiers, versions, limits and
capability status in both languages.

Keep station lifecycle, local Hardware RPC and private FPGA transport guidance here. Remote owns API admission, durable leases, quarantine and deployment. Contracts owns the schemas and generated bindings. Simulator owns the backend and worker implementation.

Use [Task Continuity](https://github.com/cirnetlab/cirnet-rules/blob/master/en/docs/distribution.md#task-continuity) for temporary plans, acceptance criteria and progress. Keep independent design specifications and durable plans with their owning repository and link them from the checkpoint. Mark items complete after verification.

After verifying the change, update the paired pages
and the [release changelog](../../changelogs/README.md).

Keep dated records in the baseline archive. Cite their source revision for
superseded plans or specs. Keep raw logs and build output in an owned
external session. Before handoff, compare the language pair, verify
links and anchors, and review Git diff and ignored output.
[Verification guide](verification.md)
