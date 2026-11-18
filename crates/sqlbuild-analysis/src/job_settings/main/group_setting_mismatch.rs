//! A group's members must agree on the build-wide settings that can change any compiled SQL.

use std::collections::BTreeMap;

use serde_json::Value as Json;

use crate::job_settings::constants::GROUP_AGREED_SETTINGS;
use crate::job_settings::models::DeclaredRun;

/// Why `members` cannot run as one build, naming the setting and two members.
#[must_use]
pub fn group_setting_mismatch(
    members: &[String],
    declared: &BTreeMap<String, DeclaredRun>,
) -> Option<String> {
    let value = |job: &str, key: &str| -> Option<Json> {
        match declared.get(job) {
            Some(DeclaredRun::Job { settings, .. }) => settings.get(key).cloned(),
            _ => None,
        }
    };
    for key in GROUP_AGREED_SETTINGS {
        let mut members = members.iter();
        let Some(first) = members.next() else {
            continue;
        };
        let expected: Option<Json> = value(first, key);
        if let Some(other) = members.find(|member| value(member, key) != expected) {
            return Some(format!(
                "jobs {first} and {other} set {key} differently; a group runs its jobs as one \
                 build, which has one {key}"
            ));
        }
    }
    None
}
