use crate::refactoring::_helpers::edits::text_edits::apply_text_edits;
use crate::refactoring::_helpers::planning::job_selectors::job_selector_parts;
use crate::refactoring::_helpers::scanning::chars::chars;
use crate::refactoring::errors::RefactorError;
use crate::refactoring::models::ModelFacts;
use crate::refactoring::tests::helpers::{model, write_files};
use crate::refactoring::tests::test_types::JobSelectorTestCase;

const JOB: &str = "orchestration/jobs/orders/refresh.toml";

/// A model the refactoring leaves alone keeps its name and path.
const STAYS: (&str, &str, &str, &str) = ("customers", "models/marts/customers.sql", "", "");

#[test]
fn given_job_selectors_when_models_are_renamed_or_moved_then_exact_parts_follow()
-> Result<(), RefactorError> {
    let test_cases = [
        JobSelectorTestCase {
            description: "exact names follow; lookalikes, globs, tags and other kinds stay",
            models: &[
                (
                    "orders",
                    "models/marts/orders.sql",
                    "sales",
                    "models/marts/sales.sql",
                ),
                STAYS,
            ],
            job: "select = \"+orders+ orders_x tag:orders source:orders job:orders orders* order?s orders,customers  orders~customers +customers~orders+\"\n",
            expected_job: "select = \"+sales+ orders_x tag:orders source:orders job:orders orders* order?s sales,customers  sales~customers +customers~sales+\"\n",
            expected_manual: &[],
        },
        JobSelectorTestCase {
            description: "literal and escaped strings keep their style",
            models: &[
                (
                    "orders",
                    "models/marts/orders.sql",
                    "sales",
                    "models/marts/sales.sql",
                ),
                STAYS,
            ],
            job: "select = 'orders'\nexclude = \"orders \\u0074ag:x\"\n",
            expected_job: "select = 'sales'\nexclude = \"sales tag:x\"\n",
            expected_manual: &[],
        },
        JobSelectorTestCase {
            description: "a folder every model leaves moves, nested folders included",
            models: &[
                (
                    "orders",
                    "models/staging/orders.sql",
                    "orders",
                    "models/core/orders.sql",
                ),
                (
                    "lines",
                    "models/staging/sub/lines.sql",
                    "lines",
                    "models/core/sub/lines.sql",
                ),
                STAYS,
            ],
            job: "select = \"models/staging/ path:models/staging +models/staging/sub+ models/stagingx models\"\n",
            expected_job: "select = \"models/core/ path:models/core +models/core/sub+ models/stagingx models\"\n",
            expected_manual: &[],
        },
        JobSelectorTestCase {
            description: "a folder only some models leave stays and is reported",
            models: &[
                (
                    "orders",
                    "models/staging/orders.sql",
                    "orders",
                    "models/core/orders.sql",
                ),
                ("lines", "models/staging/lines.sql", "", ""),
            ],
            job: "name = \"x_job\"\nselect = \"models/staging orders\"\n",
            expected_job: "name = \"x_job\"\nselect = \"models/staging orders\"\n",
            expected_manual: &[
                "2:10 this job selects folder 'models/staging', and orders moved out of it; check the job still selects what it should",
            ],
        },
        JobSelectorTestCase {
            description: "a folder moving into one that holds other models stays and is reported",
            models: &[
                (
                    "orders",
                    "models/staging/orders.sql",
                    "orders",
                    "models/marts/orders.sql",
                ),
                STAYS,
            ],
            job: "name = \"x_job\"\nselect = \"models/staging\"\nexclude = \"models/staging\"\n",
            expected_job: "name = \"x_job\"\nselect = \"models/staging\"\nexclude = \"models/staging\"\n",
            expected_manual: &[
                "2:10 this job selects folder 'models/staging', and orders moved out of it; check the job still selects what it should",
                "3:11 this job selects folder 'models/staging', and orders moved out of it; check the job still selects what it should",
            ],
        },
        JobSelectorTestCase {
            description: "a multi-line selector is reported instead of rewritten",
            models: &[(
                "orders",
                "models/marts/orders.sql",
                "sales",
                "models/marts/sales.sql",
            )],
            job: "select = \"\"\"\norders\"\"\"\n",
            expected_job: "select = \"\"\"\norders\"\"\"\n",
            expected_manual: &[
                "1:10 this selector is a multi-line string; change it to 'sales' by hand",
            ],
        },
    ];
    for test_case in test_cases {
        let project =
            tempfile::tempdir().map_err(|error| RefactorError::value(error.to_string()))?;
        write_files(project.path(), &[(JOB, test_case.job)])?;
        let models: Vec<ModelFacts> = test_case
            .models
            .iter()
            .map(|(name, path, _, _)| ModelFacts {
                path: (*path).to_owned(),
                ..model(name, Some("table"), "analytics")
            })
            .collect();
        let renamed: Vec<(&ModelFacts, &str, &str)> = test_case
            .models
            .iter()
            .zip(&models)
            .filter(|((_, _, new_name, _), _)| !new_name.is_empty())
            .map(|((_, _, new_name, new_path), facts)| (facts, *new_name, *new_path))
            .collect();
        let parts = job_selector_parts(project.path(), &models, &renamed)?;
        let edits = parts
            .edits
            .into_iter()
            .map(|(_, edit)| edit)
            .collect::<Vec<_>>();
        let manual: Vec<String> = parts
            .manual
            .iter()
            .chain(&parts.notes)
            .map(|location| {
                format!(
                    "{}:{} {}",
                    location.line.unwrap_or_default(),
                    location.column.unwrap_or_default(),
                    location.reason
                )
            })
            .collect();
        assert_eq!(
            apply_text_edits(&chars(test_case.job), &edits)?
                .into_iter()
                .collect::<String>(),
            test_case.expected_job,
            "{}",
            test_case.description
        );
        assert_eq!(
            manual, test_case.expected_manual,
            "{}",
            test_case.description
        );
    }
    Ok(())
}
