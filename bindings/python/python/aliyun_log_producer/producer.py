"""Python-owned event dispatch over the Rust BaseProducer."""
import threading

from ._native import _BaseProducer, ProducerError


def _poll(native):
    # This function owns no reference to the public Producer wrapper.
    while not native._is_closed():
        native._poll(0.1)


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
        self._thread = threading.Thread(
            target=_poll, args=(self._native,),
            name="sls-producer-poll", daemon=True,
        )
        try:
            self._thread.start()
        except BaseException:
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
            self._thread.join()  # Also releases the GIL while waiting.

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        self.close()

    def __del__(self):
        native = getattr(self, "_native", None)
        if native is not None:
            native._begin_close()  # Never join or execute Python callbacks in __del__.
