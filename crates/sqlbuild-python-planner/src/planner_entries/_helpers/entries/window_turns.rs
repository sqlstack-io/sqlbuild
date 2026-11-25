//! An explicit cursor window fitted to each model's floor: planned, clamped with a warning, skipped, or a planning error; nothing to build at all is P001.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;
use sqlbuild_python_base::records::types::ErrorId;

use crate::planner_entries::_helpers::entries::planning_error::{
    planning_error, window_clamp_warning, window_skip,
};
use crate::planner_entries::models::{WindowTally, WindowTurn};
use crate::planner_resolution::main::cursor::config_str::config_str;
use crate::planner_resolution::main::cursor::model_cursor_overrides::model_cursor_overrides;
use crate::planner_resolution::models::WindowFit;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;

/// Fit the explicit window to `model`'s floor, then plan it with `plan` unless the window ends before the floor.
pub(crate) fn window_turn<'py>(
    model: &Bound<'py, PyAny>,
    (cursor_overrides, overrides): (&Bound<'py, PyAny>, (Bound<'py, PyAny>, Bound<'py, PyAny>)),
    plan: impl FnOnce(
        (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)>,
) -> PyResult<WindowTurn<'py>> {
    let (window, fit) = model_cursor_overrides(model, cursor_overrides, overrides)?;
    if let WindowFit::BeforeFloor { floor, window } = &fit {
        return Ok(WindowTurn::Skipped(
            window_skip(model, floor)?,
            (floor.clone(), window.clone()),
        ));
    }
    let clamped = window_clamp_warning(model, &fit)?;
    Ok(WindowTurn::Planned(plan(window)?, clamped))
}

impl WindowTally {
    /// The planned row and its warnings to finish; a skipped or failed model's row goes straight to `entries`, and `None`.
    pub(crate) fn take<'py>(
        &mut self,
        (entries, warnings): (&mut Vec<Bound<'py, PyAny>>, &mut Vec<Bound<'py, PyAny>>),
        model: &Bound<'py, PyAny>,
        turn: PyResult<WindowTurn<'py>>,
    ) -> PyResult<Option<(Bound<'py, PyAny>, Bound<'py, PyAny>)>> {
        match turn {
            Ok(WindowTurn::Planned(planned, clamped)) => {
                warnings.extend(clamped);
                self.cursor_models_planned += usize::from(!config_str(model, "cursor")?.is_none());
                Ok(Some(planned))
            }
            Ok(WindowTurn::Skipped(entry, (floor, window))) => {
                let name = name_object(model)?.str()?.to_string();
                entries.push(entry);
                self.window_skips
                    .push((format!("{name} (cursor_start {floor})"), window));
                Ok(None)
            }
            Err(error) => {
                let (entry, warning) = planning_error(model, error)?;
                entries.push(entry);
                warnings.push(warning);
                Ok(None)
            }
        }
    }

    /// P001 when the explicit window skipped every selected cursor model, naming the window and each one's floor.
    pub(crate) fn refuse_empty_window(&self, py: Python<'_>) -> PyResult<()> {
        let Some((_, window)) = self.window_skips.first() else {
            return Ok(());
        };
        if self.cursor_models_planned > 0 {
            return Ok(());
        }
        let floors: Vec<&str> = self
            .window_skips
            .iter()
            .map(|(model, _)| model.as_str())
            .collect();
        Err(new_plain_error(
            py,
            ErrorId::CompileInput,
            &format!(
                "the explicit cursor window {window} leaves nothing to build: it ends at or before the cursor_start of every selected cursor model: {}",
                floors.join(", ")
            ),
        ))
    }
}
