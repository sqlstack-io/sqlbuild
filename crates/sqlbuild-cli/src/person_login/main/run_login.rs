//! Person login commands run before project discovery and never write credentials into a project.

use crate::person_login::_helpers::{credentials, flow, transport};
use crate::person_login::models::{LoginAction, LoginOptions};
use sqlbuild_orchestration::api::main::parse_origin::parse_origin;
use sqlbuild_orchestration::credentials::models::SessionIdentity;

pub(crate) fn run_login(
    options: LoginOptions,
    report: impl Fn(&str),
) -> Result<Option<SessionIdentity>, std::io::Error> {
    let origin = parse_origin(&options.server, "CLI").map_err(std::io::Error::other)?;
    let (logout_credential, cleanup) = if options.action == LoginAction::Logout {
        report("Signing out of SQLBuild CLI…\n");
        let credential = credentials::load(&origin);
        let cleanup = credentials::remove(&origin);
        match credential {
            Ok(Some(credential)) => (Some(credential), cleanup),
            Ok(None) => {
                cleanup?;
                report("SQLBuild CLI signed out successfully\n");
                return Ok(None);
            }
            Err(error) => {
                cleanup?;
                return Err(error);
            }
        }
    } else {
        (None, Ok(()))
    };
    let client = transport::connect(&origin, options.ca.as_deref())?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        if let Some(credential) = logout_credential {
            let remote = transport::revoke(&client, &credential).await;
            cleanup?;
            remote?;
            report("SQLBuild CLI signed out successfully\n");
            return Ok(None);
        }
        if options.action == LoginAction::Status {
            report("Checking SQLBuild CLI session…\n");
            let credential = credentials::load(&client.origin)?
                .ok_or_else(|| std::io::Error::other("not signed in; run sqb login"))?;
            let identity = transport::current(&client, &credential).await?;
            report("SQLBuild CLI session verified successfully\n");
            return Ok(Some(identity));
        }
        report("Starting SQLBuild CLI sign-in…\n");
        let credential = flow::login(&client, options.no_browser, &report).await?;
        match credentials::save(&client.origin, &credential) {
            Ok(true) => (),
            Ok(false) => report("OS credential store unavailable; using an owner-only credential file in your user configuration directory.\n"),
            Err(error) => {
                let _ = transport::revoke(&client, &credential).await;
                return Err(error);
            }
        }
        if let Err(error) = transport::current(&client, &credential).await {
            let _ = transport::revoke(&client, &credential).await;
            credentials::remove(&client.origin)?;
            return Err(error);
        }
        report("SQLBuild CLI signed in successfully\n");
        Ok::<_, std::io::Error>(None)
    })
}
