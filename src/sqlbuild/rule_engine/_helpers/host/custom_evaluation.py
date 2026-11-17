"""Isolated Python host for selected repository-defined rules."""

from __future__ import annotations

from collections import Counter
from collections.abc import Callable
from contextlib import AbstractContextManager, nullcontext
from dataclasses import dataclass, field
from itertools import chain
from pathlib import Path
from time import thread_time_ns

from sqlbuild._native import (
    CUSTOM_RULE_PROJECT_SUBJECT,
    CustomHostPartition,
    CustomRuleEvaluation,
    CustomRuleRun,
    Finding,
    HostCancelledError,
    Model,
    NonHermeticRuleError,
    Project,
    Rule,
    RulesConfig,
    RulesError,
    RuleSubject,
)
from sqlbuild.rule_engine._helpers.host.module_state import rule_namespaces
from sqlbuild.rule_engine._helpers.host.tracked_facts import tracked_fact_views
from sqlbuild.rule_engine.classes.fact_reads import FactReads
from sqlbuild.rule_engine.classes.module_state_monitor import ModuleStateMonitor
from sqlbuild.rule_engine.classes.project_tree import public_model
from sqlbuild.rule_engine.classes.rule_context import (
    EvaluationRuleContext,
    RuleFactViews,
    build_rule_fact_views,
)
from sqlbuild.rule_engine.classes.runtime_guard import RuntimeGuard
from sqlbuild.rule_engine.constants import CUSTOM_HOST_CANCELLED_MESSAGE
from sqlbuild.rule_engine.types import CustomRulePlan, RuleFactModel, RuleFactProject

type _FindingKey = tuple[str, str, int, int, str, str]


@dataclass
class _RuleTimings:
    """Invocations and CPU nanoseconds per rule code, excluding the runtime guard's checks."""

    by_code: dict[str, list[int]] = field(default_factory=dict)

    def record(self, *, code: str, cpu_ns: int) -> None:
        totals: list[int] = self.by_code.setdefault(code, [0, 0])
        totals[0] += 1
        totals[1] += max(cpu_ns, 0)


@dataclass(frozen=True)
class _PassInputs:
    project: RuleFactProject
    config: RulesConfig
    project_dir: Path
    selected_rules: tuple[Rule, ...]
    dialect: str
    facts: RuleFactViews
    plan: CustomRulePlan | None
    guard: RuntimeGuard | None
    cancelled: Callable[[], bool] | None
    timings: _RuleTimings


def evaluate_custom_rules(
    *,
    project: RuleFactProject,
    config: RulesConfig,
    project_dir: Path,
    selected_rules: tuple[Rule, ...],
    dialect: str = "generic",
    plan: CustomRulePlan | None = None,
    track_reads: bool = False,
    verify_determinism: bool = False,
    guard: RuntimeGuard | None = None,
    partition: CustomHostPartition | None = None,
) -> CustomRuleRun:
    """Run selected custom rules for every planned subject through the Python authoring API."""

    custom_rules: tuple[Rule, ...] = tuple(
        rule for rule in selected_rules if rule.custom and (plan is None or rule.code in plan)
    )
    if not custom_rules:
        return CustomRuleRun(evaluations=(), untracked_codes=frozenset())
    models: tuple[RuleFactModel, ...] = tuple(
        sorted(project.models, key=lambda item: item.relative_path.as_posix())
    )
    facts: RuleFactViews = build_rule_fact_views(
        project=project, project_dir=project_dir, dialect=dialect
    )
    reads: FactReads | None = FactReads() if track_reads else None
    tracker: _ReadTracker = _ReadTracker(
        reads=reads,
        rules=custom_rules,
        rules_root=project_dir / "rules",
        detect_state=partition is not None and partition.detect_state,
    )

    inputs: _PassInputs = _PassInputs(
        project=project,
        config=config,
        project_dir=project_dir,
        selected_rules=selected_rules,
        dialect=dialect,
        facts=facts,
        plan=plan,
        guard=guard,
        cancelled=None if partition is None else partition.cancelled,
        timings=_RuleTimings(),
    )

    def run_pass(
        *, rules: tuple[Rule, ...], subjects: tuple[RuleFactModel, ...], active: _ReadTracker
    ) -> list[CustomRuleEvaluation]:
        return _evaluate_pass(inputs=inputs, rules=rules, models=subjects, tracker=active)

    evaluations: list[CustomRuleEvaluation] = run_pass(
        rules=custom_rules, subjects=models, active=tracker
    )
    monitor_ns: int = tracker.monitor_ns
    if verify_determinism:
        repeat_tracker: _ReadTracker = _ReadTracker(
            reads=None, rules=(), rules_root=project_dir / "rules"
        )
        repeated: list[CustomRuleEvaluation] = run_pass(
            rules=custom_rules[::-1], subjects=models[::-1], active=repeat_tracker
        )
        monitor_ns += repeat_tracker.monitor_ns
        _verify_repeatable(
            rules=custom_rules,
            first=list(chain.from_iterable(item.findings for item in evaluations)),
            repeated=list(chain.from_iterable(item.findings for item in repeated)),
        )
    return CustomRuleRun(
        evaluations=tuple(evaluations),
        untracked_codes=frozenset(tracker.untracked_codes),
        uncacheable_codes=frozenset(tracker.uncacheable_codes),
        observed=() if reads is None else tuple(reads.observed.items()),
        stateful_codes=frozenset(tracker.stateful_codes),
        rule_timings={
            code: (invocations, cpu_ns)
            for code, (invocations, cpu_ns) in inputs.timings.by_code.items()
        },
        monitor_ns=monitor_ns,
    )


class _ReadTracker:
    """Attribute reads to one invocation; rules whose module state changes become untracked."""

    def __init__(
        self,
        *,
        reads: FactReads | None,
        rules: tuple[Rule, ...],
        rules_root: Path,
        detect_state: bool = False,
    ) -> None:
        self.reads: FactReads | None = reads
        self.untracked_codes: set[str] = set()
        self.uncacheable_codes: set[str] = set()
        self.stateful_codes: set[str] = set()
        self.monitor_ns: int = 0
        started: int = thread_time_ns()
        self._state: ModuleStateMonitor | None = (
            ModuleStateMonitor(
                namespaces={rule.code: rule_namespaces((rule.check,)) for rule in rules},
                rules_root=rules_root,
            )
            if reads is not None or detect_state
            else None
        )
        self.monitor_ns += thread_time_ns() - started

    def start(self) -> None:
        if self.reads is not None:
            self.reads.current = set()
            self.reads.untracked = False
            self.reads.uncacheable = False

    def finish(self, *, code: str) -> frozenset[tuple[object, ...]] | None:
        reads: FactReads | None = self.reads
        observed: set[tuple[object, ...]] | None = None
        if reads is not None:
            observed = reads.current
            reads.current = None
            if reads.untracked:
                self.untracked_codes.add(code)
            if reads.uncacheable:
                self.uncacheable_codes.add(code)
        return None if observed is None else frozenset(observed)

    def finish_subject(self) -> None:
        """Mark codes whose module state changed since the last check; project rules check each."""

        if self._state is None:
            return
        started: int = thread_time_ns()
        changed: tuple[str, ...] = self._state.changed_codes()
        self.monitor_ns += thread_time_ns() - started
        self.stateful_codes.update(changed)
        if self.reads is not None:
            self.untracked_codes.update(changed)


def _evaluate_pass(
    *,
    inputs: _PassInputs,
    rules: tuple[Rule, ...],
    models: tuple[RuleFactModel, ...],
    tracker: _ReadTracker,
) -> list[CustomRuleEvaluation]:
    evaluations: list[CustomRuleEvaluation] = []
    project: RuleFactProject = inputs.project
    plan: CustomRulePlan | None = inputs.plan
    guard: RuntimeGuard | None = inputs.guard
    context_facts: RuleFactViews = (
        inputs.facts
        if tracker.reads is None
        else tracked_fact_views(views=inputs.facts, reads=tracker.reads)
    )

    def context(rule: Rule) -> EvaluationRuleContext:
        return EvaluationRuleContext(
            model=None,
            rule=rule,
            config=inputs.config,
            project=project,
            project_dir=inputs.project_dir,
            selected_rules=inputs.selected_rules,
            dialect=inputs.dialect,
            facts=context_facts,
            reads=tracker.reads,
        )

    project_rules: tuple[Rule, ...] = tuple(
        rule for rule in rules if rule.subject is RuleSubject.PROJECT
    )
    model_rules: tuple[Rule, ...] = tuple(
        rule for rule in rules if rule.subject is RuleSubject.MODEL
    )
    model_contexts: tuple[tuple[Rule, EvaluationRuleContext, frozenset[str] | None], ...] = tuple(
        (rule, context(rule), None if plan is None else plan.get(rule.code)) for rule in model_rules
    )
    for rule in project_rules:
        planned_project: frozenset[str] | None = None if plan is None else plan.get(rule.code)
        if planned_project is not None and CUSTOM_RULE_PROJECT_SUBJECT not in planned_project:
            continue
        ctx: EvaluationRuleContext = context(rule)
        _stop_if_cancelled(inputs)
        tracker.start()
        try:
            findings: list[Finding] = _invoke(
                rule=rule,
                subject=Project(
                    model_count=len(project.models),
                    target_name=project.effective_target_name,
                ),
                ctx=ctx,
                guard=guard,
                timings=inputs.timings,
            )
        except NonHermeticRuleError:
            raise
        except Exception as error:
            raise RulesError(f"rule {rule.code} failed for the project: {error}") from error
        evaluations.append(
            CustomRuleEvaluation(
                code=rule.code,
                subject=None,
                findings=tuple(findings),
                reads=tracker.finish(code=rule.code),
            )
        )
        tracker.finish_subject()
    for model in models:
        subject: Model = public_model(model)
        path: str = model.relative_path.as_posix()
        evaluated: bool = False
        for rule, ctx, planned in model_contexts:
            if planned is not None and path not in planned:
                continue
            evaluated = True
            _stop_if_cancelled(inputs)
            tracker.start()
            try:
                findings = _invoke(
                    rule=rule, subject=subject, ctx=ctx, guard=guard, timings=inputs.timings
                )
            except NonHermeticRuleError:
                raise
            except Exception as error:
                raise RulesError(
                    f"rule {rule.code} failed for {model.relative_path}: {error}"
                ) from error
            evaluations.append(
                CustomRuleEvaluation(
                    code=rule.code,
                    subject=path,
                    findings=tuple(findings),
                    reads=tracker.finish(code=rule.code),
                )
            )
        if evaluated:
            tracker.finish_subject()
    return evaluations


def _stop_if_cancelled(inputs: _PassInputs) -> None:
    """Stop between invocations, outside any Rule's guarded scope, once the run is cancelled."""

    if inputs.cancelled is not None and inputs.cancelled():
        raise HostCancelledError(CUSTOM_HOST_CANCELLED_MESSAGE)


def _invoke(
    *,
    rule: Rule,
    subject: object,
    ctx: EvaluationRuleContext,
    guard: RuntimeGuard | None,
    timings: _RuleTimings,
) -> list[Finding]:
    if rule.subject_parameter is None or rule.context_parameter is None:
        raise RulesError(f"custom rule {rule.code} has no resolved typed signature")
    scope: AbstractContextManager[None] = nullcontext() if guard is None else guard.rule(rule.code)
    guard_started: int = 0 if guard is None else guard.check_ns
    started: int = thread_time_ns()
    try:
        with scope:
            result: object = rule.check(
                **{
                    rule.subject_parameter: subject,
                    rule.context_parameter: ctx,
                }
            )
    finally:
        guard_ns: int = 0 if guard is None else guard.check_ns - guard_started
        timings.record(code=rule.code, cpu_ns=thread_time_ns() - started - guard_ns)
    if not isinstance(result, list) or any(not isinstance(finding, Finding) for finding in result):
        raise RulesError(f"custom rule {rule.code} must return list[Finding]")
    return result


def _verify_repeatable(
    *, rules: tuple[Rule, ...], first: list[Finding], repeated: list[Finding]
) -> None:
    for rule in rules:
        before: Counter[_FindingKey] = _finding_keys(findings=first, code=rule.code)
        after: Counter[_FindingKey] = _finding_keys(findings=repeated, code=rule.code)
        if before != after:
            raise RulesError(
                f"custom rule {rule.code} is not deterministic: evaluating every subject again "
                f"after the others changed its findings from {before.total()} to "
                f"{after.total()}; keep per-subject results independent of module-level state "
                "and evaluation order"
            )


def _finding_keys(*, findings: list[Finding], code: str) -> Counter[_FindingKey]:
    return Counter(
        (
            finding.code,
            finding.path.as_posix(),
            finding.line,
            finding.column,
            finding.message,
            finding.remediation,
        )
        for finding in findings
        if finding.code == code
    )
