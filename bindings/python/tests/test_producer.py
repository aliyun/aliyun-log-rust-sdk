import functools
import gc
import inspect
import subprocess
import sys
import threading
import time
import weakref
from concurrent.futures import ThreadPoolExecutor

import pytest

from aliyun_log_producer import (
    DeliveryError,
    EnqueueFullError,
    Log,
    Producer,
    ProducerClosedError,
    ProducerConfig,
    ProducerError,
)


def test_log_snapshot_and_ranges():
    contents = [("key", "first"), ("key", "second")]
    log = Log(contents, time=2**32 - 1, time_ns=999999999)
    contents.clear()
    assert log.contents == [("key", "first"), ("key", "second")]
    log.contents.clear()
    assert len(log.contents) == 2
    assert log.time == 2**32 - 1
    assert log.time_ns == 999999999
    assert Log([], time=0).time_ns is None
    assert abs(Log([]).time - time.time()) < 2
    assert Log([]).time_ns is None
    with pytest.raises(AttributeError):
        log.time = 10
    for invalid in [-1, 2**32]:
        with pytest.raises(OverflowError):
            Log([], time=invalid)
    with pytest.raises(ValueError):
        Log([], time_ns=1000000000)
    with pytest.raises(TypeError):
        Log([("key", 12)])


@pytest.mark.parametrize("compression", ["zstd", "lz4"])
def test_wire_send_and_callback(make_producer, service, compression):
    producer, writer = make_producer(compression=compression, security_token="test-token")
    results = []
    log = Log([("key", "first-value"), ("key", "second-value")], time=1700000000)
    assert writer.send(log, source="python-source", topic="python-topic", on_delivery=results.append) is None
    producer.close()
    assert results == [None]
    assert len(service.requests) == 1
    path, headers, body = service.requests[0]
    assert path == "/logstores/store/shards/lb"
    assert headers["x-log-compresstype"] == compression
    assert headers["x-acs-security-token"] == "test-token"
    assert headers["Authorization"].startswith("LOG test-id:")
    if compression == "zstd":
        import zstandard
        raw = zstandard.ZstdDecompressor().decompress(body)
    else:
        import lz4.block
        raw = lz4.block.decompress(body, uncompressed_size=int(headers["x-log-bodyrawsize"]))
    assert len(raw) == int(headers["x-log-bodyrawsize"])
    assert raw.index(b"first-value") < raw.index(b"second-value")
    assert b"python-source" in raw and b"python-topic" in raw
    assert log.contents == [("key", "first-value"), ("key", "second-value")]


@pytest.mark.parametrize("user_agent", [None, "my-python-app/1.0"])
def test_user_agent_default_and_override(make_producer, service, user_agent):
    from aliyun_log_producer import __version__
    producer, writer = make_producer(user_agent=user_agent)
    writer.send({"message": "user agent"})
    producer.close()
    assert service.requests[0][1]["User-Agent"] == (user_agent or f"aliyun-log-python-producer/{__version__}")


def test_plain_send_does_not_inspect_callbacks(make_producer, monkeypatch):
    producer, writer = make_producer()
    def unexpected(*args):
        raise AssertionError("plain send must not construct a callback bridge")
    monkeypatch.setattr(inspect, "iscoroutinefunction", unexpected)
    writer.send(Log([("message", "plain")]))
    writer.send(Log([]), on_delivery=None)
    producer.close()


def test_callable_forms_and_reference_release(make_producer):
    producer, writer = make_producer()
    results = []
    class Receiver:
        def __call__(self, error):
            results.append(("object", error))
        def method(self, error):
            results.append(("method", error))
    receiver = Receiver()
    ref = weakref.ref(receiver)
    def function(error):
        results.append(("function", error))
    def with_context(context, error):
        results.append((context, error))
    callbacks = [function, lambda error: results.append(("lambda", error)), receiver,
                 receiver.method, functools.partial(with_context, "partial")]
    for callback in callbacks:
        writer.send(Log([]), on_delivery=callback)
    del receiver, callbacks, callback
    producer.close()
    gc.collect()
    assert ref() is None
    assert sorted(results) == [(name, None) for name in ["function", "lambda", "method", "object", "partial"]]


def test_noncallable_reports_invocation_error(make_producer, monkeypatch):
    producer, writer = make_producer()
    failures = []
    monkeypatch.setattr(sys, "unraisablehook", failures.append)
    for invalid in [1, "callback"]:
        assert writer.send(Log([]), on_delivery=invalid) is None
    producer.close()
    assert [item.exc_type for item in failures] == [TypeError, TypeError]
    # Delivery callbacks may complete in either order.
    assert {item.object for item in failures} == {1, "callback"}


def test_callback_return_values_are_ignored(make_producer, monkeypatch):
    producer, writer = make_producer()
    failures, results = [], []
    monkeypatch.setattr(sys, "unraisablehook", failures.append)
    class Awaitable:
        def __await__(self):
            raise AssertionError("return values must not be awaited")
        def close(self):
            raise AssertionError("return values must not be closed")
    async def coroutine():
        raise AssertionError("return values must not be executed")
    returned = coroutine()
    try:
        for callback in [lambda error: 42, lambda error: Awaitable(),
                         lambda error: returned, results.append]:
            writer.send(Log([]), on_delivery=callback)
        producer.close()
        assert results == [None]
        assert failures == []
        assert inspect.getcoroutinestate(returned) == inspect.CORO_CREATED
    finally:
        returned.close()


def test_callback_exception_does_not_stop_worker(make_producer, monkeypatch):
    producer, writer = make_producer()
    failures, results = [], []
    monkeypatch.setattr(sys, "unraisablehook", failures.append)
    def broken(error):
        raise RuntimeError("callback failed")
    writer.send(Log([]), on_delivery=broken)
    writer.send(Log([]), on_delivery=results.append)
    producer.close()
    assert results == [None]
    assert [item.exc_type for item in failures] == [RuntimeError]
    assert failures[0].object is broken


def test_structured_delivery_failure(make_producer, service):
    service.status = 403
    service.body = b'{"errorCode":"Unauthorized","errorMessage":"permission denied"}'
    producer, writer = make_producer()
    results = []
    writer.send(Log([]), on_delivery=results.append)
    # Delivery failures are data; flush/close still succeed.
    producer.flush()
    producer.close()
    assert len(results) == 1
    error = results[0]
    assert isinstance(error, DeliveryError)
    assert (error.kind, error.message, error.http_status, error.error_code, error.request_id) == (
        "server", "permission denied", 403, "Unauthorized", "python-test-request")
    with pytest.raises(AttributeError):
        error.kind = "changed"


def test_delivery_timeout_is_callback_data(make_producer, service):
    service.release.clear()
    producer, writer = make_producer(delivery_timeout=0.05)
    results = []
    writer.send(Log([]), on_delivery=results.append)
    producer.close()
    assert len(results) == 1 and results[0].kind == "timeout"
    assert results[0].http_status is None


def test_retries_notify_once(make_producer, service):
    service.status = 503
    service.body = b'{"errorCode":"InternalServerError","errorMessage":"retry later"}'
    producer, writer = make_producer(max_attempts=2, base_backoff=0.001, max_backoff=0.001)
    results = []
    writer.send(Log([]), on_delivery=results.append)
    producer.close()
    assert len(service.requests) == 2
    assert len(results) == 1 and results[0].http_status == 503


def test_flush_excludes_callback_close_waits_and_releases_gil(make_producer):
    producer, writer = make_producer()
    entered, release, finished = threading.Event(), threading.Event(), threading.Event()
    def callback(error):
        entered.set()
        release.wait(5)
        finished.set()
    try:
        writer.send(Log([]), on_delivery=callback)
        assert entered.wait(5)
        producer.flush()
        assert not finished.is_set()
        # The Python thread can wake and run while close is blocked in Rust.
        thread = threading.Timer(0.05, release.set)
        thread.start()
        producer.close()
        thread.join(2)
        assert finished.is_set()
    finally:
        release.set()


def test_flush_waits_for_delivery_and_releases_gil(make_producer, service):
    service.release.clear()
    producer, writer = make_producer()
    results = []
    writer.send(Log([]), on_delivery=results.append)
    assert service.received.wait(5)
    timer = threading.Timer(0.05, service.release.set)
    timer.start()
    producer.flush()
    timer.join(2)
    producer.close()
    assert results == [None]


def test_rejection_releases_callback_without_invoking(make_producer):
    producer, writer = make_producer()
    producer.close()
    calls = []
    class Callback:
        def __call__(self, error):
            calls.append(error)
    callback = Callback()
    ref = weakref.ref(callback)
    log = Log([("message", "original")])
    with pytest.raises(ProducerClosedError):
        writer.send(log, on_delivery=callback)
    del callback
    gc.collect()
    assert ref() is None
    assert calls == [] and log.contents == [("message", "original")]
    producer.close()
    producer.close()


def test_capacity_rejection_and_retry(make_producer, service):
    service.release.clear()
    producer, writer = make_producer(buffer_bytes=1)
    log, calls = Log([("message", "held in flight")]), []
    writer.send(log)
    assert service.received.wait(5)
    # The soft pressure flag is refreshed by the Rust runtime tick.
    deadline = time.monotonic() + 3
    accepted = 0
    while True:
        try:
            writer.send(log, on_delivery=calls.append)
            accepted += 1
        except EnqueueFullError:
            break
        assert time.monotonic() < deadline
        time.sleep(0.01)
    assert calls == []
    class RejectedCallback:
        def __call__(self, error):
            raise AssertionError("rejected callbacks must never run")
    callback = RejectedCallback()
    ref = weakref.ref(callback)
    with pytest.raises(EnqueueFullError):
        writer.send(log, on_delivery=callback)
    del callback
    gc.collect()
    assert ref() is None
    service.release.set()
    producer.flush()
    # Pressure may take another tick to clear; retry keeps the same input.
    while True:
        try:
            writer.send(log, on_delivery=calls.append)
            break
        except EnqueueFullError:
            assert time.monotonic() < deadline
            time.sleep(0.01)
    producer.close()
    assert calls == [None] * (accepted + 1)
    assert log.contents == [("message", "held in flight")]


def test_multithreaded_send_and_callback_reentry(make_producer):
    producer, writer = make_producer()
    results, reentered = [], threading.Event()
    def callback(error):
        writer.send(Log([("nested", "send")]))
        results.append(error)
        reentered.set()
    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(lambda _: writer.send(Log([]), on_delivery=callback), range(40)))
    assert reentered.wait(5)
    # Wait until all reentrant sends have finished before stopping admission.
    deadline = time.monotonic() + 5
    while len(results) < 40:
        assert time.monotonic() < deadline
        time.sleep(0.01)
    producer.flush()
    producer.close()
    assert results == [None] * 40


def test_concurrent_send_and_close(make_producer):
    producer, writer = make_producer()
    results = []
    started = threading.Event()
    def send_loop():
        accepted = 0
        for _ in range(500):
            try:
                writer.send(Log([]), on_delivery=results.append)
                accepted += 1
                started.set()
            except ProducerClosedError:
                break
        return accepted
    with ThreadPoolExecutor(max_workers=4) as pool:
        sends = [pool.submit(send_loop) for _ in range(2)]
        assert started.wait(5)
        closes = [pool.submit(producer.close) for _ in range(2)]
        accepted = sum(future.result() for future in sends)
        for future in closes:
            future.result()
    assert results == [None] * accepted


def test_writer_outlives_python_producer(service):
    producer = Producer(ProducerConfig(endpoint=service.endpoint, access_key_id="test-id",
                                       access_key_secret="test-secret", linger=0))
    writer = producer.writer("127", "store")
    del producer
    gc.collect()
    done = threading.Event()
    writer.send(Log([]), on_delivery=lambda error: done.set())
    assert done.wait(5)


@pytest.mark.parametrize("option,value", [("callback_capacity", 0), ("max_attempts", 0),
    ("batch_size_threshold", 9 * 1024 * 1024), ("delivery_timeout", 0), ("linger", float("nan")),
    ("user_agent", "bad\r\nheader"), ("compression", "gzip"), ("endpoint", ""), ("access_key_id", "")])
def test_invalid_config(option, value):
    options = dict(endpoint="cn-hangzhou.log.aliyuncs.com", access_key_id="test-id", access_key_secret="test-secret")
    options[option] = value
    with pytest.raises(ValueError):
        Producer(ProducerConfig(**options))


def test_config_repr_redacts_credentials():
    config = ProducerConfig(endpoint="cn-hangzhou.log.aliyuncs.com", access_key_id="private-id",
                            access_key_secret="private-secret", security_token="private-token")
    assert all(value not in repr(config) for value in ["private-id", "private-secret", "private-token"])


@pytest.mark.parametrize("explicit_close", [True, False])
def test_interpreter_shutdown(service, explicit_close):
    script = f'''
import time
from aliyun_log_producer import Producer, ProducerConfig, Log
p = Producer(ProducerConfig(endpoint={service.endpoint!r}, access_key_id="test-id",
    access_key_secret="test-secret", linger=0, batch_count_threshold=1, ))
w = p.writer("127", "store")
def callback(error):
    time.sleep(0.001)
for _ in range(100):
    w.send(Log([]), on_delivery=callback)
if {explicit_close!r}:
    p.close()
else:
    time.sleep(0.005)
'''
    # Multiple exits exercise finalization while native workers call into Python.
    for _ in range(5):
        result = subprocess.run([sys.executable, "-c", script], capture_output=True, text=True, timeout=10)
        assert result.returncode == 0, result.stderr
        assert "Fatal Python error" not in result.stderr


@pytest.mark.parametrize("timestamp,nanoseconds", [(1700000000, 123456789), (0, 0), (2**32 - 1, None)])
def test_dict_send_matches_log_wire_and_snapshots_input(make_producer, service, timestamp, nanoseconds):
    import lz4.block
    producer, writer = make_producer(compression="lz4", generate_pack_id=False, batch_count_threshold=1)
    contents = {"message": "hello", "unicode": "中文", "empty": ""}
    expected = list(contents.items())
    delivered = []
    writer.send(contents, time=timestamp, time_ns=nanoseconds,
                source="dict-source", topic="dict-topic", on_delivery=delivered.append)
    contents.clear()
    contents["message"] = "changed after send"
    producer.flush()
    writer.send(Log(expected, time=timestamp, time_ns=nanoseconds),
                source="dict-source", topic="dict-topic")
    producer.close()
    assert delivered == [None]
    assert len(service.requests) == 2
    bodies = [lz4.block.decompress(body, uncompressed_size=int(headers["x-log-bodyrawsize"]))
              for _, headers, body in service.requests]
    assert bodies[0] == bodies[1]


def test_dict_send_accepts_current_time_empty_and_keyword_input(make_producer):
    producer, writer = make_producer()
    results = []
    writer.send({"message": "now"}, on_delivery=results.append)
    writer.send(log={}, on_delivery=results.append)
    writer.send({"message": "explicit fraction"}, time_ns=42, on_delivery=results.append)
    producer.close()
    assert results == [None] * 3


@pytest.mark.parametrize("contents,options,error", [
    ({"key": 123}, {}, TypeError),
    ({123: "value"}, {}, TypeError),
    ([("key", "value")], {}, TypeError),
    ({}, {"time": -1}, OverflowError),
    ({}, {"time": 2**32}, OverflowError),
    ({}, {"time_ns": -1}, OverflowError),
    ({}, {"time_ns": 1_000_000_000}, ValueError),
    ({}, {"time": 1.5}, TypeError),
    (Log([], time=0), {"time": 1}, TypeError),
    (Log([], time=0), {"time_ns": 1}, TypeError),
])
def test_dict_send_invalid_input_is_not_admitted(make_producer, service, contents, options, error):
    producer, writer = make_producer()
    results = []
    with pytest.raises(error):
        writer.send(contents, on_delivery=results.append, **options)
    producer.close()
    assert results == []
    assert service.requests == []


def test_dict_send_after_close_preserves_input_without_callback(make_producer):
    producer, writer = make_producer()
    producer.close()
    contents, results = {"message": "retryable"}, []
    with pytest.raises(ProducerClosedError):
        writer.send(contents, time=1700000000, on_delivery=results.append)
    assert contents == {"message": "retryable"}
    assert results == []


@pytest.mark.parametrize("error_type", [EnqueueFullError, ProducerClosedError])
def test_producer_errors_share_a_catchable_base(error_type):
    with pytest.raises(ProducerError):
        raise error_type("producer operation failed")
