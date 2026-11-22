"""The adapter connects in progress, found on the running threads' stacks."""

import sys
from types import FrameType

from sqlbuild.cli.entry.constants import ADAPTER_BASE_CLASS, ADAPTER_BASE_MODULE, CONNECT_METHOD


def connects_in_progress() -> list[FrameType]:
    """Return each running frame of an adapter's `connect`, without importing anything."""

    adapter: object = getattr(sys.modules.get(ADAPTER_BASE_MODULE), ADAPTER_BASE_CLASS, None)
    if not isinstance(adapter, type):
        return []
    connecting: list[FrameType] = []
    for top in sys._current_frames().values():
        frame: FrameType | None = top
        while frame is not None:
            if frame.f_code.co_name == CONNECT_METHOD and isinstance(
                frame.f_locals.get("self"), adapter
            ):
                connecting.append(frame)
            frame = frame.f_back
    return connecting
