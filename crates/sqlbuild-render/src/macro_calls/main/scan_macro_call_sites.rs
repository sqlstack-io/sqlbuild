//! Top-level macro call sites of one authored SQL string, as Python's expansion loop visits them.

use crate::macro_calls::_helpers::offsets::char_offsets;
use crate::macro_calls::_helpers::scanner::{
    call_name, mentions_typed_reference, nested_names, python_argument_syntax, top_level_calls,
};
use crate::macro_calls::models::{MacroCallScan, MacroCallSite, MacroScanFailure};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// Every complete top-level call site in order, and where Python's scan raises, if it does.
pub fn scan_macro_call_sites(sql: &str, syntax: Option<&LexicalSyntax>) -> MacroCallScan {
    let (calls, failure) = top_level_calls(sql, syntax);
    let mut boundaries: Vec<usize> = calls
        .iter()
        .flat_map(|call| [call.start, call.open + 1, call.close + 1])
        .collect();
    let failed_call: Option<usize> = failure.as_ref().and_then(|failure| failure.call_start);
    boundaries.extend(failed_call);
    let offsets = char_offsets(sql, &boundaries);
    let sites: Vec<MacroCallSite> = calls
        .into_iter()
        .zip(offsets.as_chunks::<3>().0)
        .map(|(call, offsets)| {
            let args = &sql[call.open + 1..call.close];
            let python_syntax: LexicalSyntax = python_argument_syntax();
            let argument_syntax: Option<&LexicalSyntax> = if call.python {
                Some(&python_syntax)
            } else {
                syntax
            };
            let tree_names: Option<Vec<String>> =
                call_tree_names(&call.name, args, argument_syntax);
            MacroCallSite {
                start: offsets[0],
                args_start: offsets[1],
                end: offsets[2],
                name: call.name,
                tree_names,
                typed_reference_text: mentions_typed_reference(args),
            }
        })
        .collect();
    let failure: Option<MacroScanFailure> = failure.map(|failure| MacroScanFailure {
        call_start: failure.call_start.and(offsets.last().copied()),
        call_name: failed_call_name(sql, failure.call_start),
        error: failure.error,
    });
    MacroCallScan { sites, failure }
}

/// The name of the call at byte `start` that Python's evaluation raises in, if any.
fn failed_call_name(sql: &str, start: Option<usize>) -> Option<String> {
    Some(call_name(sql, start?))
}

/// The call's name, then each nested call's once; None where a nested scan raises.
fn call_tree_names(name: &str, args: &str, syntax: Option<&LexicalSyntax>) -> Option<Vec<String>> {
    let nested: Vec<String> = match nested_names(args, syntax) {
        Ok(nested) => nested,
        Err(_scan_error) => return None,
    };
    let mut tree_names: Vec<String> = vec![name.to_owned()];
    for nested_name in nested {
        if !tree_names.contains(&nested_name) {
            tree_names.push(nested_name);
        }
    }
    Some(tree_names)
}
