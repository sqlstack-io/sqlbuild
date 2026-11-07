pub(crate) struct ForwardedArgumentsTestCase {
    pub(crate) description: &'static str,
    pub(crate) arguments: &'static [&'static str],
    pub(crate) expected_lines: &'static [&'static str],
}

pub(crate) struct ForwardedExitTestCase {
    pub(crate) description: &'static str,
    pub(crate) exit: i32,
    pub(crate) expected_code: i32,
}

pub(crate) struct ForwardedStreamsTestCase {
    pub(crate) description: &'static str,
    pub(crate) stdin: &'static str,
    pub(crate) expected_stdout: &'static str,
    pub(crate) expected_stderr: &'static str,
}

pub(crate) struct MissingSqbTestCase {
    pub(crate) description: &'static str,
    pub(crate) arguments: &'static [&'static str],
    pub(crate) expected_code: Option<i32>,
    pub(crate) expected_stdout: &'static str,
    pub(crate) expected_stderr_start: String,
}

#[cfg(unix)]
pub(crate) struct ForwardedSignalTestCase {
    pub(crate) description: &'static str,
    pub(crate) signal: &'static str,
    pub(crate) expected_signal: Option<i32>,
}

#[cfg(windows)]
pub(crate) struct ForwardedBreakTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_result: &'static str,
}
