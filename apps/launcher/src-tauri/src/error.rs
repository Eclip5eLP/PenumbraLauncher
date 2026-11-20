//! Errors returned to the UI as `{ code, message }`.

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("unknown item: {0}")]
    UnknownItem(String),
    #[error("unknown job: {0}")]
    UnknownJob(u64),
    #[error("{0}")]
    InvalidState(String),
    #[error("invalid settings: {0}")]
    Settings(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
}

impl AppError {
    fn code(&self) -> &'static str {
        match self {
            AppError::UnknownItem(_) => "unknown_item",
            AppError::UnknownJob(_) => "unknown_job",
            AppError::InvalidState(_) => "invalid_state",
            AppError::Settings(_) => "invalid_settings",
            AppError::Io(_) => "io",
            AppError::Tauri(_) => "internal",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("AppError", 2)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
