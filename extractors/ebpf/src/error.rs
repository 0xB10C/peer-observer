use shared::log::SetLoggerError;
use std::error;
use std::fmt;
use std::io::Error as IoError;
use std::num::ParseIntError;
use std::time::SystemTimeError;

#[derive(Debug)]
pub enum RuntimeError {
    LibBpf(libbpf_rs::Error),
    NoSuchBPFMap(String),
    NoSuchBPFProg(String),
    Io(IoError),
    IntParse(ParseIntError),
    SystemTime(SystemTimeError),
    SetLogger(SetLoggerError),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RuntimeError::LibBpf(e) => write!(f, "libbpf error: {}", e),
            RuntimeError::Io(e) => write!(f, "IO error {}", e),
            RuntimeError::IntParse(e) => write!(f, "int parse error {}", e),
            RuntimeError::SystemTime(e) => write!(f, "system time error {}", e),
            RuntimeError::SetLogger(e) => write!(f, "set logger error {}", e),
            RuntimeError::NoSuchBPFMap(map) => write!(f, "could not find the BPF map {}", map),
            RuntimeError::NoSuchBPFProg(prog) => {
                write!(f, "could not find the BPF program {}", prog)
            }
        }
    }
}

impl error::Error for RuntimeError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            RuntimeError::LibBpf(ref e) => Some(e),
            RuntimeError::Io(ref e) => Some(e),
            RuntimeError::IntParse(ref e) => Some(e),
            RuntimeError::SystemTime(ref e) => Some(e),
            RuntimeError::SetLogger(ref e) => Some(e),
            RuntimeError::NoSuchBPFMap(_) => None,
            RuntimeError::NoSuchBPFProg(_) => None,
        }
    }
}

impl From<libbpf_rs::Error> for RuntimeError {
    fn from(e: libbpf_rs::Error) -> Self {
        RuntimeError::LibBpf(e)
    }
}

impl From<IoError> for RuntimeError {
    fn from(e: IoError) -> Self {
        RuntimeError::Io(e)
    }
}

impl From<ParseIntError> for RuntimeError {
    fn from(e: ParseIntError) -> Self {
        RuntimeError::IntParse(e)
    }
}

impl From<SystemTimeError> for RuntimeError {
    fn from(e: SystemTimeError) -> Self {
        RuntimeError::SystemTime(e)
    }
}

impl From<SetLoggerError> for RuntimeError {
    fn from(e: SetLoggerError) -> Self {
        RuntimeError::SetLogger(e)
    }
}
