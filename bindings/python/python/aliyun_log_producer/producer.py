"""Python-owned event dispatch over the Rust BaseProducer."""
import atexit
import sys
import threading
import weakref

from ._native import _BaseProducer
from .credentials import _CredentialsManager


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


def _poll(native, stop, credentials):
    # This function owns no reference to the public Producer wrapper.
    try:
        while not stop.is_set() and not native._is_closed():
            if credentials is not None:
                credentials.refresh_if_due()
            if not stop.is_set():
                native._poll(0.1)
    finally:
        if sys.implementation.name == "graalpy":
            with _graalpy_lock:
                _graalpy_threads.pop(threading.current_thread(), None)


class Producer:
    """Thread-safe producer with automatic batching, retries and callbacks.

    Obtain writers with writer(project, logstore). Successful send means local
    admission; on_delivery reports the final outcome. Delivery callbacks run
    serially and may send more logs. Retries may duplicate logs.

    Use a with statement or explicitly close before exit. Calling send, flush or
    close from user __del__ methods or finalizers is unsupported.
    """
    def __init__(self, config):
        """Obtain initial dynamic credentials and start background workers.

        Initial credential fetch runs once on the calling thread; failure raises
        ProducerError. Credentials are cached and refreshed automatically.
        Invalid configuration raises ValueError; out-of-range integers may raise
        OverflowError. No asyncio loop is required.
        """
        provider = config._credentials_provider
        self._credentials = _CredentialsManager(provider) if provider is not None else None
        self._native = _BaseProducer(
            config,
            external_credentials=self._credentials.credentials if self._credentials is not None else None,
        )
        stop = threading.Event()
        self._join_lock = threading.Lock()
        self._thread = threading.Thread(
            target=_poll, args=(self._native, stop, self._credentials),
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

    def flush(self):
        """Wait for prior delivery; later sends may continue.

        Does not wait for callbacks or report individual delivery failures.
        Calling from this producer's callback, credentials provider, or a user
        finalizer is unsupported and may deadlock.
        """
        self._native._flush()

    def close(self):
        """Stop all writers and wait for delivery, callbacks and shutdown.

        Safe to call repeatedly. Does not report individual delivery failures.
        Callbacks must return for close to complete. Calling from this producer's
        callback, credentials provider, or a user finalizer is unsupported and
        may deadlock.
        """
        self._native._begin_close()
        try:
            self._native._wait_closed()
        finally:
            # GraalPy 25.0's Thread.join cannot safely stop the same thread
            # concurrently. Serialize only joining, after native shutdown.
            with self._join_lock:
                self._thread.join()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        self.close()

    def __del__(self):
        native = getattr(self, "_native", None)
        if native is not None:
            native._begin_close()  # Never join or execute Python callbacks in __del__.
