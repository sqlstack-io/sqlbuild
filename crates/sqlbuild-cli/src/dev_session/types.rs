//! The shapes a `sqb dev` session passes around.

use std::future::Future;
use std::pin::Pin;

use sqlbuild_core::screen_text::models::WarehouseState;

/// A foreground connection that is still open.
pub(crate) type Connecting<'a> = Pin<Box<dyn Future<Output = WarehouseState> + Send + 'a>>;
