"""Generated rules skill freshness behavior tests."""

from pathlib import Path
from stat import S_IMODE

import pytest

from sqlbuild._native import RuleExemption, RulesConfig, RulesError
from sqlbuild._native import install_rules_skills as install_skills
from tests.unit.src.sqlbuild.rule_engine.main.skills._test_types import (
    SkillDivergenceTestCase,
    SkillFreshnessTestCase,
    SkillGuidanceTestCase,
    SkillInstallationTestCase,
)
from tests.unit.src.sqlbuild.rule_engine.main.skills.helpers import write_custom_rule

RULES_SKILL_PATHS: tuple[str, ...] = (
    ".agents/skills/sqlbuild-rules/SKILL.md",
    ".claude/skills/sqlbuild-rules/SKILL.md",
    ".opencode/skills/sqlbuild-rules/SKILL.md",
)


@pytest.mark.parametrize(
    "test_case",
    [
        SkillFreshnessTestCase(
            description="changed active rules make installed guidance stale",
            selected_rule="SQBRMODEL101",
            changed_rule="SQBRMODEL102",
            expected_fresh=True,
            expected_stale=False,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_active_rules_when_installing_skills_then_check_tracks_policy_freshness(
    tmp_path: Path,
    test_case: SkillFreshnessTestCase,
) -> None:
    config: RulesConfig = RulesConfig(select=(test_case.selected_rule,))

    installed: bool = install_skills(config=config, project_dir=tmp_path, check=False)
    fresh: bool = install_skills(config=config, project_dir=tmp_path, check=True)
    stale: bool = install_skills(
        config=RulesConfig(select=(test_case.changed_rule,)),
        project_dir=tmp_path,
        check=True,
    )

    assert installed is True
    assert fresh is test_case.expected_fresh
    assert stale is test_case.expected_stale
    skill: str = (tmp_path / ".agents/skills/sqlbuild-rules/SKILL.md").read_text(encoding="utf-8")
    assert test_case.selected_rule in skill
    assert test_case.changed_rule not in skill


@pytest.mark.parametrize(
    "test_case",
    [
        SkillDivergenceTestCase(
            description="locally edited owned skill",
            selected_rule="SQBRMODEL101",
            local_edit="local edit\n",
            expected_error="refusing to overwrite divergent rules skill",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_locally_edited_owned_skill_when_installing_then_refuses_to_overwrite(
    tmp_path: Path,
    test_case: SkillDivergenceTestCase,
) -> None:
    config: RulesConfig = RulesConfig(select=(test_case.selected_rule,))
    _ = install_skills(config=config, project_dir=tmp_path, check=False)
    path: Path = tmp_path / ".agents/skills/sqlbuild-rules/SKILL.md"
    path.write_text(
        path.read_text(encoding="utf-8") + test_case.local_edit,
        encoding="utf-8",
    )

    with pytest.raises(RulesError, match=test_case.expected_error):
        install_skills(config=config, project_dir=tmp_path, check=False)


@pytest.mark.parametrize(
    "test_case",
    [
        SkillInstallationTestCase(
            description="later divergent target leaves every target unchanged",
            selected_rule="SQBRMODEL101",
            changed_rule="SQBRMODEL102",
            expected_error="refusing to overwrite divergent rules skill",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_later_target_failure_when_installing_then_all_targets_remain_unchanged(
    tmp_path: Path,
    test_case: SkillInstallationTestCase,
) -> None:
    original_config: RulesConfig = RulesConfig(select=(test_case.selected_rule,))
    _ = install_skills(config=original_config, project_dir=tmp_path, check=False)
    paths: tuple[Path, ...] = tuple(tmp_path / value for value in RULES_SKILL_PATHS)
    divergent: Path = paths[-1]
    divergent.write_text(divergent.read_text(encoding="utf-8") + "local edit\n", encoding="utf-8")
    expected_contents: tuple[bytes, ...] = tuple(path.read_bytes() for path in paths)
    unrelated: Path = tmp_path / ".claude" / "notes.txt"
    unrelated.write_text("keep me\n", encoding="utf-8")

    with pytest.raises(RulesError, match=test_case.expected_error):
        install_skills(
            config=RulesConfig(select=(test_case.changed_rule,)),
            project_dir=tmp_path,
            check=False,
        )

    assert tuple(path.read_bytes() for path in paths) == expected_contents
    assert unrelated.read_bytes() == b"keep me\n"


@pytest.mark.parametrize(
    "test_case",
    [
        SkillInstallationTestCase(
            description="missing targets are only inspected",
            selected_rule="SQBRMODEL101",
            changed_rule="SQBRMODEL102",
            expected_error="",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_targets_when_checking_then_filesystem_is_not_mutated(
    tmp_path: Path,
    test_case: SkillInstallationTestCase,
) -> None:
    unrelated: Path = tmp_path / "notes.txt"
    unrelated.write_text("keep me\n", encoding="utf-8")

    fresh: bool = install_skills(
        config=RulesConfig(select=(test_case.selected_rule,)),
        project_dir=tmp_path,
        check=True,
    )

    assert fresh is test_case.expected_fresh
    assert tuple(path.relative_to(tmp_path) for path in tmp_path.rglob("*")) == (Path("notes.txt"),)
    assert unrelated.read_bytes() == b"keep me\n"


@pytest.mark.parametrize(
    "test_case",
    [
        SkillGuidanceTestCase(
            description="exact exception retains its selected rule guidance",
            expected_fragment="### SQBRMODEL101:",
            absent_fragment="### SQBRMODEL102:",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_exact_exception_when_installing_then_selected_guidance_is_retained(
    tmp_path: Path,
    test_case: SkillGuidanceTestCase,
) -> None:
    config: RulesConfig = RulesConfig(
        select=("SQBRMODEL101",),
        rule_exceptions=(
            RuleExemption(
                rule="SQBRMODEL101",
                path="sql/models/mart/commerce__mart__orders.sql",
                reason="Migration is tracked",
            ),
        ),
    )

    _ = install_skills(config=config, project_dir=tmp_path, check=False)

    skill: str = (tmp_path / RULES_SKILL_PATHS[0]).read_text(encoding="utf-8")
    assert test_case.expected_fragment in skill
    assert test_case.absent_fragment not in skill
    assert "Migration is tracked" in skill


@pytest.mark.parametrize(
    "test_case",
    [
        SkillGuidanceTestCase(
            description="global ignore removes selected family guidance",
            expected_fragment="### SQBRGRAPH101:",
            absent_fragment="### SQBRMODEL101:",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_global_ignore_when_installing_then_ignored_guidance_is_removed(
    tmp_path: Path,
    test_case: SkillGuidanceTestCase,
) -> None:
    config: RulesConfig = RulesConfig(
        select=("SQBRMODEL101", "SQBRGRAPH101"), ignore=("SQBRMODEL",)
    )

    _ = install_skills(config=config, project_dir=tmp_path, check=False)

    skill: str = (tmp_path / RULES_SKILL_PATHS[0]).read_text(encoding="utf-8")
    assert test_case.expected_fragment in skill
    assert test_case.absent_fragment not in skill


@pytest.mark.parametrize(
    "test_case",
    [
        SkillGuidanceTestCase(
            description="selected custom rule renders configured effective option",
            expected_fragment="Effective options: {'required_domain': 'configured_domain'}",
            absent_fragment="'required_domain': 'default_domain'",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_selected_custom_rule_when_installing_then_effective_options_are_rendered(
    tmp_path: Path,
    test_case: SkillGuidanceTestCase,
) -> None:
    _ = write_custom_rule(root=tmp_path)
    config: RulesConfig = RulesConfig(
        select=("XSQBRT105",),
        rule_options={"XSQBRT105": {"required_domain": "configured_domain"}},
    )

    _ = install_skills(config=config, project_dir=tmp_path, check=False)

    skill: str = (tmp_path / RULES_SKILL_PATHS[0]).read_text(encoding="utf-8")
    assert "### XSQBRT105: skill-test-rule" in skill
    assert "custom guidance message" in skill
    assert test_case.expected_fragment in skill
    assert test_case.absent_fragment not in skill


@pytest.mark.parametrize(
    "test_case",
    [
        SkillGuidanceTestCase(
            description="repeated all-target installation is byte deterministic",
            expected_fragment="### SQBRMODEL101:",
            absent_fragment="### SQBRMODEL102:",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_same_policy_when_reinstalling_then_all_targets_are_byte_identical(
    tmp_path: Path,
    test_case: SkillGuidanceTestCase,
) -> None:
    config: RulesConfig = RulesConfig(select=("SQBRMODEL101",))
    _ = install_skills(config=config, project_dir=tmp_path, check=False)
    paths: tuple[Path, ...] = tuple(tmp_path / value for value in RULES_SKILL_PATHS)
    first: tuple[bytes, ...] = tuple(path.read_bytes() for path in paths)
    assert tuple(S_IMODE(path.stat().st_mode) for path in paths) == (0o644, 0o644, 0o644)
    for path in paths:
        path.chmod(0o640)

    _ = install_skills(config=config, project_dir=tmp_path, check=False)

    second: tuple[bytes, ...] = tuple(path.read_bytes() for path in paths)
    assert first == second
    assert len(set(second)) == 1
    assert test_case.expected_fragment.encode() in second[0]
    assert test_case.absent_fragment.encode() not in second[0]
    assert tuple(S_IMODE(path.stat().st_mode) for path in paths) == (0o640, 0o640, 0o640)
