use crate::argv::main::given_setting_dests::given_setting_dests;
use crate::argv::tests::test_types::GivenSettingsTestCase;

#[test]
fn given_a_command_line_when_reading_its_own_settings_then_every_written_option_counts() {
    let test_cases = [
        GivenSettingsTestCase {
            description: "long options, with = or a separate value, and global options",
            argv: &[
                "--no-color",
                "build",
                "--vars={\"a\": \"1\"}",
                "--concurrency",
                "3",
                "orders_job",
            ],
            expected_given: Some(&["concurrency", "no_color", "vars"]),
        },
        GivenSettingsTestCase {
            description: "an abbreviated option names its full option's setting",
            argv: &["build", "--concur", "3", "--select", "job:orders_job"],
            expected_given: Some(&["concurrency", "select"]),
        },
        GivenSettingsTestCase {
            description: "a negative flag gives the setting it shares",
            argv: &["plan", "--no-ignore-audit-failures", "--no-job-settings"],
            expected_given: Some(&["ignore_audit_failures", "job_settings"]),
        },
        GivenSettingsTestCase {
            description: "an option gives every setting of its mutually exclusive group",
            argv: &["build", "--reload", "orders_job"],
            expected_given: Some(&["load_sources", "reload"]),
        },
        GivenSettingsTestCase {
            description: "a selection alone gives no other setting",
            argv: &["build", "orders_job"],
            expected_given: Some(&[]),
        },
        GivenSettingsTestCase {
            description: "a command line that does not parse",
            argv: &["build", "--no-such-option"],
            expected_given: None,
        },
    ];
    for test_case in test_cases {
        let argv: Vec<String> = test_case.argv.iter().map(|arg| (*arg).to_owned()).collect();
        let given: Option<Vec<String>> =
            given_setting_dests(&argv).map(|dests| dests.into_iter().collect());
        let given: Option<Vec<&str>> = given
            .as_ref()
            .map(|dests| dests.iter().map(String::as_str).collect());
        assert_eq!(
            given.as_deref(),
            test_case.expected_given,
            "{}",
            test_case.description
        );
    }
}
