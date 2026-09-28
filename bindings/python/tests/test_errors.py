"""All public argument failures can be caught through the SDK base exception."""
import inspect
import sys

import pytest

from aliyun_log_producer import (
    ConfigError, Credentials, DeliveryError, EnqueueFullError,
    InvalidArgumentError, Log, Producer, ProducerClosedError, ProducerConfig,
    ProducerError,
)


@pytest.mark.parametrize("exception", [ConfigError, InvalidArgumentError, EnqueueFullError, ProducerClosedError])
def test_exception_hierarchy(exception):
    assert issubclass(exception, ProducerError)
    assert not issubclass(exception, (ValueError, TypeError, OverflowError))
    assert not issubclass(DeliveryError, BaseException)


@pytest.mark.parametrize("args,kwargs", [
    ((), {}), (("example.com",), {}),
    ((), {"endpoint": 1}), ((), {"endpoint": "example.com", "unknown": 1}),
    ((), {"endpoint": "example.com", "access_key_id": "id", "access_key_secret": "secret", "linger": "fast"}),
    ((), {"endpoint": "example.com", "access_key_id": "id", "access_key_secret": "secret", "buffer_bytes": -1}),
    ((), {"endpoint": "example.com", "access_key_id": "id", "access_key_secret": "secret", "max_attempts": 2**100}),
])
def test_config_binding_errors(args, kwargs):
    with pytest.raises(ProducerError) as caught:
        ProducerConfig(*args, **kwargs)
    assert type(caught.value) is ConfigError


@pytest.mark.parametrize("args,kwargs", [((), {}), ((None,), {}), ((), {"unknown": 1})])
def test_producer_binding_errors(args, kwargs):
    with pytest.raises(ConfigError):
        Producer(*args, **kwargs)


@pytest.mark.parametrize("factory,args,kwargs", [
    (Log, (), {}), (Log, ([],), {"unknown": 1}),
    (Log, ([],), {"contents": []}), (Log, ([], 1), {}),
    (Credentials, (), {}), (Credentials, (), {"access_key_id": 1, "access_key_secret": "secret"}),
    (Credentials, (), {"access_key_id": "id", "access_key_secret": "secret", "unknown": 1}),
])
def test_snapshot_binding_errors(factory, args, kwargs):
    with pytest.raises(InvalidArgumentError):
        factory(*args, **kwargs)


@pytest.mark.parametrize("args,kwargs", [
    ((), {}), (({},), {"unknown": 1}), (({},), {"log": {}}),
    (({}, {}), {}), (({},), {"source": 1}), (({},), {"topic": None}),
])
def test_send_binding_errors_do_not_admit_logs(make_producer, service, args, kwargs):
    producer, writer = make_producer()
    callbacks = []
    with pytest.raises(InvalidArgumentError):
        writer.send(*args, on_delivery=callbacks.append, **kwargs)
    producer.close()
    assert service.requests == []
    assert callbacks == []


def test_writer_binding_errors(make_producer):
    producer, _ = make_producer()
    for args, kwargs in [((), {}), ((1, "store"), {}), (("127", "bad/name"), {}),
                         (("127", "store"), {"unknown": 1})]:
        with pytest.raises(InvalidArgumentError):
            producer.writer(*args, **kwargs)


def test_signatures_remain_descriptive(make_producer):
    _, writer = make_producer()
    # Native constructor introspection is available on CPython 3.10+.
    if sys.implementation.name == "cpython" and sys.version_info >= (3, 10):
        assert list(inspect.signature(Log).parameters) == ["contents", "time", "time_ns"]
        assert "endpoint" in inspect.signature(ProducerConfig).parameters
        assert "access_key_id" in inspect.signature(Credentials).parameters
    # PyPy does not expose text signatures for native methods either.
    if sys.implementation.name != "pypy":
        assert "log" in inspect.signature(writer.send).parameters
    assert list(inspect.signature(Producer).parameters) == ["config"]


@pytest.mark.parametrize("exception", [KeyboardInterrupt, SystemExit, MemoryError])
def test_numeric_conversion_does_not_swallow_control_exceptions(exception):
    class Interrupted:
        def __float__(self):
            raise exception()
    with pytest.raises(exception):
        ProducerConfig(endpoint="example.com", access_key_id="id",
                       access_key_secret="secret", linger=Interrupted())
