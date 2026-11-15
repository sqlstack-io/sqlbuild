//! Macros declaring named records that cross the Python boundary as positional tuples.

/// Implement the positional traits for another crate's struct, its fields in tuple order.
macro_rules! positional_fields {
    ($($mode:ident),+; $name:ident $body:tt) => {
        $(
            positional_fields!(
                @$mode $name $body
            );
        )+
    };
    (@extract $name:ident { $($field:ident : $ty:ty),+ $(,)? }) => {
        impl $crate::types::FromPositional for $name {
            fn from_positional(value: &pyo3::Bound<'_, pyo3::PyAny>) -> pyo3::PyResult<Self> {
                let ($($field,)+): ($($ty,)+) = pyo3::types::PyAnyMethods::extract(value)?;
                Ok($name { $($field),+ })
            }
        }
    };
    (@into $name:ident { $($field:ident : $ty:ty),+ $(,)? }) => {
        impl $crate::types::IntoPositional for $name {
            fn into_positional<'py>(
                self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyTuple>> {
                let $name { $($field),+ } = self;
                pyo3::IntoPyObject::into_pyobject(($($field,)+), py)
            }
        }
    };
}

/// Declare a struct naming the positions of a tuple Python sends (`extract`) or receives (`into`).
#[macro_export]
macro_rules! positional_record {
    (
        $($mode:ident),+;
        $(#[$meta:meta])*
        $vis:vis struct $name:ident <$lifetime:lifetime> $body:tt
    ) => {
        $(#[$meta])*
        $vis struct $name<$lifetime> $body

        $(
            $crate::positional_record!(
                @$mode $name <$lifetime> $body
            );
        )+
    };
    (
        $($mode:ident),+;
        $(#[$meta:meta])*
        $vis:vis struct $name:ident $body:tt
    ) => {
        $(#[$meta])*
        $vis struct $name $body

        $(
            $crate::positional_record!(
                @$mode $name $body
            );
        )+
    };
    (@extract $name:ident {
        $($(#[$field_meta:meta])* $field_vis:vis $field:ident : $ty:ty),+ $(,)?
    }) => {
        impl<'py> pyo3::FromPyObject<'py> for $name {
            fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
                let ($($field,)+): ($($ty,)+) = pyo3::types::PyAnyMethods::extract(value)?;
                Ok(Self { $($field),+ })
            }
        }
    };
    (@extract $name:ident <$lifetime:lifetime> {
        $($(#[$field_meta:meta])* $field_vis:vis $field:ident : $ty:ty),+ $(,)?
    }) => {
        impl<$lifetime> pyo3::FromPyObject<$lifetime> for $name<$lifetime> {
            fn extract_bound(
                value: &pyo3::Bound<$lifetime, pyo3::PyAny>,
            ) -> pyo3::PyResult<Self> {
                let ($($field,)+): ($($ty,)+) = pyo3::types::PyAnyMethods::extract(value)?;
                Ok(Self { $($field),+ })
            }
        }
    };
    (@into $name:ident {
        $($(#[$field_meta:meta])* $field_vis:vis $field:ident : $ty:ty),+ $(,)?
    }) => {
        impl<'py> pyo3::IntoPyObject<'py> for $name {
            type Target = pyo3::types::PyTuple;
            type Output = pyo3::Bound<'py, pyo3::types::PyTuple>;
            type Error = pyo3::PyErr;

            fn into_pyobject(self, py: pyo3::Python<'py>) -> Result<Self::Output, Self::Error> {
                pyo3::IntoPyObject::into_pyobject(($(self.$field,)+), py)
            }
        }
    };
    (@into $name:ident <$lifetime:lifetime> {
        $($(#[$field_meta:meta])* $field_vis:vis $field:ident : $ty:ty),+ $(,)?
    }) => {
        impl<$lifetime> pyo3::IntoPyObject<$lifetime> for $name<$lifetime> {
            type Target = pyo3::types::PyTuple;
            type Output = pyo3::Bound<$lifetime, pyo3::types::PyTuple>;
            type Error = pyo3::PyErr;

            fn into_pyobject(
                self,
                py: pyo3::Python<$lifetime>,
            ) -> Result<Self::Output, Self::Error> {
                pyo3::IntoPyObject::into_pyobject(($(self.$field,)+), py)
            }
        }
    };
}
