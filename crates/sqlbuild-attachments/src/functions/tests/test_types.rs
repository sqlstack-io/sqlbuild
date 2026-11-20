use crate::functions::models::{
    FunctionHeader, FunctionLanguage, FunctionNamespace, HeaderValue, NamespaceInputs,
};

pub(super) struct FunctionHeaderTestCase {
    pub(super) description: &'static str,
    pub(super) header: Vec<(String, HeaderValue)>,
    pub(super) language: FunctionLanguage,
    pub(super) table_function: bool,
    pub(super) expected_header: FunctionHeader,
}

pub(super) struct FunctionNamespaceTestCase {
    pub(super) description: &'static str,
    pub(super) inputs: NamespaceInputs,
    pub(super) expected_namespace: FunctionNamespace,
}

pub(super) struct FunctionReferenceTestCase {
    pub(super) description: &'static str,
    pub(super) references: Vec<(&'static str, &'static str)>,
    pub(super) expected_error: Option<&'static str>,
}

pub(super) struct FrameArgumentTestCase {
    pub(super) description: &'static str,
    pub(super) dataframe: Option<&'static str>,
    pub(super) argument_type: &'static str,
    pub(super) expected_error: Option<&'static str>,
}

pub(super) struct TableFunctionPartitionTestCase {
    pub(super) description: &'static str,
    pub(super) calls: Vec<(&'static str, &'static str, bool)>,
    pub(super) expected_error: Option<&'static str>,
}

pub(super) struct TableFunctionArityTestCase {
    pub(super) description: &'static str,
    pub(super) calls: Vec<(&'static str, &'static str, Option<usize>)>,
    pub(super) expected_error: Option<&'static str>,
}
