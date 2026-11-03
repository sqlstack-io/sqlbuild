//! Login resolves its native command options without project discovery or invocation capture.

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::decision::{flag, text};
use crate::native_commands::_helpers::terminal::{finish_lost_stdout, write_output};
use crate::person_login::main::run_login::run_login;
use crate::person_login::models::{LoginAction, LoginOptions};

pub(crate) fn run_login_invocation(namespace: &Namespace) -> Option<i32> {
    let command = text(namespace, "command")?;
    let action = match command.as_str() {
        "logout" => LoginAction::Logout,
        "login" if text(namespace, "login_action").as_deref() == Some("status") => {
            LoginAction::Status
        }
        "login" => LoginAction::Start,
        _ => return None,
    };
    Some(
        match run_login(
            LoginOptions {
                action,
                server: text(namespace, "login_server").unwrap_or_default(),
                ca: text(namespace, "login_ca").map(Into::into),
                no_browser: flag(namespace, "login_no_browser"),
            },
            |message| {
                drop(write_output(&[(Stream::Stderr, message)]));
            },
        ) {
            Ok(Some(identity)) => match serde_json::to_string(&identity) {
                Ok(identity) => {
                    let lost = write_output(&[(Stream::Stdout, &format!("{identity}\n"))]);
                    finish_lost_stdout(lost.as_ref(), 0)
                }
                Err(_) => 1,
            },
            Ok(None) => 0,
            Err(error) => {
                drop(write_output(&[(
                    Stream::Stderr,
                    &format!("SQLBuild CLI {command} failed: {error}\n"),
                )]));
                1
            }
        },
    )
}
