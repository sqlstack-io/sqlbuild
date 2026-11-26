use crate::loads::main::yaml_line::yaml_line;
use crate::loads::tests::test_types::YamlLineTestCase;

const SOURCES_FILE: &str = "load:\n  name: shop\n  kind: postgres\n  tables:\n    public.orders:\n      unique_key: id\nsources:\n  - name: shop__orders\n";

#[test]
fn given_a_setting_path_when_finding_its_line_then_the_deepest_written_key_is_found() {
    let test_cases = [
        YamlLineTestCase {
            description: "a nested table setting",
            path: &["tables", "public.orders", "unique_key"],
            expected_line: Some(6),
        },
        YamlLineTestCase {
            description: "a setting the file doesn't write points at its nearest written parent",
            path: &["tables", "public.orders", "cursor"],
            expected_line: Some(5),
        },
        YamlLineTestCase {
            description: "no path points at the load",
            path: &[],
            expected_line: Some(1),
        },
    ];
    for test_case in test_cases {
        let path: Vec<String> = test_case
            .path
            .iter()
            .map(|segment| (*segment).to_owned())
            .collect();
        assert_eq!(
            yaml_line(SOURCES_FILE, &path).map_err(|error| error.message),
            Ok(test_case.expected_line),
            "{}",
            test_case.description
        );
    }
}
