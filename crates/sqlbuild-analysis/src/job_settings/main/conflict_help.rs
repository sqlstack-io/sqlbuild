//! The help a job setting conflict gives, from the setting's row in the table.

use crate::job_settings::constants::{RETRY_CONFLICT_HELP, RETRY_SETTING};
use crate::job_settings::main::job_setting_spec::job_setting_spec;
use crate::job_settings::models::{CommandLineValues, SettingScope};

/// How to settle named jobs disagreeing on `setting`; a flag only for the value it can give.
#[must_use]
pub fn conflict_help(setting: &str) -> String {
    let Some(spec) = job_setting_spec(setting) else {
        return if setting == RETRY_SETTING {
            RETRY_CONFLICT_HELP.to_owned()
        } else {
            "make the jobs agree, or build with --no-job-settings".to_owned()
        };
    };
    let only_one: &str = if spec.scope == SettingScope::Node {
        "set it in only one of them, "
    } else {
        ""
    };
    let options: String = spec.options.join(" or ");
    let flag: String = match spec.command_line {
        CommandLineValues::Any => format!("override it with {options}, "),
        CommandLineValues::OnlyTrue => format!("pass {options} to make it true for all, "),
        CommandLineValues::OnlyFalse => format!("pass {options} to make it false for all, "),
    };
    format!("make the jobs agree, {only_one}{flag}or build with --no-job-settings")
}
