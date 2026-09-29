import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from socketserver import TCPServer

import pytest

from aliyun_log_producer import Producer, ProducerConfig


@pytest.fixture
def service(monkeypatch):
    # Project 127 + endpoint 0.0.1 becomes 127.0.0.1 in the real client.
    monkeypatch.setenv("NO_PROXY", "127.0.0.1,localhost")
    monkeypatch.setenv("no_proxy", "127.0.0.1,localhost")
    state = type("Service", (), {})()
    state.status = 200
    state.body = b""
    state.requests = []
    state.received = threading.Event()
    state.release = threading.Event()
    state.release.set()

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):
            body = self.rfile.read(int(self.headers["Content-Length"]))
            state.requests.append((self.path, self.headers, body))
            state.received.set()
            if not state.release.wait(10):
                return
            self.send_response(state.status)
            self.send_header("Content-Length", str(len(state.body)))
            self.send_header("x-log-requestid", "python-test-request")
            self.end_headers()
            try:
                self.wfile.write(state.body)
            except (BrokenPipeError, ConnectionResetError):
                pass  # Expected when a delivery deadline expires.

        def log_message(self, *args):
            pass

    class Server(ThreadingHTTPServer):
        # The producer opens concurrent connections. HTTPServer's tiny default
        # listen backlog otherwise tests TCP retransmission delays, not bindings.
        request_queue_size = 1024

        def server_bind(self):
            # HTTPServer resolves its own address with getfqdn(), which can
            # stall on CI hosts. This loopback fixture needs no DNS name.
            TCPServer.server_bind(self)
            self.server_name, self.server_port = self.server_address

    server = Server(("127.0.0.1", 0), Handler)
    state.endpoint = f"http://0.0.1:{server.server_port}"
    thread = threading.Thread(target=lambda: server.serve_forever(poll_interval=0.01))
    thread.start()
    yield state
    state.release.set()
    server.shutdown()
    server.server_close()
    thread.join(5)


@pytest.fixture
def make_producer(service):
    producers = []

    def make(**overrides):
        options = dict(
            endpoint=service.endpoint,
            access_key_id="test-id",
            access_key_secret="test-secret",
            processing_workers=1,
            max_attempts=1,
            linger=0,
        )
        options.update(overrides)
        producer = Producer(ProducerConfig(**options))
        producers.append(producer)
        return producer, producer.writer("127", "store")

    yield make
    service.release.set()
    for producer in producers:
        producer.close()
