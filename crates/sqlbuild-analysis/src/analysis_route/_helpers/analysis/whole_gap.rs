use crate::analysis_route::models::ParserGap;

/// A gap `read_whole` names itself, which no parser reports as invalid syntax.
pub(crate) fn whole_gap(construct: &str, offset: usize) -> ParserGap {
    ParserGap {
        construct: construct.to_owned(),
        message: construct.to_owned(),
        offset,
        syntax: false,
    }
}
