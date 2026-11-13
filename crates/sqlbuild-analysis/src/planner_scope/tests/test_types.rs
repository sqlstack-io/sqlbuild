//! Loader graph test cases.

#[derive(Debug)]
pub(crate) struct LoaderTestCase {
    pub description: &'static str,
    pub executable: Vec<&'static str>,
    pub expected_selected: Vec<&'static str>,
}

#[derive(Debug)]
pub(crate) struct NodeLoadsTestCase {
    pub description: &'static str,
    pub loaders: Vec<(&'static str, Vec<&'static str>)>,
    pub sources: Vec<(&'static str, Option<&'static str>)>,
    pub upstream: Vec<(&'static str, Vec<&'static str>)>,
    pub expected_loads: Vec<(&'static str, Vec<&'static str>)>,
}

#[derive(Debug)]
pub(crate) struct DeepLoaderTestCase {
    pub description: &'static str,
    pub depth: usize,
    pub expected_count: usize,
}
