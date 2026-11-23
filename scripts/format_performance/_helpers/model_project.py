"""Synthetic unformatted model projects for formatter performance guards."""

from pathlib import Path

_SHAPE_CYCLE: int = 10
_WIDE_SHAPE: int = 0
_CTE_CHAIN_SHAPES: int = 4
_WIDE_COLUMNS: int = 300
_CTE_STEPS: int = 6


def write_model_format_project(
    *, project_dir: Path, model_count: int, large_model_lines: int = 0
) -> None:
    """Write deterministic unformatted models of mixed shape, width and length."""

    project_dir.mkdir()
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "format_models"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    models_dir: Path = project_dir / "sql" / "models"
    models_dir.mkdir(parents=True)
    for model_index in range(model_count):
        models_dir.joinpath(f"orders_{model_index:05d}.sql").write_text(
            'MODEL (description "Generated order model");\n\n'
            + _model_sql(model_index=model_index),
            encoding="utf-8",
        )
    if large_model_lines:
        models_dir.joinpath("large_orders.sql").write_text(
            'MODEL (description "Generated large order model");\n\n'
            + _large_model_sql(line_count=large_model_lines),
            encoding="utf-8",
        )


def _model_sql(*, model_index: int) -> str:
    upstream: str = (
        f'__ref("orders_{model_index - 1:05d}")'
        if model_index
        else "(select 1 as order_id, 2 as customer_id, 3 as amount)"
    )
    shape: int = model_index % _SHAPE_CYCLE
    if shape == _WIDE_SHAPE:
        columns: str = ", ".join(
            f"coalesce(o.amount, 0) * {column} as amount_{column}"
            for column in range(_WIDE_COLUMNS)
        )
        return f"select o.order_id, {columns} from {upstream} o\n"
    if shape < _CTE_CHAIN_SHAPES:
        ctes: str = ",\n".join(
            f"step_{step} as (select order_id, customer_id, amount + {step} as amount "
            f"from {'base' if step == 0 else f'step_{step - 1}'} "
            f"where amount > {step} -- keep positive amounts\n)"
            for step in range(_CTE_STEPS)
        )
        return (
            f"with base as (select order_id, customer_id, amount from {upstream}),\n{ctes}\n"
            "select order_id, customer_id, sum(amount) over (partition by customer_id "
            "order by order_id rows between unbounded preceding and current row) as amount, "
            "case when amount > 100 then 'large' when amount > 10 then 'medium' "
            "else 'small' end as size_band from step_5\n"
        )
    return (
        "/* orders slice */\n"
        f"select o.order_id, o.customer_id,o.amount, 'order {model_index}' as label "
        f"from {upstream} o where o.amount>{model_index} and o.customer_id is not null "
        "or o.order_id in (1,2,3,4,5,6,7,8)\n"
    )


def _large_model_sql(*, line_count: int) -> str:
    steps: str = ",\n".join(
        f"step_{index} as (select order_id, customer_id, amount + {index} as amount "
        f"from {'base' if index == 0 else f'step_{index - 1}'} -- step {index}\n)"
        for index in range(line_count // 2)
    )
    return (
        "with base as (select 1 as order_id, 2 as customer_id, 3 as amount),\n"
        f"{steps}\nselect order_id, customer_id, amount from step_{line_count // 2 - 1}\n"
    )
