"""Stable constants for the CLI process entrypoint."""

NATIVE_ALLOCATOR_PURGE_DELAY_VARIABLE: str = "MIMALLOC_PURGE_DELAY"
NATIVE_ALLOCATOR_PURGE_DELAY_MILLISECONDS: str = "10"
OUTPUT_STREAM_ENCODING: str = "utf-8"
OUTPUT_STREAM_ERRORS: str = "backslashreplace"
INTERRUPTED_EXIT_CODE: int = 130
TERMINATED_EXIT_CODE: int = 143
INTERRUPTED_LINE: str = "error: interrupted (Ctrl+C)"
INTERRUPT_GRACE_SECONDS: float = 0.6
INTERRUPT_POLL_SECONDS: float = 0.05
FORCED_EXIT_WAIT_SECONDS: float = 2.0
ADAPTER_BASE_MODULE: str = "sqlbuild.adapter.contract.classes.base_adapter"
ADAPTER_BASE_CLASS: str = "BaseAdapter"
CONNECT_METHOD: str = "connect"
WINDOWS_PLATFORM: str = "win32"
