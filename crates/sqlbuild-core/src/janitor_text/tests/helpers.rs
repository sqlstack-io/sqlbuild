use crate::janitor_text::models::{
    ArchiveCandidateText, ArchivedRelationText, JanitorItemText, JanitorPhase, JanitorPlanText,
    OldNameViewText, RelationUsageText, UsageCheckText,
};

/// A direct-mode plan for `dev` that archives one relation and deletes one archive.
pub(super) fn direct_archive_plan() -> JanitorPlanText {
    JanitorPlanText {
        target_name: Some("dev".to_owned()),
        retention_days: 14,
        direct_mode: true,
        archive_retention_days: 14,
        archive_candidates: vec![ArchiveCandidateText {
            name: "dev.old_orders".to_owned(),
            ..ArchiveCandidateText::default()
        }],
        archive_deletions: vec![ArchivedRelationText {
            name: "dev.old_products".to_owned(),
            ..ArchivedRelationText::default()
        }],
        ..JanitorPlanText::default()
    }
}

/// A finished action of `phase`, failed when `error` is given.
pub(super) fn item(
    phase: JanitorPhase,
    (action, name, destination): (&str, &str, Option<&str>),
    error: Option<&str>,
) -> JanitorItemText {
    JanitorItemText {
        phase,
        action: action.to_owned(),
        name: name.to_owned(),
        destination: destination.map(str::to_owned),
        error: error.map(str::to_owned),
        duration_seconds: Some(0.5),
    }
}

/// A direct-mode plan archiving `count` relations, with `skipped` objects for two reasons.
pub(super) fn large_plan(count: usize, skipped: usize) -> JanitorPlanText {
    JanitorPlanText {
        target_name: Some("dev".to_owned()),
        retention_days: 14,
        direct_mode: true,
        archive_retention_days: 14,
        archive_candidates: (0..count)
            .map(|index| ArchiveCandidateText {
                name: format!("dev.old_orders_{index:03}"),
                archive_name: format!("dev._sqb_archive__20260924t101500z__old_orders_{index:03}"),
                ..ArchiveCandidateText::default()
            })
            .collect(),
        skipped_relations: (0..skipped)
            .map(|index| {
                let reasons: [&str; 2] =
                    ["relation is newer than 14 days", "not tracked by SQLBuild"];
                let reason: &str = reasons[usize::from(index % 3 != 0)];
                (format!("dev.recent_{index:03}"), reason.to_owned())
            })
            .collect(),
        ..JanitorPlanText::default()
    }
}

/// A plan with `count` live old-name views expiring on one day.
pub(super) fn old_name_plan(count: usize) -> JanitorPlanText {
    JanitorPlanText {
        old_name_live: (0..count)
            .map(|index| OldNameViewText {
                name: format!("dev.orders_v{index:02}"),
                destination_model: format!("orders_{index:02}"),
                expires_on: Some("2026-11-05".to_owned()),
                ..OldNameViewText::default()
            })
            .collect(),
        ..JanitorPlanText::default()
    }
}

/// Reads by `readers` of `query_count` queries, last on 2026-10-21.
fn usage(readers: &[&str], query_count: i64) -> RelationUsageText {
    RelationUsageText {
        last_read_at: "2026-10-21 09:30:00".to_owned(),
        readers: readers.iter().map(|reader| (*reader).to_owned()).collect(),
        query_count,
        via: None,
    }
}

/// An in-use view, an in-use table read by five users, and an unread table.
fn usage_candidates() -> [ArchiveCandidateText; 3] {
    [
        ArchiveCandidateText {
            name: "dev.orders_daily".to_owned(),
            archive_name: "dev._sqb_archive__20261022t101500z__orders_daily".to_owned(),
            usage: Some(usage(&["svc_reporting"], 14)),
            ..ArchiveCandidateText::default()
        },
        ArchiveCandidateText {
            name: "dev.old_orders".to_owned(),
            archive_name: "dev._sqb_archive__20261022t101500z__old_orders".to_owned(),
            is_table: true,
            changed_on: Some("2026-09-01".to_owned()),
            usage: Some(usage(
                &["svc_reporting", "alice", "bob", "carol", "dave"],
                40,
            )),
            ..ArchiveCandidateText::default()
        },
        ArchiveCandidateText {
            name: "dev.old_customers".to_owned(),
            archive_name: "dev._sqb_archive__20261022t101500z__old_customers".to_owned(),
            is_table: true,
            ..ArchiveCandidateText::default()
        },
    ]
}

const CHAINS_NOT_FOLLOWED: &str = "view chains not followed: grant SNOWFLAKE.OBJECT_VIEWER";

/// A plan for `dev` whose usage was checked over `7d`.
fn checked_plan(keep_in_use: bool) -> JanitorPlanText {
    JanitorPlanText {
        target_name: Some("dev".to_owned()),
        retention_days: 14,
        direct_mode: true,
        archive_retention_days: 14,
        usage_check: Some(UsageCheckText {
            checked: true,
            reason: None,
            window: "7d".to_owned(),
            keep_in_use,
            ..UsageCheckText::default()
        }),
        ..JanitorPlanText::default()
    }
}

/// A checked plan archiving every usage candidate, in use or not.
pub(super) fn usage_plan() -> JanitorPlanText {
    JanitorPlanText {
        archive_candidates: usage_candidates().to_vec(),
        ..checked_plan(false)
    }
}

/// A checked plan archiving a table read through four named views and a view read through one.
pub(super) fn via_usage_plan() -> JanitorPlanText {
    let views: Vec<String> = ["daily", "weekly", "monthly", "yearly"]
        .iter()
        .map(|grain| format!("dev.orders_{grain}"))
        .collect();
    JanitorPlanText {
        archive_candidates: vec![
            ArchiveCandidateText {
                name: "dev.orders_base".to_owned(),
                is_table: true,
                changed_on: Some("2026-09-01".to_owned()),
                usage: Some(RelationUsageText {
                    via: Some(views),
                    ..usage(&["alice"], 3)
                }),
                ..ArchiveCandidateText::default()
            },
            ArchiveCandidateText {
                name: "dev.orders_view".to_owned(),
                usage: Some(RelationUsageText {
                    via: Some(Vec::new()),
                    ..usage(&["alice"], 3)
                }),
                ..ArchiveCandidateText::default()
            },
        ],
        ..checked_plan(false)
    }
}

/// A checked plan whose usage source could not follow view chains.
pub(super) fn chains_unfollowed_plan() -> JanitorPlanText {
    JanitorPlanText {
        archive_candidates: usage_candidates().to_vec(),
        usage_check: Some(UsageCheckText {
            checked: true,
            window: "7d".to_owned(),
            notices: vec![CHAINS_NOT_FOLLOWED.to_owned()],
            ..UsageCheckText::default()
        }),
        ..checked_plan(false)
    }
}

/// A checked plan keeping the two in-use candidates and archiving the unread one.
pub(super) fn kept_usage_plan() -> JanitorPlanText {
    let [view, table, unread] = usage_candidates();
    JanitorPlanText {
        archive_candidates: vec![unread],
        kept_in_use: vec![view, table],
        ..checked_plan(true)
    }
}

/// A plan whose usage was not checked because the adapter has no usage source.
pub(super) fn unchecked_plan() -> JanitorPlanText {
    JanitorPlanText {
        usage_check: Some(UsageCheckText {
            checked: false,
            reason: Some("unsupported adapter".to_owned()),
            window: "7d".to_owned(),
            ..UsageCheckText::default()
        }),
        ..direct_archive_plan()
    }
}
