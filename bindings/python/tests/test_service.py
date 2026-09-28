import socket


def test_loopback_service_does_not_need_reverse_dns(monkeypatch, request):
    def unexpected_lookup(*args, **kwargs):
        raise AssertionError("the local test service must not perform reverse DNS")

    monkeypatch.setattr(socket, "getfqdn", unexpected_lookup)
    monkeypatch.setattr(socket, "gethostbyaddr", unexpected_lookup)
    make_producer = request.getfixturevalue("make_producer")
    producer, writer = make_producer()
    results = []
    writer.send({"message": "no DNS required"}, on_delivery=results.append)
    producer.close()
    assert results == [None]
    assert len(request.getfixturevalue("service").requests) == 1
