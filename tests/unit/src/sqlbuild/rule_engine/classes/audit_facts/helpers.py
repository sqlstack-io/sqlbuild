from pathlib import Path

from sqlbuild._native import (
    CompiledAudit,
    CompiledObjectKey,
    CompiledResourceType,
    DiscoveredAuditBlock,
    DiscoveredAuditFile,
)
from sqlbuild.compiler.compile.types import CompiledAuditView


def build_audit(*, name: str, attached_target_name: str | None) -> CompiledAuditView:
    relative_path: Path = Path(f"sql/audits/{name}.sql")
    audit_file: DiscoveredAuditFile = DiscoveredAuditFile(
        file_path=relative_path,
        relative_path=relative_path,
        contents="",
        blocks=(),
    )
    return CompiledAudit(
        key=CompiledObjectKey(CompiledResourceType.AUDIT, name),
        scope_deps=(),
        name=name,
        definition_name=name,
        audit_file=audit_file,
        audit_block=DiscoveredAuditBlock(audit_index=0, header_values={}, sql_body="SELECT 1"),
        sql_body="SELECT 1",
        attached_target_name=attached_target_name,
    )
