use tokio::task::JoinError;

#[derive(Debug)]
pub enum DtsupErrorType {
  ParseFileError,

  JoinError,
}

#[derive(Debug)]
pub struct Error {
  err_type: DtsupErrorType,
  reason: Option<String>,
}

impl Error {
  pub fn new(err_type: DtsupErrorType) -> Self {
    Self {
      err_type,
      reason: None,
    }
  }

  pub fn new_with_reason(err_type: DtsupErrorType, reason: &str) -> Self {
    Self {
      err_type,
      reason: Some(reason.to_owned()),
    }
  }
}

impl From<JoinError> for Error {
  fn from(_: JoinError) -> Self {
    Error::new(DtsupErrorType::JoinError)
  }
}

// impl From<anyhow::Error> for Error {
//   fn from(err: anyhow::Error) -> Self {
//     // Error::new()
//   }
// }
