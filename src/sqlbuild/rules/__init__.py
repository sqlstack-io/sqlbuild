"""Public compiler-integrated rule authoring API."""

from collections.abc import Callable

from sqlbuild._native import (
    Finding,
    Model,
    Project,
    ProjectPath,
    RuleCase,
    RuleFile,
    RuleOption,
    RuleResult,
    SqlDocument,
    SqlNode,
    SqlTreeNode,
)
from sqlbuild.rule_engine.main.define import rule as _rule
from sqlbuild.rule_engine.main.evaluate_rule import evaluate_rule
from sqlbuild.rule_engine.types import RuleCheck, RuleContext

__all__ = (
    "Finding",
    "Model",
    "Project",
    "ProjectPath",
    "RuleCase",
    "RuleContext",
    "RuleFile",
    "RuleOption",
    "RuleResult",
    "SqlDocument",
    "SqlNode",
    "SqlTreeNode",
    "evaluate_rule",
    "rule",
)


def rule(
    *,
    code: str,
    message: str,
    remediation: str,
    slug: str | None = None,
    options: tuple[RuleOption[object], ...] = (),
    enabled_by_default: bool = False,
) -> Callable[[RuleCheck], RuleCheck]:
    """Declare one repository-owned compiler rule."""

    return _rule(
        code=code,
        message=message,
        remediation=remediation,
        slug=slug,
        options=options,
        enabled_by_default=enabled_by_default,
    )
