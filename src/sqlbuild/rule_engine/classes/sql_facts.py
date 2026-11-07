"""SQL facts indexed by custom-rule model subjects."""

from functools import cached_property

from sqlbuild._native import Model, ModelSql, RuleUsageError, SqlDocument
from sqlbuild.rule_engine.types import RuleFactModel, RuleFactProject


class SqlFacts:
    """Authored and expanded SQL indexed by public model identity."""

    def __init__(self, *, dialect: str, project: RuleFactProject) -> None:
        self._dialect: str = dialect
        self._project: RuleFactProject = project
        self._documents: dict[object, ModelSql] = {}

    @cached_property
    def _models(self) -> dict[object, RuleFactModel]:
        return {model.relative_path: model for model in self._project.models}

    def for_model(self, model: Model) -> ModelSql:
        cached: ModelSql | None = self._documents.get(model.path)
        if cached is not None:
            return cached
        compiled: RuleFactModel = self._resolve(model)
        documents: ModelSql = ModelSql(
            authored=SqlDocument(source=compiled.authored_sql, dialect=self._dialect),
            expanded=SqlDocument(source=compiled.query_sql, dialect=self._dialect),
        )
        self._documents[model.path] = documents
        return documents

    def _resolve(self, model: Model) -> RuleFactModel:
        try:
            return self._models[model.path]
        except KeyError as error:
            raise RuleUsageError(f"model is not part of this project: {model.path}") from error
