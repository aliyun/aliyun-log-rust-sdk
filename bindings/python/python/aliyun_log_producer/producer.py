"""Python-owned event dispatch over the Rust BaseProducer."""
import atexit
import sys
import threading
import weakref

from ._native import _BaseProducer, ProducerError


# GraalPy terminates daemon threads at context shutdown. They must first leave
# native calls: killing a thread inside PyEval_RestoreThread can crash GraalPy.
_graalpy_threads = {}
_graalpy_lock = threading.Lock()


def _stop_graalpy_threads():
    with _graalpy_lock:
        threads = list(_graalpy_threads.items())
    for thread, (native, stop, _) in threads:
        stop.set()
        native._begin_close()
    for thread, (_, _, join_lock) in threads:
        with join_lock:
            thread.join()


if sys.implementation.name == "graalpy":
    atexit.register(_stop_graalpy_threads)


def _poll(native, stop):
    # This function owns no reference to the public Producer wrapper.
    try:
        while not stop.is_set() and not native._is_closed():
            native._poll(0.1)
    finally:
        if sys.implementation.name == "graalpy":
            with _graalpy_lock:
                _graalpy_threads.pop(threading.current_thread(), None)


class Producer:
    """Thread-safe producer with automatic batching, retries and callbacks.

    Construction validates local configuration and starts background workers;
    it makes no service requests or credential fetches. Obtain writers with
    writer(project, logstore). Successful send means local admission; use
    on_delivery to receive the final delivery outcome. Callbacks run serially
    on one Python-created thread, and retries may duplicate logs.

    flush waits for prior delivery only. close stops every writer and waits
    indefinitely for delivery, callbacks and shutdown. Neither aggregates
    individual delivery failures. Use a with statement or explicitly close
    before exit. Waiting releases the GIL and requires no asyncio loop.

    Runtime failures derive from ProducerError; invalid configuration raises
    ValueError (out-of-range integers may raise OverflowError).
    """
    def __init__(self, config):
        self._native = _BaseProducer(config)
        stop = threading.Event()
        self._join_lock = threading.Lock()
        self._thread = threading.Thread(
            target=_poll, args=(self._native, stop),
            name="sls-producer-poll", daemon=True,
        )
        if sys.implementation.name == "graalpy":
            # GraalPy does not invoke Python __del__, but supports weakrefs.
            weakref.finalize(self, self._native._begin_close)
            with _graalpy_lock:
                _graalpy_threads[self._thread] = (self._native, stop, self._join_lock)
        try:
            self._thread.start()
        except BaseException:
            with _graalpy_lock:
                _graalpy_threads.pop(self._thread, None)
            self._native._begin_close()
            raise

    def writer(self, project, logstore):
        """Get a thread-safe writer without checking remote existence or permissions.

        Invalid destination names raise ValueError. All writers share this
        producer's endpoint, credentials and lifecycle.
        """
        return self._native._writer(project, logstore, self)

    def _check_wait(self):
        self._native._check_wait()
        if threading.current_thread() is self._thread:
            raise ProducerError("cannot wait for this producer from its callback")

    def flush(self):
        """Wait for delivery accepted before this call; callbacks may still run."""
        self._check_wait()
        self._native._flush()

    def close(self):
        """Stop admission, drain callbacks on the poll thread, and join it."""
        self._check_wait()
        self._native._begin_close()
        try:
            self._native._wait_closed()  # Releases the GIL during the wait.
        finally:
            # GraalPy 25.0's Thread.join cannot safely stop the same thread
            # concurrently. Serialize only joining, after native shutdown.
            with self._join_lock:
                self._thread.join()  # Also releases the GIL while waiting.

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        self.close()

    def __del__(self):
        native = getattr(self, "_native", None)
        if native is not None:
            native._begin_close()  # Never join or execute Python callbacks in __del__.
