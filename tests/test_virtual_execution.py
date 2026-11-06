from concurrent.futures import ThreadPoolExecutor
from dataclasses import replace
from threading import Event

import pytest
from hardware_service.station import virtual
from hardware_service.station.virtual import VirtualHardwareStation


def arm_station(s, experiment_id='e1'):
    graph={'schema_version':'1.0','circuit_id':'input-led',
           'modules':[{'id':'X','type':'DIGITAL_SWITCH','properties':{'state':True}},{'id':'L','type':'LED'}],
           'connections':[{'source':'X.OUT','destination':'L.IN'}]}
    s.apply_circuit(graph)
    p={'schema_version':'1.0','station_id':s.station_id,'experiment_id':experiment_id,'power_policy':{'profile_id':'virtual-1.0','guarded':True},
       'endpoint_bindings':[],'routing_allocations':[],'instrument_allocations':[],'clock_config':{},'generator_config':[],
       'capture_config':{'signals':['L.IN'],'duration_ns':20},'test_vectors':[],
       'runtime_safety_policy':{'profile_id':'virtual-1.0','watchdog_required':True}}
    s.prepare_execution(p,{'schema_version':'1.0','decision':'RUN_NORMAL','reasons':[]})
    s.arm_capture(p['capture_config'])
    return s


def test_virtual_station_executes_and_capture_and_reset_are_real():
    s=arm_station(VirtualHardwareStation('v1'))
    assert s.get_capabilities().virtual_digital is True, 'virtual simulator integration missing'
    assert s.start_execution()['accepted'] is True
    assert s.read_capture()['samples']==[{'signal':'L.IN','time_ns':0,'value':1}]
    s.stop_execution()
    assert s.state=='SAFE_STATE'
    s.reset()
    assert s.read_capture()['available'] is False


def test_invalid_virtual_graph_returns_stable_error_without_running():
    s=VirtualHardwareStation('v1')
    with pytest.raises(RuntimeError) as caught:
        s.apply_circuit({'schema_version':'1.0','circuit_id':'bad','modules':[{'id':'G','type':'NO_MODEL'}],'connections':[]})
    assert getattr(caught.value,'code',None)=='UNEXECUTABLE'


def test_virtual_trigger_configuration_is_truthfully_unavailable():
    s=VirtualHardwareStation('v1')
    reply=s.configure_trigger({'signal':'L.IN','edge':'RISING'})
    assert reply['accepted'] is False and reply['code']=='NOT_IMPLEMENTED'


@pytest.fixture
def native_station(monkeypatch):
    """Use a controlled cancellation boundary while testing the real station."""
    virtual.close_simulation_pool()
    monkeypatch.setenv('CIRNET_SIMULATION_WORKERS', '1')
    descriptor = virtual.get_backend_descriptor('legacy')
    descriptor = replace(descriptor, identity=replace(descriptor.identity, id='cpp'))
    compile_graph = virtual.compile_graph
    monkeypatch.setattr(virtual, 'select_backend_id', lambda backend_id: 'cpp')
    monkeypatch.setattr(virtual, 'get_backend_descriptor', lambda backend_id: descriptor)
    monkeypatch.setattr(virtual, 'compile_graph', lambda graph, **kwargs: replace(
        compile_graph(graph, backend_id='legacy'), backend=descriptor.identity))

    def unexpected_legacy_pool():
        pytest.fail('native execution entered the legacy process pool')

    monkeypatch.setattr(virtual, 'simulation_pool', unexpected_legacy_pool)
    yield arm_station(VirtualHardwareStation('v1'))
    virtual.close_simulation_pool()


@pytest.mark.parametrize('operation', ['stop_execution', 'reset'])
def test_native_safe_ack_waits_for_cancelled_job_and_fences_late_capture(native_station, monkeypatch, operation):
    started, cancelled, finished = Event(), Event(), Event()

    def run_plan(*args, cancel):
        started.set()
        assert cancel.wait(5)
        cancelled.set()
        assert finished.wait(5)
        return {'available': True, 'samples': [{'signal': 'L.IN', 'time_ns': 0, 'value': 1}]}

    monkeypatch.setattr(virtual, 'run_plan', run_plan)
    with ThreadPoolExecutor(max_workers=2) as calls:
        running = calls.submit(native_station.start_execution, experiment_id='e1')
        try:
            if not started.wait(1):
                running.result(timeout=1)
                pytest.fail('native job did not enter')
            stopping = calls.submit(getattr(native_station, operation))
            assert cancelled.wait(2)
            assert not stopping.done(), 'SAFE_STATE was acknowledged while the native job remained active'
        finally:
            finished.set()
        assert stopping.result(timeout=2)['state'] == 'SAFE_STATE'
        with pytest.raises(virtual.HardwareError, match='INVALID_STATE'):
            running.result(timeout=2)
    assert native_station.read_capture()['available'] is False


def test_native_unconfirmed_termination_keeps_faulted_owner_and_rejects_safe_ack(native_station, monkeypatch):
    def run_plan(*args, cancel):
        raise virtual.SimulationError('SAFE_STATE_FAILED')

    monkeypatch.setattr(virtual, 'run_plan', run_plan)
    with pytest.raises(virtual.HardwareError, match='DRIVER_ERROR'):
        native_station.start_execution(experiment_id='e1')
    for _ in range(2):
        with pytest.raises(virtual.HardwareError, match='DRIVER_ERROR'):
            native_station.stop_execution()
        assert native_station.state == 'FAULT'
        assert native_station._experiment_id == 'e1'


def test_native_stop_deadline_rejects_ack_until_pending_cleanup_finishes(native_station, monkeypatch):
    started, finished = Event(), Event()

    def run_plan(*args, cancel):
        started.set()
        assert finished.wait(5)
        assert cancel.is_set()
        raise virtual.SimulationError('CANCELLED')

    monkeypatch.setattr(virtual, 'run_plan', run_plan)
    monkeypatch.setattr(virtual, 'NATIVE_STOP_TIMEOUT_SECONDS', 0.05, raising=False)
    with ThreadPoolExecutor(max_workers=1) as calls:
        running = calls.submit(native_station.start_execution, experiment_id='e1')
        try:
            if not started.wait(1):
                running.result(timeout=1)
                pytest.fail('native job did not enter')
            with pytest.raises(virtual.HardwareError, match='DRIVER_ERROR'):
                native_station.stop_execution()
            assert native_station.state == 'FAULT'
            assert native_station._experiment_id == 'e1'
        finally:
            finished.set()
        with pytest.raises(virtual.HardwareError, match='INVALID_STATE'):
            running.result(timeout=2)
    assert native_station.stop_execution()['state'] == 'SAFE_STATE'


def test_native_saturation_rejects_extra_start_and_leaves_stop_available(native_station, monkeypatch):
    started = Event()
    other = arm_station(VirtualHardwareStation('v2'), 'e2')

    def run_plan(*args, cancel):
        started.set()
        assert cancel.wait(5)
        raise virtual.SimulationError('CANCELLED')

    monkeypatch.setattr(virtual, 'run_plan', run_plan)
    with ThreadPoolExecutor(max_workers=2) as calls:
        running = calls.submit(native_station.start_execution, experiment_id='e1')
        try:
            if not started.wait(1):
                running.result(timeout=1)
                pytest.fail('native job did not enter')
            extra = calls.submit(other.start_execution, experiment_id='e2')
            with pytest.raises(virtual.HardwareError, match='RESOURCE_CONFLICT'):
                extra.result(timeout=1)
            assert other.state == 'ARMED'
        finally:
            native_station.stop_execution()
        with pytest.raises(virtual.HardwareError, match='INVALID_STATE'):
            running.result(timeout=2)
