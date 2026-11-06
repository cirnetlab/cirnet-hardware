from copy import deepcopy
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor, TimeoutError
from multiprocessing import get_context
from threading import Event, Lock, RLock
from jsonschema import ValidationError
from contracts.runtime_config import simulation_worker_count
from contracts.validation import validate
from contracts.instrument_validation import validate_plan_instruments
from hardware_service.models import BreadboardCapability, StationCapabilities
from hardware_service.station.base import HardwareStation, HardwareError, StationState as S, serialized
from cirnet_sim import backends as simulator_backends
from cirnet_sim.backends import (SimulationError, run_plan, compile_graph,
    select_backend_id, get_backend_descriptor)

NATIVE_STOP_TIMEOUT_SECONDS = 2
_NATIVE_ERRORS = {
    'BACKEND_UNAVAILABLE': 'HARDWARE_UNAVAILABLE',
    'DRIVER_ERROR': 'DRIVER_ERROR',
    'PROTOCOL_MISMATCH': 'PROTOCOL_MISMATCH',
    'RESOURCE_CONFLICT': 'RESOURCE_CONFLICT',
    'SAFE_STATE_FAILED': 'DRIVER_ERROR',
    'TIMEOUT': 'TIMEOUT',
}


class _NativeJobs:
    """Run native calls in Hardware so control can signal their owner directly."""

    def __init__(self, workers):
        self._limit = workers
        self._executor = ThreadPoolExecutor(max_workers=workers, thread_name_prefix='cirnet-cpp')
        self._lock = Lock()
        self._jobs = {}

    def submit(self, *args):
        with self._lock:
            # A completed future has already returned its worker lane even
            # if its callback has not yet removed it from this registry.
            if sum(not job.done() for job in self._jobs) >= self._limit:
                raise HardwareError('RESOURCE_CONFLICT')
            cancel = Event()
            future = self._executor.submit(run_plan, *args, cancel=cancel)
            self._jobs[future] = cancel
        future.add_done_callback(self._discard)
        return future, cancel

    def _discard(self, future):
        with self._lock:
            self._jobs.pop(future, None)

    def shutdown(self):
        with self._lock:
            for cancel in self._jobs.values():
                cancel.set()
        self._executor.shutdown(wait=True, cancel_futures=True)
        close_workers = getattr(simulator_backends, 'close_native_workers', None)
        if close_workers is not None:
            close_workers()


_pool=None
_native_pool=None
_pool_lock=RLock()

def simulation_pool():
    global _pool
    with _pool_lock:
        if _pool is None:
            _pool=ProcessPoolExecutor(max_workers=simulation_worker_count(),mp_context=get_context('spawn'))
        return _pool

def close_simulation_pool():
    global _pool, _native_pool
    with _pool_lock:
        try:
            if _native_pool is not None:
                _native_pool.shutdown()
        finally:
            _native_pool=None
            if _pool:
                try:
                    _pool.shutdown(wait=True,cancel_futures=True)
                finally:
                    _pool=None


def _submit_execution(native, *args):
    global _native_pool
    # Keep creation and submission ordered with service shutdown.
    with _pool_lock:
        if not native:
            return simulation_pool().submit(run_plan, *args), None
        if _native_pool is None:
            _native_pool = _NativeJobs(simulation_worker_count())
        return _native_pool.submit(*args)

class VirtualHardwareStation(HardwareStation):
    """Independent digital session backed by bounded process-worker execution."""
    def __init__(self, station_id, *, backend_id=None):
        super().__init__(station_id)
        self._backend_id=select_backend_id(backend_id)
        self._backend=get_backend_descriptor(self._backend_id)
        self._circuit = None
        self._inputs = {}
        self._capture = None
        self._virtual_plan=None
        self._generation=0
        self._future=None
        self._cancel=None

    def new_session(self):
        """Create independent experiment state with this station's backend."""
        session=VirtualHardwareStation(self.station_id,backend_id=self._backend_id)
        if session._backend.identity!=self._backend.identity:
            raise HardwareError('UNEXECUTABLE')
        return session

    def get_capabilities(self):
        backend=self._backend
        available=backend.available
        analyzer=backend.logic_analyzer_channels if available else 0
        generator=backend.generator_channels if available else 0
        scope=backend.oscilloscope_channels if available else 0
        digital_events=backend.identity.id in ('legacy', 'cpp') and available
        return StationCapabilities(self.station_id, "simulation", available,
            virtual_digital=available and backend.digital,virtual_analog=available and backend.analog,
            breadboards=(BreadboardCapability("BB0"), BreadboardCapability("BB1")),
            hardware_profile_version="virtual-1.0" if digital_events else "UNAVAILABLE",
            firmware_version="virtual-events-1.0" if digital_events else "UNAVAILABLE",
            instrument_capabilities={"logic_analyzer":{"channels":analyzer,"runtime_reconfiguration_supported":False,"status":"digital-events" if analyzer else "NOT_IMPLEMENTED"},
                         "generator":{"channels":generator,"runtime_reconfiguration_supported":False,"status":"available" if generator else "NOT_IMPLEMENTED"},
                         "oscilloscope":{"channels":scope,"runtime_reconfiguration_supported":False,"status":"available" if scope else "NOT_IMPLEMENTED"}})

    @serialized
    def validate_configuration(self, configuration):
        valid = isinstance(configuration, dict)
        return {"valid":valid,"code":"OK" if valid else "INVALID_CONFIGURATION"}

    @serialized
    def apply_circuit(self, circuit):
        self._require_state(S.IDLE, S.SAFE_STATE, S.READY)
        self._circuit=self._virtual_plan=None
        if not isinstance(circuit, dict) or circuit.get("schema_version") not in ("1.0","1.1","1.2") or not isinstance(circuit.get("modules"),list) or not isinstance(circuit.get("connections"),list):
            raise HardwareError("UNEXECUTABLE")
        try:
            self._virtual_plan=compile_graph(circuit,backend_id=self._backend_id)
            if self._virtual_plan.backend!=self._backend.identity:
                raise SimulationError('BACKEND_IDENTITY_MISMATCH')
        except SimulationError:
            self._circuit=self._virtual_plan=None
            raise HardwareError('UNEXECUTABLE') from None
        self._circuit = deepcopy(circuit)
        self._generation+=1
        self._inputs.clear()
        self._plan = self._capture = None
        self._state = S.READY
        return self._reply()

    @serialized
    def set_input(self, input_id, value):
        if type(value) is not int or value not in (0,1):
            return {"accepted":False,"code":"INVALID_LOGIC_LEVEL"}
        self._inputs[input_id] = value
        return self._reply(input_id=input_id,value=value)

    @serialized
    def prepare_execution(self, plan, decision):
        self._require_state(S.READY)
        if self._circuit is None:
            raise HardwareError("INVALID_STATE")
        try:
            plan=validate("InstrumentationExecutionPlan",plan)
            decision=validate("ExecutionDecision",decision)
        except ValidationError:
            raise HardwareError("UNEXECUTABLE") from None
        if plan["station_id"] != self.station_id or not decision["decision"].startswith("RUN_"):
            raise HardwareError("UNEXECUTABLE")
        try:
            validate_plan_instruments(plan, self.get_capabilities().to_dict())
        except (ValueError, ValidationError):
            raise HardwareError("UNEXECUTABLE") from None
        aliases={b['endpoint_id']:b['node_id'] for b in plan['endpoint_bindings']}
        if any(port not in self._virtual_plan.directions for port in aliases.values()): raise HardwareError('UNEXECUTABLE')
        self._plan=plan
        self._aliases=aliases
        self._experiment_id=plan["experiment_id"]
        return self._reply()

    @serialized
    def apply_routing(self):
        if self._plan is None: raise HardwareError("INVALID_STATE")
        return self._reply(hardware_applied=False,mode="simulation")

    @serialized
    def guarded_power_up(self):
        if self._plan is None: raise HardwareError("INVALID_STATE")
        return self._reply(hardware_applied=False,mode="simulation")

    @serialized
    def configure_clock(self, configuration):
        return {'accepted':False,'code':'NOT_IMPLEMENTED','feature':'clock'}

    def configure_generator(self, configuration):
        return {"accepted":False,"code":"NOT_IMPLEMENTED","feature":"generator"}

    @serialized
    def configure_trigger(self, configuration):
        return {'accepted':False,'code':'NOT_IMPLEMENTED','feature':'trigger'}

    @serialized
    def arm_capture(self, configuration):
        self._require_state(S.READY)
        if self._plan is None: raise HardwareError("INVALID_STATE")
        if 'trigger' in configuration: raise HardwareError('NOT_IMPLEMENTED')
        self._plan["capture_config"]=validate("CaptureConfiguration",configuration)
        self._state=S.ARMED
        return self._reply()

    def start_execution(self,*,experiment_id=None):
        with self._lock:
            if experiment_id is not None and self._experiment_id!=experiment_id:
                raise HardwareError('RESOURCE_CONFLICT')
            self._require_state(S.ARMED)
            generation=self._generation
            aliases=self._aliases
            config=deepcopy(self._plan['capture_config'])
            config['signals']=[aliases.get(s,s) for s in config['signals']]
            vectors=[{'time_ns':v['time_ns'],'inputs':{aliases.get(s,s):x for s,x in v['inputs'].items()}} for v in self._plan['test_vectors']]
            clock=deepcopy(self._plan['clock_config'])
            if clock: clock['signal']=aliases.get(clock['signal'],clock['signal'])
            self._future, self._cancel = future, cancel = _submit_execution(
                self._backend_id == 'cpp', self._virtual_plan, config, vectors, clock, deepcopy(self._inputs))
            self._state=S.RUNNING
        try: capture=future.result(timeout=10)
        except TimeoutError:
            if cancel is not None:
                cancel.set()
            future.cancel()
            with self._lock:
                if generation!=self._generation: raise HardwareError('INVALID_STATE') from None
                self._state=S.FAULT
            raise HardwareError('TIMEOUT') from None
        except SimulationError as error:
            code = _NATIVE_ERRORS.get(str(error), 'UNEXECUTABLE') if cancel is not None else 'UNEXECUTABLE'
            with self._lock:
                if generation!=self._generation: raise HardwareError('INVALID_STATE') from None
                self._state=S.FAULT
            raise HardwareError(code) from None
        except Exception:
            with self._lock:
                if generation!=self._generation: raise HardwareError('INVALID_STATE') from None
                self._state=S.FAULT
            raise HardwareError('DRIVER_ERROR') from None
        with self._lock:
            if generation!=self._generation: raise HardwareError('INVALID_STATE')
            reverse={port:endpoint for endpoint,port in aliases.items()}
            for sample in capture['samples']: sample['signal']=reverse.get(sample['signal'],sample['signal'])
            self._capture=capture
            self._future=None
            self._cancel=None
            return self._reply()

    @serialized
    def read_capture(self):
        if self._capture and self._state==S.RUNNING: self._state=S.CAPTURING
        return deepcopy(self._capture) if self._capture else {"available":False,"code":"NOT_IMPLEMENTED","samples":[]}

    @serialized
    def enter_safe_state(self):
        self._generation+=1
        self._plan=None
        if self._cancel is not None:
            self._capture=None
            self._cancel_native_execution()
        elif self._future is not None:
            self._future.cancel()
        self._future=None
        self._cancel=None
        self._experiment_id=None
        self._state=S.SAFE_STATE
        return self._reply(hardware_applied=False,mode="simulation")

    def _cancel_native_execution(self):
        self._cancel.set()
        if self._future.cancel():
            return
        try:
            # The provider never takes the station lock. A completed reply
            # means the work ended; cancellation means the child was reaped.
            self._future.result(timeout=NATIVE_STOP_TIMEOUT_SECONDS)
            return
        except SimulationError as error:
            if str(error) != 'SAFE_STATE_FAILED':
                return
        except Exception:
            # A timeout or an unknown worker failure does not establish safety.
            pass
        self._state=S.FAULT
        raise HardwareError('DRIVER_ERROR') from None

    def stop_execution(self):
        return self.enter_safe_state()

    @serialized
    def reset(self):
        self._circuit=self._capture=None
        self._virtual_plan=None
        self._inputs.clear()
        return self.enter_safe_state()
