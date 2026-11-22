"""Stable constants for the CLI process entrypoint."""

NATIVE_ALLOCATOR_PURGE_DELAY_VARIABLE: str = "MIMALLOC_PURGE_DELAY"
NATIVE_ALLOCATOR_PURGE_DELAY_MILLISECONDS: str = "10"
OUTPUT_STREAM_ENCODING: str = "utf-8"
OUTPUT_STREAM_ERRORS: str = "backslashreplace"
INTERRUPTED_EXIT_CODE: int = 130
INTERRUPTED_LINE: str = "error: interrupted (Ctrl+C)"
