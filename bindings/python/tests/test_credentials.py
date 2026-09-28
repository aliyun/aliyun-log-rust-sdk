import gc
import subprocess
import sys
import threading
import time
import weakref

import pytest

from aliyun_log_producer import Credentials, Producer, ProducerConfig, ProducerError


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


@pytest.mark.parametrize("dynamic", [False, True])
def test_native_constructor_rejects_mismatched_credentials_mode(dynamic):
    from aliyun_log_producer._native import _BaseProducer, _ExternalCredentials

    class Provider:
        def get_credentials(self):
            raise AssertionError("native construction must not fetch Python credentials")

    options = ({"credentials_provider": Provider()} if dynamic else
               {"access_key_id": "id", "access_key_secret": "secret"})
    settings = ProducerConfig(endpoint="example.com", **options)
    external = None if dynamic else _ExternalCredentials(snapshot())
    with pytest.raises(ValueError, match="external_credentials"):
        _BaseProducer(settings, external_credentials=external)


def test_config_is_lazy_and_initial_snapshot_is_loaded_during_construction(service):
    calls = []
    class Provider:
        def __repr__(self):
            raise AssertionError("config must not call provider repr")
        def get_credentials(self):
            calls.append(threading.get_ident())
            return snapshot(security_token="dynamic-token")
    settings = config(service, Provider())
    repr(settings)
    assert calls == []
    with Producer(settings) as producer:
        assert calls == [threading.get_ident()]
        for _ in range(3):
            producer.writer("127", "store").send({"message": "cached"})
            producer.flush()
    assert calls == [threading.get_ident()]
    assert len(service.requests) == 3
    assert all(headers["x-acs-security-token"] == "dynamic-token" for _, headers, _ in service.requests)


def test_refresh_runs_on_delivery_thread_without_waiting_for_new_sends(service):
    refreshed = threading.Event()
    threads = []
    class Provider:
        def get_credentials(self):
            threads.append(threading.get_ident())
            if len(threads) == 1:
                return snapshot("first", security_token="first-token", expires_at=int(time.time()) + 600)
            refreshed.set()
            return snapshot("second")
    with Producer(config(service, Provider())) as producer:
        producer.writer("127", "store").send({})
        producer.flush()
        # Move the internal deadline forward instead of waiting eight minutes.
        producer._credentials._next_refresh = 0
        assert refreshed.wait(5)
        # The fetch signals before publishing; a callback executes after publication.
        delivered = threading.Event()
        callback_threads = []
        def callback(error):
            assert error is None
            callback_threads.append(threading.get_ident())
            delivered.set()
        producer.writer("127", "store").send({}, on_delivery=callback)
        assert delivered.wait(5)
        producer.writer("127", "store").send({})
        producer.flush()
    assert threads == [threading.get_ident(), callback_threads[0]]
    assert service.requests[0][1]["Authorization"].startswith("LOG first:")
    assert service.requests[-1][1]["Authorization"].startswith("LOG second:")
    assert "x-acs-security-token" not in service.requests[-1][1]


def test_shared_config_has_independent_refresh_state_and_releases_provider(service):
    class Provider:
        calls = 0
        def get_credentials(self):
            self.calls += 1
            return snapshot(str(self.calls))
    provider = Provider()
    ref = weakref.ref(provider)
    settings = config(service, provider)
    first, second = Producer(settings), Producer(settings)
    assert provider.calls == 2
    assert first._credentials is not second._credentials
    first.writer("127", "store").send({})
    first.close()
    second.writer("127", "store").send({})
    second.close()
    assert [h["Authorization"].split(":")[0] for _, h, _ in service.requests] == ["LOG 1", "LOG 2"]
    del provider, settings, first, second
    deadline = time.monotonic() + 3
    while ref() is not None and time.monotonic() < deadline:
        gc.collect()
        time.sleep(0.01)
    assert ref() is None


@pytest.mark.parametrize("failure", ["exception", "wrong_type", "expired"])
def test_initial_failure_raises_without_sending_or_leaking_secrets(service, failure):
    class Provider:
        def get_credentials(self):
            if failure == "exception":
                raise RuntimeError("private-secret")
            if failure == "wrong_type":
                return {"secret": "private-secret"}
            return snapshot(expires_at=1)
    with pytest.raises(ProducerError) as error:
        Producer(config(service, Provider()))
    assert "private-secret" not in str(error.value)
    assert not service.requests


def test_refresh_backoff_preserves_snapshot_and_recovers(monkeypatch, caplog):
    from aliyun_log_producer import credentials as module
    clock = [100.0]
    monkeypatch.setattr(module.time, "time", lambda: clock[0])
    monkeypatch.setattr(module.time, "monotonic", lambda: clock[0])
    class Provider:
        calls = 0
        def get_credentials(self):
            self.calls += 1
            if self.calls == 1:
                return snapshot(expires_at=700)
            if self.calls <= 4:
                raise RuntimeError("private-secret")
            return snapshot("recovered")
    provider = Provider()
    manager = module._CredentialsManager(provider)
    published = []
    class Sink:
        def set(self, value):
            published.append(value)
    manager.credentials = Sink()
    assert 579 <= manager._next_refresh <= 580
    for delay in [0.1, 0.2, 15]:
        clock[0] = manager._next_refresh
        manager.refresh_if_due()
        assert manager._next_refresh == pytest.approx(clock[0] + delay)
        assert not published
        manager.refresh_if_due()
    assert provider.calls == 4
    assert "private-secret" not in caplog.text
    clock[0] = manager._next_refresh
    manager.refresh_if_due()
    assert published[0].access_key_id == "recovered"
    assert manager._next_refresh == float("inf")


def test_blocked_refresh_shares_callback_thread_but_not_rust_delivery(service):
    entered, release, returned = threading.Event(), threading.Event(), threading.Event()
    class Provider:
        calls = 0
        def get_credentials(self):
            self.calls += 1
            if self.calls > 1:
                entered.set()
                assert release.wait(5)
                returned.set()
            return snapshot()
    producer = Producer(config(service, Provider()))
    closed = threading.Event()
    callback = threading.Event()
    closer = None
    try:
        producer._credentials._next_refresh = 0
        assert entered.wait(3)
        producer.writer("127", "store").send({}, on_delivery=lambda _: callback.set())
        producer.flush()  # Uses the previous native snapshot without waiting for Python.
        assert len(service.requests) == 1
        assert not callback.is_set()
        closer = threading.Thread(target=lambda: (producer.close(), closed.set()))
        closer.start()
        assert not closed.wait(0.1)
        assert not returned.is_set()
    finally:
        release.set()
        if closer is not None:
            closer.join(5)
        producer.close()
    assert closed.is_set() and returned.is_set() and callback.is_set()


def test_process_exit_after_python_managed_refresh(service):
    script = f"""
import threading
from aliyun_log_producer import Credentials, Producer, ProducerConfig
refreshed = threading.Event()
class Provider:
    calls = 0
    def get_credentials(self):
        self.calls += 1
        if self.calls > 1:
            refreshed.set()
        return Credentials(access_key_id="id", access_key_secret="secret")
p = Producer(ProducerConfig(endpoint={service.endpoint!r}, credentials_provider=Provider()))
p._credentials._next_refresh = 0
assert refreshed.wait(5)
# Exercise interpreter shutdown while the poll thread is still active.
"""
    result = subprocess.run([sys.executable, "-c", script], capture_output=True, text=True, timeout=10)
    assert result.returncode == 0, result.stderr
    assert "Fatal Python error" not in result.stderr
