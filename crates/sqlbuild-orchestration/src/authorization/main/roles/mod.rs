//! Role definitions, assignments and administrator grants.

pub mod assign_builtin_role;
pub mod change_administrator;
pub mod compile_roles;
pub mod grant_admin;
pub mod list_role_definitions;
pub mod list_role_holdings;
pub(crate) mod person_assigns_roles;
pub(crate) mod publish_custom_roles;
pub(crate) mod publish_group_roles;
