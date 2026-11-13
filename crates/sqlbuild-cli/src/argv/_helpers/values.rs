//! `_get_values`, `_get_value` and `_check_value`, and how `ArgumentError` names an action.

use sqlbuild_core::text::main::python_str_repr::python_str_repr;

use crate::argv::_helpers::parsing::Run;
use crate::argv::constants::PATH_CONVERTER;
use crate::argv::models::{ActionSpec, Default, Nargs, TypeFailure, Value};

/// `_get_action_name`: how errors name an action, if it has a name.
pub(crate) fn action_name(action: &ActionSpec) -> Option<String> {
    if !action.option_strings.is_empty() {
        return Some(action.option_strings.join("/"));
    }
    if let Some(Some(metavar)) = &action.metavar {
        return Some(metavar.clone());
    }
    if let Some(dest) = &action.dest {
        return Some(dest.clone());
    }
    action
        .choices
        .as_ref()
        .filter(|choices| !choices.is_empty())
        .map(|choices| format!("{{{}}}", choices.join(",")))
}

/// `str(ArgumentError(action, message))`.
pub(crate) fn argument_error(action: &ActionSpec, message: &str) -> String {
    match action_name(action) {
        Some(name) => format!("argument {name}: {message}"),
        None => message.to_owned(),
    }
}

impl Run<'_> {
    /// `_get_values`: the converted value an action is called with, or `None` for `SUPPRESS`.
    pub(crate) fn action_values(
        &mut self,
        action: &ActionSpec,
        args: &[String],
    ) -> Result<Option<Value>, String> {
        let positional: bool = action.option_strings.is_empty();
        if args.is_empty() && matches!(action.nargs, Nargs::Optional | Nargs::Attached) {
            let value: Value = if positional {
                match &action.default {
                    Default::Suppress => return Ok(None),
                    Default::Value(value) => value.clone(),
                }
            } else {
                action.constant.clone()
            };
            return match value {
                Value::Str(text) => self.converted_value(action, &text).map(Some),
                value => Ok(Some(value)),
            };
        }
        if args.is_empty() && action.nargs == Nargs::ZeroOrMore && positional {
            return Ok(match &action.default {
                Default::Suppress => None,
                Default::Value(Value::None) => Some(Value::List(Vec::new())),
                Default::Value(value) => Some(value.clone()),
            });
        }
        if args.len() == 1
            && matches!(
                action.nargs,
                Nargs::Single | Nargs::Optional | Nargs::Attached
            )
        {
            let value: Value = self.converted_value(action, &args[0])?;
            self.check_value(action, &value)?;
            return Ok(Some(value));
        }
        let mut values: Vec<Value> = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.converted_value(action, arg)?);
        }
        let checked: &[Value] = if action.nargs == Nargs::Parser {
            values.get(..1).unwrap_or_default()
        } else {
            &values
        };
        for value in checked {
            self.check_value(action, value)?;
        }
        Ok(Some(Value::List(values)))
    }

    /// `_get_value`: apply the action's `type=` converter.
    pub(crate) fn converted_value(
        &mut self,
        action: &ActionSpec,
        text: &str,
    ) -> Result<Value, String> {
        let Some(converter) = action.converter.as_deref() else {
            return Ok(Value::Str(text.to_owned()));
        };
        if converter == PATH_CONVERTER {
            return Ok(Value::Path(text.to_owned()));
        }
        match self.host.convert(converter, text) {
            Ok(value) => Ok(value),
            Err(TypeFailure::ArgumentType(message)) => Err(argument_error(action, &message)),
            Err(TypeFailure::Invalid) => Err(argument_error(
                action,
                &format!("invalid {converter} value: {}", self.host.repr(text)),
            )),
        }
    }

    /// `_check_value`: a converted value must be one of the action's choices.
    fn check_value(&self, action: &ActionSpec, value: &Value) -> Result<(), String> {
        let Some(choices) = &action.choices else {
            return Ok(());
        };
        let Value::Str(text) = value else {
            return Ok(());
        };
        if choices.iter().any(|choice| choice == text) {
            return Ok(());
        }
        if let Some((_, instead)) = action
            .removed_choices
            .iter()
            .find(|(removed, _)| removed == text)
        {
            return Err(argument_error(
                action,
                &format!("{} was removed; {instead}", self.host.repr(text)),
            ));
        }
        let listed: Vec<String> = choices
            .iter()
            .map(|choice| python_str_repr(choice))
            .collect();
        Err(argument_error(
            action,
            &format!(
                "invalid choice: {} (choose from {})",
                self.host.repr(text),
                listed.join(", ")
            ),
        ))
    }
}
