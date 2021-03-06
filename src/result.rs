use std::string::FromUtf8Error;
use tokio::task::JoinError;

#[derive(Debug)]
pub enum DtsupErrorType {
  GraphMissingError,
  ParseFileError,
  UTF8Error,

  JoinError,
  IOError,
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

impl From<std::io::Error> for Error {
  fn from(err: std::io::Error) -> Self {
    Error::new_with_reason(DtsupErrorType::IOError, &err.to_string())
  }
}

impl From<FromUtf8Error> for Error {
  fn from(err: FromUtf8Error) -> Self {
    Error::new_with_reason(DtsupErrorType::UTF8Error, &err.to_string())
  }
}

// impl From<anyhow::Error> for Error {
//   fn from(err: anyhow::Error) -> Self {
//     // Error::new()
//   }
// }
