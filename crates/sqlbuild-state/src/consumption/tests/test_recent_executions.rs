use crate::consumption::main::recent_executions::recent_executions;
use crate::consumption::models::{ConsumptionAppend, ConsumptionRow, NodeKey, RelationKey};

fn row(relation: &str, execution: &str, recorded_at: i64) -> ConsumptionRow {
    ConsumptionRow {
        row: ConsumptionAppend {
            execution_id: execution.into(),
            phase: "succeeded".into(),
            run_id: format!("run-{execution}"),
            node: NodeKey {
                kind: "model".into(),
                name: relation.into(),
            },
            relation: RelationKey {
                database: None,
                schema: "main".into(),
                name: relation.into(),
            },
            version_hash: "version".into(),
            input_count: 0,
            input: None,
        },
        recorded_at,
    }
}

#[test]
fn given_more_executions_than_the_limit_when_choosing_recent_then_the_newest_starts_win_per_relation()
 {
    let mut rows: Vec<ConsumptionRow> = Vec::new();
    for execution in 1..=4 {
        // `orders-1` started first but wrote its last row after every other execution.
        let finished: i64 = if execution == 1 {
            50
        } else {
            execution * 10 + 5
        };
        rows.push(row(
            "orders",
            &format!("orders-{execution}"),
            execution * 10,
        ));
        rows.push(row("orders", &format!("orders-{execution}"), finished));
    }
    rows.push(row("customers", "customers-1", 1));

    let mut kept: Vec<String> = recent_executions(&rows, 2)
        .into_iter()
        .map(|row| row.row.execution_id)
        .collect();
    kept.dedup();

    assert_eq!(
        kept,
        ["orders-3", "orders-4", "customers-1"],
        "each relation keeps its newest two executions by earliest row, with every row of each"
    );
    assert_eq!(recent_executions(&rows, 2).len(), 5);
}
