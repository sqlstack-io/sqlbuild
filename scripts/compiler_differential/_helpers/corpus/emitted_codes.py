"""Run the failure corpus and report the codes each case really emits."""

from __future__ import annotations

import ast
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import sqlbuild._native as _native
from scripts.compiler_differential._helpers.comparing.comparison import (
    diagnostic_codes,
    first_diagnostic,
    first_diagnostic_message,
)
from scripts.compiler_differential._helpers.corpus.corpus import build_corpus
from scripts.compiler_differential._helpers.goldens.goldens import golden_path_masks, masked_text
from scripts.compiler_differential._helpers.running.execution import run_project
from scripts.compiler_differential.constants import (
    ANALYSIS_CODE_SCAN_ROOTS,
    ANALYSIS_NATIVE_CODE_SCAN_ROOTS,
    BUILT_IN_AUDIT_SHADOW_CODE,
    COLD_COMPILE,
    CORPUS_FAILURES,
    DIAGNOSTIC_CODE_PATTERN,
    ERROR_SEVERITY,
    EXPECT_SUCCESS,
    NATIVE_CODE_CONSTANT_PATTERN,
    NATIVE_CODE_TEST_DIRECTORY,
    RENDER_CODE_SCAN_EXCLUDED,
    RENDER_NATIVE_CODE_SCAN_ROOTS,
    RENDER_RAISED_CODES,
    WARNING_SEVERITY,
)
from scripts.compiler_differential.models import (
    CorpusProject,
    DifferentialOptions,
    EmittedCodes,
    ExpectedOutcome,
    FailureCase,
    ProjectRun,
)
from sqlbuild._native import DiscoveryError, ResourceIdentityError, ScopeDiagnosticCode
from sqlbuild.compiler.compile.exceptions import CompileInputError

_SOURCE_DIRECTORY: str = "source"
_CODE_KEYWORD: str = "code"


def emitted_failure_codes(*, options: DifferentialOptions) -> dict[str, EmittedCodes]:
    """Compile every failure case and return its emitted codes by name."""

    corpus: list[CorpusProject] = build_corpus(
        repo_root=options.work_dir,
        corpora=(CORPUS_FAILURES,),
        seeds=range(0),
        dense_models=0,
        projects=(),
        project_expectation=EXPECT_SUCCESS,
    )
    with ThreadPoolExecutor(max_workers=options.jobs) as pool:
        emitted: list[EmittedCodes] = list(
            pool.map(lambda project: _emitted_codes(project=project, options=options), corpus)
        )
    return {project.name: codes for project, codes in zip(corpus, emitted, strict=True)}


def case_emitted_codes(*, case: FailureCase, options: DifferentialOptions) -> EmittedCodes:
    """Compile one failure case and return what it really emitted."""

    return _emitted_codes(
        project=CorpusProject(
            name=f"{CORPUS_FAILURES}/{case.name}",
            commands=(COLD_COMPILE,),
            expected=ExpectedOutcome(error_code=case.expected_code),
            writer=case.write,
        ),
        options=options,
    )


def discovery_error_codes() -> frozenset[str]:
    """Return the code of every `DiscoveryError` subclass, walking the whole hierarchy."""

    _ = ResourceIdentityError
    codes: set[str] = set()
    pending: list[type[DiscoveryError]] = list(DiscoveryError.__subclasses__())
    while pending:
        error_class: type[DiscoveryError] = pending.pop()
        codes.add(str(error_class.code))
        pending.extend(error_class.__subclasses__())
    return frozenset(codes)


def render_error_codes() -> frozenset[str]:
    """Return every code render, scope, and attachment can report, from compiler constants."""

    return frozenset(
        {
            CompileInputError.code,
            BUILT_IN_AUDIT_SHADOW_CODE,
            *(code.value for code in ScopeDiagnosticCode),
            *RENDER_RAISED_CODES,
        }
    )


def analysis_error_codes(compiler_root: Path) -> frozenset[str]:
    """Return the help-catalogue codes plus every code in the analysis-stage modules and crates."""

    found: set[str] = {code for code, _ in _native.semantic_help_catalogue()}
    for root in ANALYSIS_CODE_SCAN_ROOTS:
        path: Path = compiler_root / root
        for module in sorted(path.rglob("*.py")) if path.is_dir() else (path,):
            found.update(_module_codes(module))
    repo_root: Path = Path(__file__).resolve().parents[4]
    for native_module in ANALYSIS_NATIVE_CODE_SCAN_ROOTS:
        text: str = (repo_root / native_module).read_text(encoding="utf-8")
        found.update(NATIVE_CODE_CONSTANT_PATTERN.findall(text))
    return frozenset(found)


def inline_compile_codes(compile_root: Path) -> dict[str, tuple[str, ...]]:
    """Map each `code="X###"` literal in render-stage compile modules to where it is written."""

    found: dict[str, list[str]] = {}
    for path in sorted(compile_root.rglob("*.py")):
        relative: str = path.relative_to(compile_root).as_posix()
        if any(relative.startswith(f"{excluded}/") for excluded in RENDER_CODE_SCAN_EXCLUDED):
            continue
        for node in ast.walk(ast.parse(path.read_text(encoding="utf-8"))):
            code: str | None = _code_literal(node)
            if code is not None:
                found.setdefault(code, []).append(f"{relative}:{getattr(node, 'lineno', 0)}")
    repo_root: Path = Path(__file__).resolve().parents[4]
    for directory in RENDER_NATIVE_CODE_SCAN_ROOTS:
        for path in sorted((repo_root / directory).rglob("*.rs")):
            if NATIVE_CODE_TEST_DIRECTORY in path.parts:
                continue
            for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
                for code in NATIVE_CODE_CONSTANT_PATTERN.findall(line):
                    found.setdefault(code, []).append(
                        f"{path.relative_to(repo_root).as_posix()}:{line_number}"
                    )
    return {code: tuple(locations) for code, locations in found.items()}


def unaccounted_inline_codes(
    *, compile_root: Path, accounted: frozenset[str]
) -> dict[str, tuple[str, ...]]:
    """Return the inline render-stage codes that are neither required nor documented."""

    return {
        code: locations
        for code, locations in inline_compile_codes(compile_root).items()
        if code not in accounted
    }


def _module_codes(path: Path) -> set[str]:
    codes: set[str] = set()
    for node in ast.walk(ast.parse(path.read_text(encoding="utf-8"))):
        literal: str | None = _code_literal(node) or _code_constant(node)
        if literal is not None:
            codes.add(literal)
    return codes


def _code_constant(node: ast.AST) -> str | None:
    if not isinstance(node, (ast.Assign, ast.AnnAssign)):
        return None
    targets: list[ast.expr] = node.targets if isinstance(node, ast.Assign) else [node.target]
    value: ast.expr | None = node.value
    if not (
        any(isinstance(target, ast.Name) and target.id.endswith("_CODE") for target in targets)
        and isinstance(value, ast.Constant)
        and isinstance(value.value, str)
        and DIAGNOSTIC_CODE_PATTERN.fullmatch(value.value)
    ):
        return None
    return value.value


def _code_literal(node: ast.AST) -> str | None:
    if not (
        isinstance(node, ast.keyword)
        and node.arg == _CODE_KEYWORD
        and isinstance(node.value, ast.Constant)
        and isinstance(node.value.value, str)
        and DIAGNOSTIC_CODE_PATTERN.fullmatch(node.value.value)
    ):
        return None
    return node.value.value


def _emitted_codes(*, project: CorpusProject, options: DifferentialOptions) -> EmittedCodes:
    case_dir: Path = options.work_dir / project.name.replace("/", "__")
    source_dir: Path = case_dir / _SOURCE_DIRECTORY
    if project.writer is not None:
        project.writer(source_dir)
    run: ProjectRun = run_project(
        project=project, source_dir=source_dir, case_dir=case_dir, options=options
    )
    first: dict[str, object] = first_diagnostic(outcome=run.outcomes[0], severity=ERROR_SEVERITY)
    message: str | None = first_diagnostic_message(outcome=run.outcomes[0], severity=ERROR_SEVERITY)
    notes: object = first.get("notes")
    line: object = first.get("line")
    column: object = first.get("column")
    return EmittedCodes(
        errors=diagnostic_codes(outcome=run.outcomes[0], severity=ERROR_SEVERITY),
        warnings=diagnostic_codes(outcome=run.outcomes[0], severity=WARNING_SEVERITY),
        first_error_message=(
            None
            if message is None
            else masked_text(
                text=message,
                masks=golden_path_masks(case_dir=case_dir, work_dir=options.work_dir),
            )
        ),
        first_error_help=None if first.get("help") is None else str(first.get("help")),
        first_error_notes=tuple(str(note) for note in notes) if isinstance(notes, list) else (),
        first_error_location=(
            (line, column) if isinstance(line, int) and isinstance(column, int) else None
        ),
        codes=diagnostic_codes(outcome=run.outcomes[0], severity=None),
    )
