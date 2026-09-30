"""Python-owned event dispatch over the Rust BaseProducer."""
import atexit
import functools
import inspect
import sys
import threading
import weakref

from ._native import _BaseProducer, ProducerConfig, ProducerError, ConfigError, InvalidArgumentError
from .credentials import _CredentialsManager


def _arguments(error_type):
    """Translate call binding failures without catching application exceptions."""
    def decorate(function):
        signature = inspect.signature(function)
        @functools.wraps(function)
        def call(*args, **kwargs):
            try:
                signature.bind(*args, **kwargs)
            except TypeError as error:
                raise error_type(str(error)) from None
            return function(*args, **kwargs)
        return call
    return decorate


# Daemon poll threads must leave native calls and stop dispatching Python
# callbacks before interpreter teardown: a thread re-entering a finalizing
# interpreter aborts (CPython) or crashes when terminated inside
# PyEval_RestoreThread (GraalPy). The hook below therefore joins every poll
# thread while the interpreter is still intact. CPython 3.9+ runs
# threading._register_atexit callbacks after non-daemon threads finish and
# before daemon thread states are destroyed; other interpreters (GraalPy,
# CPython 3.8) terminate daemon threads late enough that plain atexit works.
# The hook is registered when the first producer is created, not at import.
_poll_threads = {}
_poll_threads_lock = threading.Lock()
_exit_hook_registered = False


def _stop_poll_threads():
    with _poll_threads_lock:
        threads = list(_poll_threads.items())
    for thread, (native, stop, _) in threads:
        stop.set()
        native._begin_close()
    for thread, (_, _, join_lock) in threads:
        with join_lock:
            thread.join()


def _register_exit_hook():
    global _exit_hook_registered
    with _poll_threads_lock:
        if _exit_hook_registered:
            return
        _exit_hook_registered = True
        register = getattr(threading, "_register_atexit", None)
        if register is not None:
            register(_stop_poll_threads)
        else:
            atexit.register(_stop_poll_threads)


def _poll(native, stop, credentials):
    # This function owns no reference to the public Producer wrapper.
    try:
        while not stop.is_set() and not native._is_closed():
            if credentials is not None:
                credentials.refresh_if_due()
            if not stop.is_set():
                native._poll(0.1)
    finally:
        with _poll_threads_lock:
            _poll_threads.pop(threading.current_thread(), None)


class Producer:
    """Thread-safe producer with automatic batching, retries and callbacks.

    Obtain writers with writer(project, logstore). Successful send means local
    admission; on_delivery reports the final outcome. Delivery callbacks run
    serially and may send more logs. Retries may duplicate logs.

    Use a with statement or explicitly close before exit. Calling send, flush or
    close from user __del__ methods or finalizers is unsupported.
    """
    @_arguments(ConfigError)
    def __init__(self, config):
        """Obtain initial dynamic credentials and start background workers.

        Initial credential fetch runs once on the calling thread; failure raises
        ProducerError. Credentials are cached and refreshed automatically.
        Invalid configuration raises ConfigError. No asyncio loop is required.
        """
        if not isinstance(config, ProducerConfig):
            raise ConfigError("config must be a ProducerConfig")
        provider = config._credentials_provider
        self._credentials = _CredentialsManager(provider) if provider is not None else None
        self._native = _BaseProducer(
            config,
            external_credentials=self._credentials.credentials if self._credentials is not None else None,
        )
        _register_exit_hook()
        stop = threading.Event()
        self._join_lock = threading.Lock()
        self._thread = threading.Thread(
            target=_poll, args=(self._native, stop, self._credentials),
            name="sls-producer-poll", daemon=True,
        )
        with _poll_threads_lock:
            _poll_threads[self._thread] = (self._native, stop, self._join_lock)
        if sys.implementation.name == "graalpy":
            # GraalPy does not invoke Python __del__, but supports weakrefs.
            weakref.finalize(self, self._native._begin_close)
        try:
            self._thread.start()
        except BaseException as error:
            with _poll_threads_lock:
                _poll_threads.pop(self._thread, None)
            self._native._begin_close()
            if isinstance(error, Exception) and not isinstance(error, MemoryError):
                raise ProducerError("failed to start producer poll thread: {}".format(error)) from error
            raise

    @_arguments(InvalidArgumentError)
    def writer(self, project, logstore):
        """Get a thread-safe writer without checking remote existence or permissions.

        Invalid destination names raise InvalidArgumentError. All writers share this
        producer's endpoint, credentials and lifecycle.
        """
        if not isinstance(project, str) or not isinstance(logstore, str):
            raise InvalidArgumentError("project and logstore must be strings")
        return self._native._writer(project, logstore, self)

    @_arguments(InvalidArgumentError)
    def flush(self):
        """Wait for prior delivery; later sends may continue.

        Does not wait for callbacks or report individual delivery failures.
        Calling from this producer's callback, credentials provider, or a user
        finalizer is unsupported and may deadlock.
        """
        self._native._flush()

    @_arguments(InvalidArgumentError)
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
