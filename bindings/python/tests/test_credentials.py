import gc
import subprocess
import sys
import threading
import time
import weakref

import pytest

from aliyun_log_producer import Credentials, Producer, ProducerConfig


def snapshot(identifier="dynamic-id", **kwargs):
    return Credentials(access_key_id=identifier, access_key_secret="private-secret", **kwargs)


def config(service, provider, **kwargs):
    return ProducerConfig(endpoint=service.endpoint, credentials_provider=provider,
                          max_attempts=1, linger=0, batch_count_threshold=1, **kwargs)


def test_credentials_snapshot_fields_and_redacted_repr():
    value = snapshot(security_token="private-token", expires_at=1700000000)
    assert value.access_key_id == "dynamic-id"
    assert value.access_key_secret == "private-secret"
    assert value.security_token == "private-token"
    assert value.expires_at == 1700000000
    assert type(value.expires_at) is int
    assert not any(secret in repr(value) for secret in ["dynamic-id", "private-secret", "private-token"])
    assert snapshot().expires_at is None
    with pytest.raises(AttributeError):
        value.expires_at = 1800000000


@pytest.mark.parametrize("kwargs,error", [
    ({"access_key_id": ""}, ValueError), ({"access_key_secret": ""}, ValueError),
    ({"expires_at": -1}, OverflowError), ({"expires_at": 2**64}, OverflowError),
    ({"expires_at": 2**64 - 1}, ValueError),
    ({"expires_at": float("nan")}, TypeError), ({"expires_at": float("inf")}, TypeError),
    ({"expires_at": 1700000000.0}, TypeError), ({"expires_at": 1700000000.25}, TypeError),
    ({"expires_at": "1700000000"}, TypeError),
])
def test_invalid_snapshot(kwargs, error):
    options = dict(access_key_id="id", access_key_secret="secret")
    options.update(kwargs)
    with pytest.raises(error):
        Credentials(**options)


@pytest.mark.parametrize("expiration", [0, 2**32 + 1])
def test_expiration_integer_roundtrip(expiration):
    value = snapshot(expires_at=expiration)
    assert value.expires_at == expiration
    assert type(value.expires_at) is int


@pytest.mark.parametrize("options", [
    {}, {"access_key_id": "id"}, {"access_key_secret": "secret"},
    {"credentials_provider": object(), "access_key_id": "id"},
    {"credentials_provider": object(), "access_key_secret": "secret"},
    {"credentials_provider": object(), "security_token": "token"},
])
def test_provider_config_requires_one_credentials_source(options):
    with pytest.raises(ValueError):
        ProducerConfig(endpoint="example.com", **options)


def test_provider_is_lazy_cached_and_runs_off_the_calling_thread(service):
    calls, threads = [], []
    class Provider:
        def __repr__(self):
            raise AssertionError("config must not call Python repr")
        def get_credentials(self):
            calls.append(1)
            threads.append(threading.get_ident())
            return snapshot(security_token="dynamic-token")
    settings = config(service, Provider())
    repr(settings)
    producer = Producer(settings)
    writer = producer.writer("127", "store")
    assert calls == []
    results = []
    try:
        for _ in range(3):
            writer.send({"message": "cached"}, on_delivery=results.append)
            producer.flush()
        producer.close()
        assert calls == [1]
        assert threads[0] != threading.get_ident()
        assert results == [None] * 3
        for _, headers, _ in service.requests:
            assert headers["Authorization"].startswith("LOG dynamic-id:")
            assert headers["x-acs-security-token"] == "dynamic-token"
    finally:
        producer.close()


def test_provider_refresh_rotates_one_complete_snapshot(service):
    class Provider:
        calls = 0
        def get_credentials(self):
            self.calls += 1
            self.expiration = int(time.time()) + (2 if self.calls == 1 else 60)
            return snapshot(f"id-{self.calls}", security_token=f"token-{self.calls}",
                            expires_at=self.expiration)
    provider = Provider()
    producer = Producer(config(service, provider))
    writer = producer.writer("127", "store")
    try:
        writer.send({"message": "first"})
        producer.flush()
        time.sleep(max(0, provider.expiration - time.time()) + 0.05)
        writer.send({"message": "second"})
        producer.close()
        assert provider.calls == 2
        for index, (_, headers, _) in enumerate(service.requests, 1):
            assert headers["Authorization"].startswith(f"LOG id-{index}:")
            assert headers["x-acs-security-token"] == f"token-{index}"
    finally:
        producer.close()


def test_shared_config_forwards_independent_provider_calls_and_releases_reference(service):
    entered, both_entered, release = threading.Event(), threading.Event(), threading.Event()
    class Provider:
        calls = 0
        def get_credentials(self):
            self.calls += 1
            entered.set()
            if self.calls == 2:
                both_entered.set()
            assert release.wait(5)
            return snapshot()
    provider = Provider()
    ref = weakref.ref(provider)
    settings = config(service, provider)
    first, second = Producer(settings), Producer(settings)
    results = []
    try:
        first.writer("127", "store").send({}, on_delivery=results.append)
        assert entered.wait(3)
        second.writer("127", "store").send({}, on_delivery=results.append)
        # Sharing a config does not merge the two clients' provider calls.
        assert both_entered.wait(3)
        assert provider.calls == 2
    finally:
        release.set()
        first.close()
        second.close()
    assert results == [None, None]
    assert provider.calls == 2
    del provider, settings, first, second
    deadline = time.monotonic() + 3
    while ref() is not None and time.monotonic() < deadline:
        gc.collect()
        time.sleep(0.01)
    assert ref() is None


def test_timed_out_fetch_does_not_block_later_provider_calls(service):
    entered, release = threading.Event(), threading.Event()
    both_entered = threading.Event()
    finished = [threading.Event(), threading.Event()]
    class Provider:
        calls = 0
        def get_credentials(self):
            index = self.calls
            self.calls += 1
            entered.set()
            if self.calls == 2:
                both_entered.set()
            try:
                assert release.wait(5)
                return snapshot()
            finally:
                finished[index].set()
    provider = Provider()
    settings = config(service, provider, delivery_timeout=0.1)
    first, second = Producer(settings), Producer(settings)
    results = []
    try:
        first.writer("127", "store").send({}, on_delivery=results.append)
        assert entered.wait(3)
        first.close()
        second.writer("127", "store").send({}, on_delivery=results.append)
        assert both_entered.wait(3)
        second.close()
        assert provider.calls == 2
        assert len(results) == 2
        assert all(result.kind == "timeout" for result in results)
        assert service.requests == []
    finally:
        release.set()
        assert all(event.wait(3) for event in finished)
        first.close()
        second.close()


@pytest.mark.parametrize("failure", ["exception", "wrong_type", "expired"])
def test_provider_failures_are_delivery_errors_without_secret_leaks(service, failure):
    class Provider:
        def get_credentials(self):
            if failure == "exception":
                raise RuntimeError("private-secret must not be exposed")
            if failure == "wrong_type":
                return {"access_key_secret": "private-secret"}
            return snapshot(expires_at=1)
    producer = Producer(config(service, Provider()))
    results = []
    try:
        producer.writer("127", "store").send({}, on_delivery=results.append)
        producer.close()
        assert len(results) == 1
        assert results[0].kind == "credentials"
        assert "private-secret" not in results[0].message
        assert service.requests == []
    finally:
        producer.close()


def test_exit_does_not_wait_for_blocked_python_provider(service):
    script = f'''
import threading
from aliyun_log_producer import Producer, ProducerConfig
entered = threading.Event()
class Provider:
    def get_credentials(self):
        entered.set()
        threading.Event().wait(60)
p = Producer(ProducerConfig(endpoint={service.endpoint!r}, credentials_provider=Provider(),
                           linger=0, delivery_timeout=0.05, max_attempts=1))
p.writer("127", "store").send({{}})
assert entered.wait(3)
p.close()
'''
    result = subprocess.run([sys.executable, "-c", script], capture_output=True, text=True, timeout=8)
    assert result.returncode == 0, result.stderr
    assert "Fatal Python error" not in result.stderr
