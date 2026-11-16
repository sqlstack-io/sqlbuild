//! Why the planner refuses selectors.

use sqlbuild_analysis::graph::errors::SelectorError;

/// A selector `sqb` refuses, or a refusal of the selection as a whole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    Selector(SelectorError),
    Selection(String),
}

impl From<SelectorError> for Refusal {
    fn from(error: SelectorError) -> Self {
        Self::Selector(error)
    }
}

impl From<&str> for Refusal {
    fn from(message: &str) -> Self {
        Self::Selection(message.to_owned())
    }
}

/// The refusal as an ad-hoc check reports it: `CODE: message` for a selector.
impl From<Refusal> for String {
    fn from(refusal: Refusal) -> Self {
        match refusal {
            Refusal::Selector(error) => format!("{}: {}", error.code, error.message),
            Refusal::Selection(message) => message,
        }
    }
}
