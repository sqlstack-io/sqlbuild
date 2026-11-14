use crate::consumption::main::recent_executions::recent_executions;
use crate::consumption::tests::helpers::recorded_row;
use crate::consumption::tests::test_types::RecentExecutionsTestCase;

#[test]
fn given_more_executions_than_the_limit_when_choosing_recent_then_the_newest_starts_win_per_relation()
 {
    // `orders-1` started first but wrote its last row after every other execution.
    let mut late_finisher = Vec::new();
    for (execution, started, finished) in [(1, 10, 50), (2, 20, 25), (3, 30, 35), (4, 40, 45)] {
        late_finisher.push(recorded_row(
            "orders",
            &format!("orders-{execution}"),
            started,
        ));
        late_finisher.push(recorded_row(
            "orders",
            &format!("orders-{execution}"),
            finished,
        ));
    }
    late_finisher.push(recorded_row("customers", "customers-1", 1));
    let test_cases = [
        RecentExecutionsTestCase {
            description: "an execution that started first but wrote last is the oldest; other relations keep theirs",
            rows: late_finisher,
            limit: 2,
            expected_executions: vec!["orders-3", "orders-4", "customers-1"],
            expected_rows: 5,
        },
        RecentExecutionsTestCase {
            description: "equal start times keep the larger execution id, as the warehouse query does",
            rows: vec![
                recorded_row("orders", "orders-a", 10),
                recorded_row("orders", "orders-b", 10),
            ],
            limit: 1,
            expected_executions: vec!["orders-b"],
            expected_rows: 1,
        },
    ];
    for test_case in test_cases {
        let kept = recent_executions(&test_case.rows, test_case.limit);
        let mut executions: Vec<&str> = kept
            .iter()
            .map(|row| row.row.execution_id.as_str())
            .collect();
        executions.dedup();
        assert_eq!(
            executions, test_case.expected_executions,
            "{}",
            test_case.description
        );
        assert_eq!(
            kept.len(),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}
