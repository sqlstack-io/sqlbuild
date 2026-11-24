"""Where one test or scenario runs, and why it cannot run locally."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(frozen=True)
class LocalRefusal:
    """One SQLQuack refusal, or one of SQLBuild's own reasons, placed in the file that holds it."""

    category: str
    construct: str
    reason: str
    code: str | None = None
    message: str | None = None
    label: str | None = None
    notes: tuple[str, ...] = field(default_factory=tuple)
    help: str | None = None
    path: str | None = None
    line: int | None = None
    column: int | None = None
    width: int = 1
    snippet: str | None = None
    snippet_column: int | None = None
    expanded_from: str | None = None

    @property
    def title(self) -> str:
        """`<category>: <construct>`, as SQLQuack's census labels a refusal."""

        return f"{self.category}: {self.construct}"


@dataclass(frozen=True)
class RunOnDecision:
    """Where one test or scenario runs: `local` or `warehouse`, and why not locally."""

    name: str
    run_on: str
    refusals: tuple[LocalRefusal, ...] = field(default_factory=tuple)

    @property
    def reason(self) -> str | None:
        """The first refusal's label; `None` for a test that runs locally."""

        return self.refusals[0].title if self.refusals else None
