use crate::errors::{ConfigError, ErrorClass};
use crate::path_defaults::constants::CONFLICT_HELP;
use crate::path_defaults::main::select_path_default::select_path_default;
use crate::path_defaults::models::PathDefaultChoice;
use crate::path_defaults::tests::test_types::SelectTestCase;

#[test]
fn given_path_default_keys_when_selecting_then_the_python_choice_is_made() {
    let test_cases = [
        SelectTestCase {
            description: "the deepest literal prefix wins",
            model_path: "sql/models/staging/orders/orders.sql",
            keys: &["staging", "staging/orders", "*/orders"],
            expected_choice: PathDefaultChoice::Selected(Some("staging/orders".to_owned())),
        },
        SelectTestCase {
            description: "equally deep literal keys keep the first in sorted order",
            model_path: "staging/orders.sql",
            keys: &["staging/orders.sql", "staging/aaaaa"],
            expected_choice: PathDefaultChoice::Selected(Some("staging/orders.sql".to_owned())),
        },
        SelectTestCase {
            description: "the most literal wildcard wins",
            model_path: "models\\marts\\finance\\revenue.sql",
            keys: &["**/finance", "marts/*", "marts/**/finance"],
            expected_choice: PathDefaultChoice::Selected(Some("marts/**/finance".to_owned())),
        },
        SelectTestCase {
            description: "a recursive glob matches zero segments",
            model_path: "marts/revenue.sql",
            keys: &["marts/**"],
            expected_choice: PathDefaultChoice::Selected(Some("marts/**".to_owned())),
        },
        SelectTestCase {
            description: "equally specific wildcards conflict",
            model_path: "marts/finance/revenue.sql",
            keys: &["*/finance", "marts/*"],
            expected_choice: PathDefaultChoice::Conflict(
                ConfigError::compile(
                    "Model path 'marts/finance/revenue.sql' matches equally specific \
                     path_defaults keys: '*/finance', 'marts/*'."
                        .to_owned(),
                )
                .with_class(ErrorClass::DiscoveryConflict)
                .with_help(CONFLICT_HELP),
            ),
        },
        SelectTestCase {
            description: "a single glob consumes exactly one segment",
            model_path: "sql/models/commerce/eu/partner_feed/staging/orders.sql",
            keys: &["commerce/*/staging"],
            expected_choice: PathDefaultChoice::Selected(None),
        },
        SelectTestCase {
            description: "a single glob outranks a recursive glob",
            model_path: "sql/models/commerce/partner_feed/staging/orders.sql",
            keys: &["commerce/**/staging", "commerce/*/staging"],
            expected_choice: PathDefaultChoice::Selected(Some("commerce/*/staging".to_owned())),
        },
        SelectTestCase {
            description: "a literal prefix outranks a recursive glob on a Windows path",
            model_path: "models\\commerce\\partner_feed\\staging\\orders.sql",
            keys: &["commerce/**/staging", "commerce/partner_feed/staging"],
            expected_choice: PathDefaultChoice::Selected(Some(
                "commerce/partner_feed/staging".to_owned(),
            )),
        },
        SelectTestCase {
            description: "no key matches",
            model_path: "staging/orders.sql",
            keys: &["marts", "*/finance"],
            expected_choice: PathDefaultChoice::Selected(None),
        },
    ];

    for test_case in test_cases {
        let keys: Vec<String> = test_case.keys.iter().map(|key| (*key).to_owned()).collect();

        let choice = select_path_default(test_case.model_path, &keys);

        assert_eq!(
            choice, test_case.expected_choice,
            "{}",
            test_case.description
        );
    }
}
