/// Errors produced by the core pipeline.
#[derive(Clone, Debug)]
pub enum CoreError {
    Io(String),
    Unsupported(String),
    Internal(String),
}

impl CoreError {
    pub fn message(&self) -> &str {
        match self {
            CoreError::Io(m) | CoreError::Unsupported(m) | CoreError::Internal(m) => m.as_str(),
        }
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;

pub fn ok<T>(value: T) -> Result<T> {
    Ok(value)
}

pub fn err<T>(message: &str) -> Result<T> {
    Err(CoreError::Unsupported(String::from(message)))
}
