"""Whether SQLQuack can use a project's local test allowances and overrides, checked at compile."""

from __future__ import annotations

import os
from collections.abc import Mapping
from dataclasses import replace
from pathlib import Path

import sqlquack
from sqlquack import ConvertOptions

from sqlbuild.executor.local._helpers.local_inputs import convert_options, local_inputs
from sqlbuild.executor.local.constants import LOCAL_INPUTS_CHECK_SQL
from sqlbuild.executor.local.models import LocalInputs


def check_local_test_inputs(*, project_dir: Path, allow: Mapping[str, bool]) -> str | None:
    """Why SQLQuack can't use the allowances, then the override files it names; `None` if it can."""

    inputs: LocalInputs = local_inputs(project_dir=project_dir, allow=allow)
    options: ConvertOptions = convert_options(inputs=inputs, schema=None, session={})
    try:
        _ = sqlquack.convert(sql=LOCAL_INPUTS_CHECK_SQL, options=replace(options, overrides=None))
    except sqlquack.SqlQuackError as error:
        return str(error)
    try:
        _ = sqlquack.convert(sql=LOCAL_INPUTS_CHECK_SQL, options=options)
    except sqlquack.SqlQuackError as error:
        return str(error).replace(f"{project_dir}{os.sep}", "")
    return None
