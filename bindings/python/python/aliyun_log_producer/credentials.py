"""Structural typing for synchronous application-defined credentials providers."""
from typing import Protocol

from ._native import Credentials


class CredentialsProvider(Protocol):
    """Return one snapshot; temporary credentials must carry their real expiry.

    No inheritance is required. Calls run on a binding-owned worker thread.
    Implementations performing I/O must set their own finite timeouts.
    """

    def get_credentials(self) -> Credentials: ...


def _run_fetch(provider, completion):
    # Keep application code on a Python-owned daemon thread, with no native
    # extension frame spanning a potentially unbounded get_credentials() call.
    completion._enter()
    try:
        try:
            value = provider.get_credentials()
        except BaseException as error:
            completion._failed(type(error).__name__)
        else:
            if isinstance(value, Credentials):
                completion._succeeded(value)
            else:
                completion._failed(None)
    finally:
        completion._exit()


def _start_fetch(provider, completion):
    import threading
    threading.Thread(target=_run_fetch, args=(provider, completion),
                     name="sls-python-credentials", daemon=True).start()
