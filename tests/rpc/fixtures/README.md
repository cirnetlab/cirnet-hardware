# Hardware RPC verification data

`hardware-rpc-v2.json` records exact responses from the CIRNETLAB Hardware reference with a verified Simulator worker. Its provenance identifies the source revisions and reviewed artifact digests. The fixture records serialized Protobuf capabilities and the accepted flag, error enum number, state, JSON metadata and hex-encoded NCAP bytes for lifecycle replies. The 100,000-event clock capture uses the exact byte count and SHA-256, with the test reconstructing each expected event from the clock period.

`historical-rpc-diagnostics.json` retains the block and aggregate statistics of the prior paired Stop and capabilities diagnostics with their full receipt digests. Side B was the reference implementation. Side C was the native Hardware binary identified in those receipts. Each current native diagnostic repeats the same workload and sample count for side C; its new external receipt carries the newly tested Hardware and worker digests. Latency and throughput are host-specific measurements.

The data came from CIRNETLAB's GPL-3.0 repository implementations and is covered by this repository's [license](../../../LICENSE). The provenance records are attribution and can be checked against the external frozen baseline. Tests read these local records and generate their client messages from the selected Contracts schema.
