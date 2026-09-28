"""Structural typing for synchronous application-defined credentials providers."""
from typing import Protocol

from ._native import Credentials


class CredentialsProvider(Protocol):
    """Return one snapshot; temporary credentials must carry their real expiry.

    No inheritance is required. Calls run on a binding-owned worker thread.
    Implementations performing I/O must set their own finite timeouts.
    """

    def get_credentials(self) -> Credentials: ...
