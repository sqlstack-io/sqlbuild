from textwrap import dedent

import pytest
from fensu import RuleCase, RuleResult, evaluate_rule

from scripts.fensu_policy._helpers.native_crossings import crossing_lines, python_field_lines
from scripts.fensu_policy._helpers.native_layers import (
    helper_references,
    layer_plan,
    upward_reference_lines,
)
from scripts.fensu_policy.models import LayerPlan
from scripts.fensu_policy.rules.native_boundary import native_module_test_patch
from tests.unit.scripts.fensu_policy.rules._test_types import (
    NativeDirectionTestCase,
    NativeLayeringTestCase,
    NativeLayerTestCase,
    NativeLinesTestCase,
    NativePatchTestCase,
    NativeReferencesTestCase,
)

TEST_PATH: str = "tests/unit/src/sqlbuild/example/test_example.py"


@pytest.mark.parametrize(
    "test_case",
    [
        NativePatchTestCase(
            description="aliased native module patch faults",
            source=dedent(
                """
                import sqlbuild._native as native_module


                def test_example(monkeypatch: object) -> None:
                    monkeypatch.setattr(native_module, "plan", None)
                """
            ),
            expected_fault_count=1,
        ),
        NativePatchTestCase(
            description="from-imported native module patch faults",
            source=dedent(
                """
                from unittest import mock

                from sqlbuild import _native


                def test_example() -> None:
                    with mock.patch.object(_native, "plan"):
                        pass
                """
            ),
            expected_fault_count=1,
        ),
        NativePatchTestCase(
            description="string native patch target faults",
            source=dedent(
                """
                from unittest import mock


                def test_example() -> None:
                    with mock.patch("sqlbuild._native.plan"):
                        pass
                """
            ),
            expected_fault_count=1,
        ),
        NativePatchTestCase(
            description="patching another module passes",
            source=dedent(
                """
                import sqlbuild.executor.runtime as runtime_module


                def test_example(monkeypatch: object) -> None:
                    monkeypatch.setattr(runtime_module, "run", None)
                """
            ),
            expected_fault_count=0,
        ),
        NativePatchTestCase(
            description="budgeted native patch passes",
            source=dedent(
                """
                import sqlbuild._native as native_module


                def test_example(monkeypatch: object) -> None:
                    monkeypatch.setattr(native_module, "plan", None)
                """
            ),
            expected_fault_count=0,
            budgets=(f"{TEST_PATH}=1",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_test_module_when_checking_native_patches_then_matches_contract(
    test_case: NativePatchTestCase,
) -> None:
    result: RuleResult = evaluate_rule(
        rule=native_module_test_patch,
        test_case=RuleCase(
            description=test_case.description,
            source=test_case.source,
            expected_fault_count=test_case.expected_fault_count,
            path=TEST_PATH,
            scope_root=None,
        ),
        rule_options={"pending_budgets": test_case.budgets},
    )

    assert result.fault_count == test_case.expected_fault_count


@pytest.mark.parametrize(
    "test_case",
    [
        NativeLinesTestCase(
            description="attribute reads, calls, imports and module strings count",
            source=dedent(
                """
                fn read(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
                    let name = value.getattr("name")?;
                    let _ = value.call_method1("get", ("x",))?;
                    let _ = imported(py, "pathlib", "Path")?.call1((name,))?;
                    let _ = py.import("sqlbuild.compiler")?;
                    Ok(())
                }
                """
            ),
            expected_lines=(3, 4, 5, 5, 6, 6),
        ),
        NativeLinesTestCase(
            description="pyclass module metadata is not a Python crossing",
            source='#[pyclass(frozen, module = "sqlbuild._native")]\nstruct Snapshot;\n',
            expected_lines=(),
        ),
        NativeLinesTestCase(
            description="multiline namespaced pyclass metadata preserves callback line numbers",
            source=dedent(
                """
                #[pyo3::pyclass(
                    frozen,
                    module = "sqlbuild._native",
                )]
                struct Snapshot;
                fn read(py: Python<'_>) {
                    py.import("sqlbuild.compiler");
                }
                """
            ),
            expected_lines=(8, 8),
        ),
        NativeLinesTestCase(
            description="raw pyclass module metadata does not hide an adjacent real crossing",
            source='#[pyclass(module = r#"sqlbuild._native"#)] struct Snapshot; fn read() { imported(py, "sqlbuild.compiler", "compile"); }\n',
            expected_lines=(1, 1),
        ),
        NativeLinesTestCase(
            description="module assignments outside pyclass remain crossings",
            source='fn read() { let module = "sqlbuild.compiler"; load(module); }\n',
            expected_lines=(1,),
        ),
        NativeLinesTestCase(
            description="comments, imports and test modules are skipped",
            source=dedent(
                """
                use pyo3::types::PyAnyMethods;
                const JSON_MODULE: &str = "dataclasses";
                // value.getattr("name")
                fn read() {}

                #[cfg(test)]
                mod tests {
                    fn probe(value: &Bound<'_, PyAny>) { let _ = value.getattr("x"); }
                }
                """
            ),
            expected_lines=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rust_source_when_finding_crossings_then_reports_each_crossing(
    test_case: NativeLinesTestCase,
) -> None:
    assert tuple(crossing_lines(source=test_case.source)) == test_case.expected_lines


@pytest.mark.parametrize(
    "test_case",
    [
        NativeLinesTestCase(
            description="Python-typed struct fields count and Rust fields do not",
            source=dedent(
                """
                pub(crate) struct Request<'py> {
                    pub(crate) project: Bound<'py, PyAny>,
                    pub(crate) options: Py<PyDict>,
                    pub(crate) name: String,
                    pub(crate) adapter: Option<Py<PyAny>>,
                }
                """
            ),
            expected_lines=(3, 4, 6),
        ),
        NativeLinesTestCase(
            description="Python-typed function parameters do not count",
            source=dedent(
                """
                fn read(value: &Bound<'_, PyAny>) -> PyResult<String> {
                    Ok(String::new())
                }
                """
            ),
            expected_lines=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rust_source_when_finding_python_fields_then_reports_struct_fields(
    test_case: NativeLinesTestCase,
) -> None:
    assert tuple(python_field_lines(source=test_case.source)) == test_case.expected_lines


HELPERS: str = "crates/sqlbuild-python/src/bindings/_helpers"
LAYERS: LayerPlan = layer_plan(
    order=("infra", "front", "plan|compile", "registry"),
    modules=(
        "boundary=infra",
        "discovery=front",
        "planner=plan",
        "compile_driver=compile",
        "compile_driver/python_calls=infra",
        "*/registration=registry",
        "misnamed=nowhere",
    ),
)


@pytest.mark.parametrize(
    "test_case",
    [
        NativeReferencesTestCase(
            description="single paths and nested brace groups expand to each named path",
            source=dedent(
                """
                use crate::bindings::_helpers::boundary::panics::guard;
                use crate::bindings::_helpers::{
                    compile_driver::{python_calls::imported, project},
                    discovery,
                };
                """
            ),
            expected_references=(
                (2, ("boundary", "panics", "guard")),
                (4, ("compile_driver", "python_calls", "imported")),
                (4, ("compile_driver", "project")),
                (5, ("discovery",)),
            ),
        ),
        NativeReferencesTestCase(
            description="self in a group names the group's module",
            source="use crate::bindings::_helpers::planner::{self, models::Entry};\n",
            expected_references=((1, ("planner",)), (1, ("planner", "models", "Entry"))),
        ),
        NativeReferencesTestCase(
            description="inline paths count; comments and test modules do not",
            source=dedent(
                """
                // use crate::bindings::_helpers::discovery::files;
                fn read() { crate::bindings::_helpers::planner::run(); }

                #[cfg(test)]
                mod tests {
                    use crate::bindings::_helpers::compile_driver::project;
                }
                """
            ),
            expected_references=((3, ("planner", "run")),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rust_source_when_finding_helper_references_then_expands_paths(
    test_case: NativeReferencesTestCase,
) -> None:
    found: tuple[tuple[int, tuple[str, ...]], ...] = tuple(
        (reference.line, reference.target)
        for reference in helper_references(source=test_case.source)
    )

    assert found == test_case.expected_references


@pytest.mark.parametrize(
    "test_case",
    [
        NativeLayerTestCase(
            description="a module takes its top-level assignment",
            module=("compile_driver", "project", "run"),
            expected_layer="compile",
        ),
        NativeLayerTestCase(
            description="a longer assignment overrides its parent",
            module=("compile_driver", "python_calls", "imported"),
            expected_layer="infra",
        ),
        NativeLayerTestCase(
            description="a wildcard assignment overrides a shorter one",
            module=("discovery", "registration"),
            expected_layer="registry",
        ),
        NativeLayerTestCase(
            description="a submodule without its own entry inherits",
            module=("discovery", "files"),
            expected_layer="front",
        ),
        NativeLayerTestCase(
            description="an entry naming an unknown layer is ignored",
            module=("misnamed",),
            expected_layer=None,
        ),
        NativeLayerTestCase(
            description="an unassigned module has no layer",
            module=("unknown",),
            expected_layer=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_module_path_when_resolving_layer_then_longest_assignment_wins(
    test_case: NativeLayerTestCase,
) -> None:
    assert LAYERS.layer_of(module=test_case.module) == test_case.expected_layer


@pytest.mark.parametrize(
    "test_case",
    [
        NativeDirectionTestCase(
            description="importing a lower layer passes",
            source="compile",
            target="infra",
            expected_upward=False,
        ),
        NativeDirectionTestCase(
            description="importing the same layer passes",
            source="compile",
            target="compile",
            expected_upward=False,
        ),
        NativeDirectionTestCase(
            description="importing a higher layer is upward",
            source="front",
            target="compile",
            expected_upward=True,
        ),
        NativeDirectionTestCase(
            description="importing a sibling layer is upward",
            source="plan",
            target="compile",
            expected_upward=True,
        ),
        NativeDirectionTestCase(
            description="the top layer may import anything below",
            source="registry",
            target="plan",
            expected_upward=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_two_layers_when_checking_direction_then_lower_and_same_layers_pass(
    test_case: NativeDirectionTestCase,
) -> None:
    upward: bool = LAYERS.points_upward(source=test_case.source, target=test_case.target)

    assert upward is test_case.expected_upward


@pytest.mark.parametrize(
    "test_case",
    [
        NativeLayeringTestCase(
            description="imports into higher and sibling layers are reported",
            path=f"{HELPERS}/planner/entries.rs",
            source=dedent(
                """
                use crate::bindings::_helpers::boundary::panics::guard;
                use crate::bindings::_helpers::compile_driver::{project, python_calls::imported};
                use crate::bindings::_helpers::discovery::registration::register;
                """
            ),
            expected_lines=[3, 4],
        ),
        NativeLayeringTestCase(
            description="references within the file's own module pass",
            path=f"{HELPERS}/compile_driver/python_calls.rs",
            source="use crate::bindings::_helpers::compile_driver::python_calls::models::Call;\n",
            expected_lines=[],
        ),
        NativeLayeringTestCase(
            description="a lower-layer file importing its higher-layer parent module is reported",
            path=f"{HELPERS}/compile_driver/python_calls.rs",
            source="use crate::bindings::_helpers::compile_driver::project::Project;\n",
            expected_lines=[1],
        ),
        NativeLayeringTestCase(
            description="mod.rs files take their directory's module",
            path=f"{HELPERS}/discovery/mod.rs",
            source="pub(crate) mod files;\nuse crate::bindings::_helpers::boundary::panics;\n",
            expected_lines=[],
        ),
        NativeLayeringTestCase(
            description="an unassigned helper module is reported",
            path=f"{HELPERS}/unknown/reader.rs",
            source="fn read() {}\n",
            expected_lines=None,
        ),
        NativeLayeringTestCase(
            description="test files and files outside the helpers are skipped",
            path=f"{HELPERS}/discovery/tests/test_files.rs",
            source="use crate::bindings::_helpers::compile_driver::project;\n",
            expected_lines=[],
        ),
        NativeLayeringTestCase(
            description="tests.rs module roots are skipped",
            path=f"{HELPERS}/discovery/tests.rs",
            source="use crate::bindings::_helpers::compile_driver::project;\n",
            expected_lines=[],
        ),
        NativeLayeringTestCase(
            description="a test_ file name outside a tests directory is source and is checked",
            path=f"{HELPERS}/discovery/test_command.rs",
            source="use crate::bindings::_helpers::compile_driver::project;\n",
            expected_lines=[1],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_helper_file_when_checking_layers_then_reports_upward_imports(
    test_case: NativeLayeringTestCase,
) -> None:
    found: list[int] | None = upward_reference_lines(
        path=test_case.path, source=test_case.source, plan=LAYERS
    )

    assert found == test_case.expected_lines
