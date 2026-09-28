import threading

import pytest

from aliyun_log_producer import Producer, ProducerConfig, Log


def test_delivery_uses_one_python_thread(service):
    calls = []
    threads = set()
    def delivery(error):
        calls.append(error)
        threads.add(threading.current_thread())
    with Producer(ProducerConfig(
        endpoint=service.endpoint, access_key_id="id", access_key_secret="secret",
        linger=0,
    )) as producer:
        writer = producer.writer("127", "store")
        for _ in range(200):
            writer.send({"message": "poll"}, on_delivery=delivery)
        producer.flush()
    assert calls == [None] * 200
    assert threads == {producer._thread}
    assert not producer._thread.is_alive()
    assert producer._thread.name == "sls-producer-poll"
    producer.close()


def test_callback_capacity_reserved_at_admission(make_producer):
    producer, writer = make_producer(callback_capacity=1)
    entered, release = threading.Event(), threading.Event()
    def slow(error):
        entered.set()
        release.wait(5)
    writer.send(Log([]), on_delivery=slow)
    assert entered.wait(5)
    try:
        from aliyun_log_producer import EnqueueFullError
        with pytest.raises(EnqueueFullError):
            writer.send(Log([]), on_delivery=lambda error: None)
        writer.send(Log([]))
        producer.flush()
    finally:
        release.set()
    producer.close()
