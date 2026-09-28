"""Python-owned refresh policy for synchronous credentials providers."""
import logging
import random
import time
from typing import Protocol

from ._native import Credentials, ProducerError, _ExternalCredentials


class CredentialsProvider(Protocol):
    """Return Credentials; no inheritance is required.

    Producer fetches once during construction, caches the result, and refreshes
    before expires_at with automatic retries. Omit expires_at for credentials
    that do not need automatic refresh.

    I/O must have finite timeouts; a blocked refresh delays callbacks and close.
    Shared providers must support concurrent calls. Calling the producer's flush
    or close here is unsupported and may deadlock.
    """

    def get_credentials(self) -> Credentials: ...


_logger = logging.getLogger(__name__)


class _CredentialsManager:
    """One producer's refresh state. Rust only receives the shared snapshot."""

    def __init__(self, provider):
        self._provider = provider
        self._failures = 0
        self._next_refresh = float("inf")
        initial = self._fetch()
        self.credentials = _ExternalCredentials(initial)
        self._schedule(initial)

    def _fetch(self):
        try:
            value = self._provider.get_credentials()
        except Exception as error:
            # Provider exception messages and tracebacks may contain secrets.
            raise ProducerError(
                "get_credentials() raised {}; exception details omitted".format(type(error).__name__)
            ) from None
        if not isinstance(value, Credentials):
            raise ProducerError("get_credentials() must return Credentials")
        if value.expires_at is not None and value.expires_at <= time.time():
            raise ProducerError("get_credentials() returned expired credentials")
        return value

    def _schedule(self, value):
        expires_at = value.expires_at
        if expires_at is None:
            self._next_refresh = float("inf")
        else:
            remaining = max(0, expires_at - time.time())
            advance = min(remaining / 5, 300)
            # One serial fetch per producer: a small jitter is enough to spread
            # synchronized refreshes without adding another scheduling window.
            jitter = random.uniform(0, min(1.0, remaining / 10))
            self._next_refresh = time.monotonic() + max(0.1, remaining - advance - jitter)

    def refresh_if_due(self):
        if time.monotonic() < self._next_refresh:
            return
        try:
            value = self._fetch()
        except BaseException as error:
            # Preserve the last published snapshot. Never retain traceback/provider
            # frames or let a user exception terminate delivery event dispatch.
            self._failures += 1
            delay = (0.1, 0.2, 15.0)[min(self._failures, 3) - 1]
            if self._failures >= 3:
                self._failures = 0
                _logger.warning("Credentials refresh failed (%s); retrying in 15 seconds",
                                type(error).__name__)
            self._next_refresh = time.monotonic() + delay
        else:
            self.credentials.set(value)
            self._failures = 0
            self._schedule(value)
