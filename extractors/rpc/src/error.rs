use shared::async_nats;
use shared::corepc_client::client_sync::Error as RPCError;
use std::error;
use std::fmt;
use std::time::SystemTimeError;

#[derive(Debug)]
pub enum FetchOrPublishError {
    Rpc(RPCError),
    SystemTime(SystemTimeError),
    NatsPublish(async_nats::error::Error<async_nats::client::PublishErrorKind>),
}

impl fmt::Display for FetchOrPublishError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FetchOrPublishError::Rpc(e) => write!(f, "RPC error: {}", e),
            FetchOrPublishError::SystemTime(e) => write!(f, "system time error {}", e),
            FetchOrPublishError::NatsPublish(e) => write!(f, "NATS publish error {}", e),
        }
    }
}

impl error::Error for FetchOrPublishError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            FetchOrPublishError::Rpc(ref e) => Some(e),
            FetchOrPublishError::SystemTime(ref e) => Some(e),
            FetchOrPublishError::NatsPublish(ref e) => Some(e),
        }
    }
}

impl From<RPCError> for FetchOrPublishError {
    fn from(e: RPCError) -> Self {
        FetchOrPublishError::Rpc(e)
    }
}

impl From<SystemTimeError> for FetchOrPublishError {
    fn from(e: SystemTimeError) -> Self {
        FetchOrPublishError::SystemTime(e)
    }
}

impl From<async_nats::error::Error<async_nats::client::PublishErrorKind>> for FetchOrPublishError {
    fn from(e: async_nats::error::Error<async_nats::client::PublishErrorKind>) -> Self {
        FetchOrPublishError::NatsPublish(e)
    }
}
