use pyo3::{exceptions::PyValueError, prelude::*};

macro_rules! validate_limit {
    ($value:expr, $field:literal) => {
        usize::try_from($value).map_err(|_| {
            PyValueError::new_err(format!("`{}` must be greater than or equal to 0", $field,))
        })
    };
}

#[pyclass(name = "_AliasLimits", frozen, eq, skip_from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_field_names)]
pub struct AliasLimits {
    #[pyo3(get)]
    pub max_total_replayed_events: usize,
    #[pyo3(get)]
    pub max_replay_stack_depth: usize,
    #[pyo3(get)]
    pub max_alias_expansions_per_anchor: usize,
}

impl Default for AliasLimits {
    fn default() -> Self {
        Self {
            max_total_replayed_events: 1_000_000,
            max_replay_stack_depth: 64,
            max_alias_expansions_per_anchor: usize::MAX,
        }
    }
}

#[pymethods]
impl AliasLimits {
    #[new]
    #[pyo3(signature = (
        max_total_replayed_events = 1_000_000,
        max_replay_stack_depth = 64,
        max_alias_expansions_per_anchor = None
    ))]
    fn new(
        max_total_replayed_events: isize,
        max_replay_stack_depth: isize,
        max_alias_expansions_per_anchor: Option<isize>,
    ) -> PyResult<Self> {
        Ok(Self {
            max_total_replayed_events: validate_limit!(
                max_total_replayed_events,
                "max_total_replayed_events"
            )?,
            max_replay_stack_depth: validate_limit!(
                max_replay_stack_depth,
                "max_replay_stack_depth"
            )?,
            max_alias_expansions_per_anchor: match max_alias_expansions_per_anchor {
                Some(value) => validate_limit!(value, "max_alias_expansions_per_anchor")?,
                None => usize::MAX,
            },
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DuplicateKeyPolicy {
    Error,
    FirstWins,
    #[default]
    LastWins,
}

impl DuplicateKeyPolicy {
    pub fn from_str(policy: Option<&str>) -> PyResult<Self> {
        match policy {
            Some("error") => Ok(Self::Error),
            Some("first_wins") => Ok(Self::FirstWins),
            Some("last_wins") | None => Ok(Self::LastWins),
            Some(value) => Err(PyValueError::new_err(format!(
                "invalid duplicate_key_policy: {value:?}"
            ))),
        }
    }
}
