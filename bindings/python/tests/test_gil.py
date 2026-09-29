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
        linger=0.01, max_attempts=1, **kwargs)
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
    creations = [pool.submit(Producer, cfg) for _ in range(4)]
    assert entered.wait(5)
    release.set()
    producers = [task.result() for task in creations]
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
    # Initial provider calls already completed on the constructing Python threads.
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
