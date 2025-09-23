use shared::async_nats;
use shared::async_nats::ConnectErrorKind;
use shared::log::SetLoggerError;
use shared::prost::DecodeError;
use std::error;
use std::fmt;

#[derive(Debug)]
pub enum RuntimeError {
    SetLogger(SetLoggerError),
    ProtobufDecode(DecodeError),
    NatsSubscribe(async_nats::client::SubscribeError),
    NatsConnect(shared::async_nats::error::Error<ConnectErrorKind>),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RuntimeError::SetLogger(e) => write!(f, "set logger error {}", e),
            RuntimeError::ProtobufDecode(e) => write!(f, "protobuf decode error {}", e),
            RuntimeError::NatsSubscribe(e) => write!(f, "NATS subscribe error {}", e),
            RuntimeError::NatsConnect(e) => write!(f, "NATS connection error {}", e),
        }
    }
}

impl error::Error for RuntimeError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            RuntimeError::SetLogger(ref e) => Some(e),
            RuntimeError::ProtobufDecode(ref e) => Some(e),
            RuntimeError::NatsSubscribe(ref e) => Some(e),
            RuntimeError::NatsConnect(ref e) => Some(e),
        }
    }
}

impl From<SetLoggerError> for RuntimeError {
    fn from(e: SetLoggerError) -> Self {
        RuntimeError::SetLogger(e)
    }
}

impl From<DecodeError> for RuntimeError {
    fn from(e: DecodeError) -> Self {
        RuntimeError::ProtobufDecode(e)
    }
}

impl From<async_nats::client::SubscribeError> for RuntimeError {
    fn from(e: async_nats::client::SubscribeError) -> Self {
        RuntimeError::NatsSubscribe(e)
    }
}

impl From<shared::async_nats::error::Error<ConnectErrorKind>> for RuntimeError {
    fn from(e: shared::async_nats::error::Error<ConnectErrorKind>) -> Self {
        RuntimeError::NatsConnect(e)
    }
}
