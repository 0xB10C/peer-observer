use shared::async_nats::ConnectErrorKind;
use shared::log::SetLoggerError;
use std::error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum RuntimeError {
    SetLogger(SetLoggerError),
    Io(io::Error),
    NatsConnect(shared::async_nats::error::Error<ConnectErrorKind>),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RuntimeError::SetLogger(e) => write!(f, "set logger error {}", e),
            RuntimeError::Io(e) => write!(f, "IO error {}", e),
            RuntimeError::NatsConnect(e) => write!(f, "NATS connection error {}", e),
        }
    }
}

impl error::Error for RuntimeError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            RuntimeError::SetLogger(ref e) => Some(e),
            RuntimeError::Io(ref e) => Some(e),
            RuntimeError::NatsConnect(ref e) => Some(e),
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

impl From<shared::async_nats::error::Error<ConnectErrorKind>> for RuntimeError {
    fn from(e: shared::async_nats::error::Error<ConnectErrorKind>) -> Self {
        RuntimeError::NatsConnect(e)
    }
}

#[derive(Debug)]
pub enum BitcoinMsgDecodeError {
    HeaderReadError(shared::tokio::io::Error),
    PayloadReadError(shared::tokio::io::Error),
    InvalidLengthBytes(std::array::TryFromSliceError),
    DecodeError(shared::bitcoin::consensus::encode::Error),
    MagicError([u8; 4], [u8; 4]),
}

impl fmt::Display for BitcoinMsgDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use BitcoinMsgDecodeError::*;
        match self {
            HeaderReadError(e) => write!(f, "Failed to read message header: {}", e),
            PayloadReadError(e) => write!(f, "Failed to read message payload: {}", e),
            InvalidLengthBytes(e) => write!(f, "Invalid length bytes in header: {}", e),
            DecodeError(e) => write!(f, "Consensus decode error: {}", e),
            MagicError(expected, got) => write!(
                f,
                "Network magic mismatch: got={}, expected={}",
                got.iter()
                    .fold(String::new(), |a, b| a + &format!("{:02x}", b)),
                expected
                    .iter()
                    .fold(String::new(), |a, b| a + &format!("{:02x}", b)),
            ),
        }
    }
}

impl std::error::Error for BitcoinMsgDecodeError {}

impl From<shared::bitcoin::consensus::encode::Error> for BitcoinMsgDecodeError {
    fn from(e: shared::bitcoin::consensus::encode::Error) -> Self {
        BitcoinMsgDecodeError::DecodeError(e)
    }
}

impl From<std::array::TryFromSliceError> for BitcoinMsgDecodeError {
    fn from(e: std::array::TryFromSliceError) -> Self {
        BitcoinMsgDecodeError::InvalidLengthBytes(e)
    }
}
