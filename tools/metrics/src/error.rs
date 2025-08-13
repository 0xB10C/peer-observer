use shared::log::SetLoggerError;
use shared::prost::DecodeError;
use std::error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum RuntimeError {
    SetLogger(SetLoggerError),
    Io(io::Error),
    ProtobufDecode(DecodeError),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RuntimeError::SetLogger(e) => write!(f, "set logger error {}", e),
            RuntimeError::Io(e) => write!(f, "IO error {}", e),
            RuntimeError::ProtobufDecode(e) => write!(f, "protobuf decode error {}", e),
        }
    }
}

impl error::Error for RuntimeError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            RuntimeError::SetLogger(ref e) => Some(e),
            RuntimeError::Io(ref e) => Some(e),
            RuntimeError::ProtobufDecode(ref e) => Some(e),
        }
    }
}

impl From<SetLoggerError> for RuntimeError {
    fn from(e: SetLoggerError) -> Self {
        RuntimeError::SetLogger(e)
    }
}

impl From<io::Error> for RuntimeError {
    fn from(e: io::Error) -> Self {
        RuntimeError::Io(e)
    }
}

impl From<DecodeError> for RuntimeError {
    fn from(e: DecodeError) -> Self {
        RuntimeError::ProtobufDecode(e)
    }
}
