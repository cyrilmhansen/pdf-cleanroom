/// Safety checks and warnings for pdf-cleanroom.
///
/// This module enforces the project's conservative approach:
/// - No redaction by overlay rectangles
/// - No preview mode that could be confused with real redaction
/// - preserve mode explicitly blocked until destructive removal is real

use std::io;

/// Check if a potentially dangerous operation was requested.
/// For the MVP, this is primarily about `preserve` being invoked
/// without a real implementation.
pub fn check_preserve_not_implemented() -> Result<(), SafetyError> {
    Err(SafetyError::PreserveNotImplemented)
}

#[derive(Debug)]
pub enum SafetyError {
    PreserveNotImplemented,
    Io(io::Error),
}

impl std::fmt::Display for SafetyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PreserveNotImplemented => {
                write!(
                    f,
                    "preserve mode is not implemented. \
                     pdf-cleanroom will never redact by overlaying black rectangles. \
                     A real destructive removal must be implemented first. \
                     Use 'rebuild' for a clean PDF reconstruction."
                )
            }
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for SafetyError {}

impl From<io::Error> for SafetyError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
