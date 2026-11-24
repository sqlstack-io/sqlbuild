"""Real playground workflow for CLI previews."""

from __future__ import annotations

import os
import shutil
import signal
import subprocess
import tempfile
from collections.abc import Callable
from pathlib import Path
from types import FrameType

from scripts.cli_preview.exceptions import PreviewSetupError
from scripts.cli_preview.models import PreviewScene
from sqlbuild._native import supports_color

_PAYMENTS_MODEL_PATH: Path = Path("sql/models/staging/stg_payments.sql")
_QUERY_MARKER: str = "  amount_cents,\n"
_CHANGED_QUERY_MARKER: str = (
    "  CAST(CASE WHEN payment_method = 'card' THEN 'preview'"
    " ELSE CAST(amount_cents AS VARCHAR) END AS INTEGER) AS amount_cents,\n"
)

type _SignalHandler = Callable[[int, FrameType | None], object] | int | None


def preview_scenes() -> tuple[PreviewScene, ...]:
    """Return the maintained real-output preview scenes."""

    return (
        PreviewScene(
            name="compile",
            description="compile summary and artifact rows",
            command=("compile",),
        ),
        PreviewScene(
            name="plan",
            description="direct first-run plan",
            command=("plan",),
        ),
        PreviewScene(
            name="plan-changed",
            description="direct query-change plan with a SQL diff",
            setup_commands=(("build",),),
            mutate_payments=True,
            command=("plan",),
        ),
        PreviewScene(
            name="build-success",
            description="successful build progress and completion",
            command=("build",),
        ),
        PreviewScene(
            name="build-failure",
            description="inline test execution error and failure footer",
            setup_commands=(("build",),),
            mutate_payments=True,
            command=("build",),
            expected_return_codes=(1,),
        ),
        PreviewScene(
            name="test-success",
            description="successful grouped SQL test output",
            command=("test",),
        ),
        PreviewScene(
            name="test-error",
            description="nested SQL test execution error",
            setup_commands=(("build",),),
            mutate_payments=True,
            command=("test",),
            expected_return_codes=(1,),
        ),
        PreviewScene(
            name="audit",
            description="audit rows and semantic summary counts",
            setup_commands=(("build",),),
            command=("audit",),
        ),
        PreviewScene(
            name="scenario",
            description="scenario execution and expectations",
            command=("scenario", "test"),
        ),
        PreviewScene(
            name="lineage",
            description="model lineage tree",
            command=("lineage", "fact_orders", "--direction", "both"),
        ),
    )


def execute_preview_scene(*, scene: PreviewScene, no_color: bool, keep: bool) -> int:
    """Create a temporary playground and render one real CLI scene."""

    workspace: Path = Path(tempfile.mkdtemp(prefix=f"sqlbuild-cli-preview-{scene.name}-"))
    project_dir: Path = workspace / "project"
    try:
        sqb: str = _resolve_sqb_executable()
        environment: dict[str, str] = dict(os.environ)
        environment.pop("VIRTUAL_ENV", None)
        _prepare_project(
            sqb=sqb,
            project_dir=project_dir,
            scene=scene,
            environment=environment,
        )
        _write_scene_header(scene=scene, no_color=no_color)
        return_code: int = _run_sqb(
            sqb=sqb,
            project_dir=project_dir,
            command=scene.command,
            environment=environment,
            no_color=no_color,
            quiet=False,
        )
        if return_code < 0:
            return 128 - return_code
        if return_code not in scene.expected_return_codes:
            return return_code or 1
        return 0
    finally:
        if keep:
            print(f"\nPreview project kept at {project_dir}")
        else:
            shutil.rmtree(path=workspace, ignore_errors=True)


def _prepare_project(
    *,
    sqb: str,
    project_dir: Path,
    scene: PreviewScene,
    environment: dict[str, str],
) -> None:
    scaffold_return_code: int = _run_process(
        args=(sqb, "playground", str(project_dir), "--template", scene.template),
        environment=environment,
        quiet=True,
    )
    if scaffold_return_code != 0:
        raise PreviewSetupError(f"failed to create {scene.template} playground")
    setup_command: tuple[str, ...]
    for setup_command in scene.setup_commands:
        return_code: int = _run_sqb(
            sqb=sqb,
            project_dir=project_dir,
            command=setup_command,
            environment=environment,
            no_color=True,
            quiet=True,
        )
        if return_code != 0:
            raise PreviewSetupError(f"preview setup command failed: sqb {' '.join(setup_command)}")
    if scene.mutate_payments:
        _mutate_payments_model(project_dir=project_dir)


def _run_sqb(
    *,
    sqb: str,
    project_dir: Path,
    command: tuple[str, ...],
    environment: dict[str, str],
    no_color: bool,
    quiet: bool,
) -> int:
    color_arguments: tuple[str, ...] = ("--no-color",) if no_color else ()
    return _run_process(
        args=(sqb, "--project-dir", str(project_dir), *color_arguments, *command),
        environment=environment,
        quiet=quiet,
    )


def _run_process(*, args: tuple[str, ...], environment: dict[str, str], quiet: bool) -> int:
    output_target: int | None = subprocess.DEVNULL if quiet else None
    process: subprocess.Popen[bytes] = subprocess.Popen(
        args=args,
        env=environment,
        stdout=output_target,
        stderr=output_target,
    )

    def _forward_signal(signum: int, frame: FrameType | None) -> None:
        del frame
        if process.poll() is None:
            try:
                process.send_signal(signum)
            except ProcessLookupError:
                return

    previous_sigint: _SignalHandler = signal.signal(signal.SIGINT, _forward_signal)
    previous_sigterm: _SignalHandler = signal.signal(signal.SIGTERM, _forward_signal)
    try:
        return process.wait()
    finally:
        signal.signal(signal.SIGINT, previous_sigint)
        signal.signal(signal.SIGTERM, previous_sigterm)


def _mutate_payments_model(*, project_dir: Path) -> None:
    model_path: Path = project_dir / _PAYMENTS_MODEL_PATH
    source: str = model_path.read_text(encoding="utf-8")
    changed: str = source.replace(_QUERY_MARKER, _CHANGED_QUERY_MARKER, 1)
    if changed == source:
        raise PreviewSetupError(f"could not mutate playground model: {model_path}")
    model_path.write_text(changed, encoding="utf-8")


def _resolve_sqb_executable() -> str:
    executable: str | None = shutil.which("sqb")
    if executable is None:
        raise PreviewSetupError("sqb executable is not available on PATH")
    return executable


def _write_scene_header(*, scene: PreviewScene, no_color: bool) -> None:
    label: str = f"{scene.name}: {scene.description}"
    use_color: bool = not no_color and supports_color()
    rendered: str = f"\033[7m  {label}  \033[0m" if use_color else label
    print(f"\n{rendered}\n", flush=True)
