use crate::loads::main::load_message::load_message;
use crate::loads::tests::test_types::LoadMessageTestCase;

#[test]
fn given_a_sqlload_message_about_a_load_when_shown_then_its_settings_are_named_under_load() {
    let test_cases = [
        LoadMessageTestCase {
            description: "the source's own table and one of its tables",
            message: "add write_strategy = \"replace\" under [sources.shop] for all its tables, or \
                      under [sources.shop.tables.\"public.orders\"] for this one",
            expected_message: "add write_strategy = \"replace\" under [load] for all its tables, \
                               or under [load.tables.\"public.orders\"] for this one",
        },
        LoadMessageTestCase {
            description: "another source's table keeps its name",
            message: "under [sources.shopping]",
            expected_message: "under [sources.shopping]",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            load_message("shop", test_case.message),
            test_case.expected_message,
            "{}",
            test_case.description
        );
    }
}
