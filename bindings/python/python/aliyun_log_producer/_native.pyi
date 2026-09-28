from collections.abc import Callable, Sequence
from typing import Literal, final, overload
from .credentials import CredentialsProvider

__version__: str

@final
class Credentials:
    def __init__(
        self, *, access_key_id: str, access_key_secret: str,
        security_token: str | None = None, expires_at: int | None = None,
    ) -> None: ...
    @property
    def access_key_id(self) -> str: ...
    @property
    def access_key_secret(self) -> str: ...
    @property
    def security_token(self) -> str | None: ...
    @property
    def expires_at(self) -> int | None: ...

@final
class ProducerConfig:
    @property
    def _credentials_provider(self) -> CredentialsProvider | None: ...
    def __init__(
        self,
        *,
        endpoint: str,
        access_key_id: str | None = None,
        access_key_secret: str | None = None,
        security_token: str | None = None,
        credentials_provider: CredentialsProvider | None = None,
        user_agent: str | None = None,
        compression: Literal["zstd", "lz4"] = "zstd",
        generate_pack_id: bool = True,
        batch_size_threshold: int | None = None,
        batch_count_threshold: int | None = None,
        linger: float | None = None,
        buffer_bytes: int | None = None,
        processing_workers: int | None = None,
        callback_capacity: int | None = None,
        max_attempts: int | None = None,
        base_backoff: float | None = None,
        max_backoff: float | None = None,
        delivery_timeout: float | None = None,
    ) -> None: ...

@final
class Log:
    def __init__(
        self,
        contents: Sequence[tuple[str, str]],
        *,
        time: int | None = None,
        time_ns: int | None = None,
    ) -> None: ...
    @property
    def time(self) -> int: ...
    @property
    def time_ns(self) -> int | None: ...
    @property
    def contents(self) -> list[tuple[str, str]]: ...

@final
class _BaseProducer:
    def __init__(self, config: ProducerConfig, *, external_credentials: _ExternalCredentials | None) -> None: ...
    def _writer(self, project: str, logstore: str, _owner: object | None = None) -> LogstoreWriter: ...
    def _flush(self) -> None: ...
    def _begin_close(self) -> None: ...
    def _is_closed(self) -> bool: ...
    def _wait_closed(self) -> None: ...
    def _poll(self, timeout: float) -> int: ...

@final
class LogstoreWriter:
    # Created by Producer.writer(), not directly constructible.
    @overload
    def send(
        self,
        log: Log,
        *,
        time: None = None,
        time_ns: None = None,
        source: str = "",
        topic: str = "",
        on_delivery: Callable[[DeliveryError | None], object] | None = None,
    ) -> None: ...
    @overload
    def send(
        self,
        log: dict[str, str],
        *,
        time: int | None = None,
        time_ns: int | None = None,
        source: str = "",
        topic: str = "",
        on_delivery: Callable[[DeliveryError | None], object] | None = None,
    ) -> None: ...

@final
class DeliveryError:
    # Created by the binding for terminal delivery failures; not an exception.
    @property
    def kind(self) -> str: ...
    @property
    def message(self) -> str: ...
    @property
    def http_status(self) -> int | None: ...
    @property
    def error_code(self) -> str | None: ...
    @property
    def request_id(self) -> str | None: ...

class ProducerError(RuntimeError): ...
class EnqueueFullError(ProducerError): ...
class ProducerClosedError(ProducerError): ...

@final
class _ExternalCredentials:
    def __init__(self, initial: Credentials) -> None: ...
    def set(self, credentials: Credentials) -> None: ...
