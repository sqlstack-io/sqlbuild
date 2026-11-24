"""Read command-output sink declarations."""

from collections.abc import Callable

from sqlbuild.python_nodes.main.read_attached_definition import read_attached_definition
from sqlbuild.runtime.event_exporting.constants import COMMAND_OUTPUT_SINK_ATTRIBUTE
from sqlbuild.runtime.output_capture.models import CommandOutputEventSinkDefinition


def get_command_output_event_sink_definition(
    function: Callable[..., object],
) -> CommandOutputEventSinkDefinition | None:
    """Return command-output sink metadata attached to a function."""

    return read_attached_definition(
        function=function,
        attribute_name=COMMAND_OUTPUT_SINK_ATTRIBUTE,
        definition_type=CommandOutputEventSinkDefinition,
    )
