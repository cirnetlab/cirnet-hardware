"""Hardware runtime settings remain compatible with deployed station environments."""
import pytest

from contracts.runtime_config import simulation_worker_count


def test_worker_setting_legacy_fallback_and_canonical_precedence(monkeypatch):
    monkeypatch.delenv('CIRNET_SIMULATION_WORKERS', raising=False)
    monkeypatch.setenv('NETCIRCUIT_SIMULATION_WORKERS', '2')
    assert simulation_worker_count() == 2
    monkeypatch.setenv('CIRNET_SIMULATION_WORKERS', '3')
    assert simulation_worker_count() == 3
    monkeypatch.setenv('CIRNET_SIMULATION_WORKERS', '')
    with pytest.raises(ValueError):
        simulation_worker_count()


def test_product_server_registers_both_rpc_names_with_same_safety_behavior():
    import grpc
    from contracts.generated import rpc_v2_pb2 as pb
    from hardware_service.server import build_server
    from hardware_service.station.physical import PhysicalHardwareStation

    server = build_server({'physical': PhysicalHardwareStation('physical')})
    port = server.add_insecure_port('127.0.0.1:0')
    server.start()
    target = pb.Target(station_id='physical', experiment_id='compatibility')
    try:
        with grpc.insecure_channel(f'127.0.0.1:{port}') as channel:
            replies = []
            for namespace in ('cirnet.hardware.v2', 'netcircuit.hardware.v2'):
                capabilities = channel.unary_unary(
                    f'/{namespace}.HardwareService/GetCapabilities',
                    request_serializer=pb.Target.SerializeToString,
                    response_deserializer=pb.StationCapabilities.FromString)(target, timeout=5)
                start = channel.unary_unary(
                    f'/{namespace}.HardwareService/StartExecution',
                    request_serializer=pb.Target.SerializeToString,
                    response_deserializer=pb.OperationReply.FromString)(target, timeout=5)
                assert not capabilities.available
                assert not start.accepted and start.code == pb.HARDWARE_UNAVAILABLE
                replies.append((capabilities, start))
            assert replies[0] == replies[1]
    finally:
        server.stop(0).wait()
