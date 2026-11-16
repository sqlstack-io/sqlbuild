//! Record each discovered model header's keys as digests, so a delta compile can tell which keys an edit changed.

use sqlbuild_discovery::header_changes::main::header_key_digests::header_key_digests;
use sqlbuild_discovery::header_changes::types::HeaderKeys;
use sqlbuild_sqltext::compiler::models::AuthoredValue;

use sqlbuild_python_base::compile_context::main::delta::capture::capture;
use sqlbuild_python_base::compile_context::main::delta::capturing::capturing;

/// Record one discovered model's header keys for the image.
pub(crate) fn record_header_keys(relative_path: &str, values: &[(String, AuthoredValue)]) {
    if !capturing() {
        return;
    }
    let keys: HeaderKeys = header_key_digests(values);
    let path: String = relative_path.replace('\\', "/");
    capture(|capture| {
        capture.header_keys.insert(path, keys);
    });
}
