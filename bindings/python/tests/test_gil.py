"""Run lock-order regressions in children so a GIL hang cannot hang pytest."""

import subprocess
import sys
import textwrap

import pytest


def run_child(service, script):
    prelude = f"""
import faulthandler
import threading
from concurrent.futures import ThreadPoolExecutor
from aliyun_log_producer import (
    Credentials, Producer, ProducerConfig, ProducerClosedError,
)
faulthandler.dump_traceback_later(10, exit=True)
endpoint = {service.endpoint!r}
def config(**kwargs):
    return ProducerConfig(endpoint=endpoint,
        linger=0, max_attempts=1, **kwargs)
"""
    result = subprocess.run(
        [sys.executable, "-c", textwrap.dedent(prelude) + textwrap.dedent(script)],
        capture_output=True, text=True, timeout=15,
    )
    assert result.returncode == 0, result.stdout + result.stderr


@pytest.mark.parametrize("processing_workers", [1, 4])
def test_concurrent_credentials_callbacks_reentry_and_close(service, processing_workers):
    run_child(service, f"""
entered, release, completed = threading.Event(), threading.Event(), threading.Event()
counts_lock = threading.Lock()
calls, delivered, failures = [], [], []
active_fetches, overlap = [], []
class Provider:
    def get_credentials(self):
        with counts_lock:
            calls.append(threading.get_ident())
            active_fetches.append(1)
            overlap.append(len(active_fetches))
            if len(active_fetches) >= 2:
                entered.set()
        try:
            assert release.wait(5)
            return Credentials(access_key_id="id", access_key_secret="secret")
        finally:
            with counts_lock:
                active_fetches.pop()

cfg = config(credentials_provider=Provider(), processing_workers={processing_workers})
with ThreadPoolExecutor(max_workers=8) as pool:
    producers = list(pool.map(lambda _: Producer(cfg), range(4)))
    writers = [p.writer("127", "store") for p in producers]
    def send(index):
        writer = writers[index]
        def callback(error):
            try:
                assert error is None
                # Registry and admission locks are acquired after detaching even
                # when this call originates inside a batched Python callback.
                nested = producers[index].writer("127", "nested")
                nested.send({{"nested": "send"}})
            except BaseException as exc:
                failures.append(repr(exc))
            finally:
                with counts_lock:
                    delivered.append(index)
                    if len(delivered) == 320:
                        completed.set()
        for _ in range(80):
            writer.send({{"message": "hello"}}, on_delivery=callback)
        producers[index].flush()
    sends = [pool.submit(send, i) for i in range(4)]
    assert entered.wait(5)
    # Wakes the Python provider while Rust flush calls are waiting for it.
    release.set()
    for task in sends:
        task.result()
    assert completed.wait(5)
    # Multiple waiters on the same producer must not block callback completion
    # or the native runtime's thread joins while holding the GIL.
    closes = [pool.submit(p.close) for p in producers for _ in range(2)]
    for task in closes:
        task.result()
assert not failures, failures
assert len(delivered) == 320
# Fetches are independently forwarded, even when producers share one config.
assert calls and max(overlap) >= 2, overlap
assert not active_fetches
""")


def test_callback_close_self_wait_is_rejected_then_shutdown_completes(service):
    run_child(service, """
p = Producer(config(access_key_id="id", access_key_secret="secret", ))
results = []
def callback(error):
    assert error is None
    try:
        p.close()
    except RuntimeError:
        results.append("self-wait rejected")
p.writer("127", "store").send({}, on_delivery=callback)
p.close()
assert results == ["self-wait rejected"], results
""")


def test_provider_flush_self_wait_is_rejected_then_delivery_completes(service):
    run_child(service, """
results = []
class Provider:
    def get_credentials(self):
        try:
            p.flush()
        except RuntimeError:
            results.append("self-wait rejected")
        return Credentials(access_key_id="id", access_key_secret="secret")
p = Producer(config(credentials_provider=Provider(), ))
p.writer("127", "store").send({}, on_delivery=results.append)
p.close()
assert results == ["self-wait rejected", None], results
""")


@pytest.mark.skipif(sys.implementation.name == "graalpy",
                    reason="GraalPy collects pure-Python objects without invoking __del__; weakref release is tested separately")
def test_callback_destructor_can_reenter_after_delivery_and_rejection(service):
    run_child(service, """
import gc
import time
def wait_for_collection(event):
    deadline = time.monotonic() + 5
    while not event.is_set() and time.monotonic() < deadline:
        gc.collect()
        event.wait(0.01)
    assert event.is_set()
p = Producer(config(access_key_id="id", access_key_secret="secret", ))
w = p.writer("127", "store")
destroyed = threading.Event()
results = []
class Delivered:
    def __call__(self, error):
        results.append(error)
    def __del__(self):
        # Decref can run arbitrary Python. It must not occur with admission or
        # submission locks held, whether immediate or deferred by PyO3.
        w.send({"from": "destructor"})
        destroyed.set()
def submit_delivery():
    # End the allocating frame before requesting GC: a tracing interpreter may
    # keep temporary C-extension arguments alive until their frame returns.
    w.send({}, on_delivery=Delivered())
submit_delivery()
wait_for_collection(destroyed)
p.close()
assert results == [None], results

destroyed.clear()
class Rejected:
    def __call__(self, error):
        results.append("unexpected callback")
    def __del__(self):
        p.close()
        try:
            w.send({})
        except ProducerClosedError:
            destroyed.set()
def submit_rejection():
    try:
        w.send({}, on_delivery=Rejected())
    except ProducerClosedError:
        pass
submit_rejection()
wait_for_collection(destroyed)
assert results == [None], results
""")
