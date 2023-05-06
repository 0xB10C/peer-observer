use std::fmt;

// structs are generated via the fn_timings.proto file
include!(concat!(env!("OUT_DIR"), "/fn_timings.rs"));

impl fmt::Display for function_timings::Function {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            function_timings::Function::Sendmsgs(x) => write!(f, "FunctionTimings: {}", x),
            function_timings::Function::Processmsgs(x) => write!(f, "FunctionTimings: {}", x),
            function_timings::Function::Atmp(x) => write!(f, "FunctionTimings: {}", x),
        }
    }
}

impl From<Vec<u64>> for NetProcessingSendMessages {
    fn from(vec: Vec<u64>) -> Self {
        NetProcessingSendMessages { times: vec }
    }
}

impl fmt::Display for NetProcessingSendMessages {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "net_processing::SendMessages({} timings)",
            self.times.len(),
        )
    }
}

impl From<Vec<u64>> for NetProcessingProcessMessages {
    fn from(vec: Vec<u64>) -> Self {
        NetProcessingProcessMessages { times: vec }
    }
}

impl fmt::Display for NetProcessingProcessMessages {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "net_processing::ProcessMessages({} timings)",
            self.times.len(),
        )
    }
}

impl From<Vec<u64>> for ValidationAtmp {
    fn from(vec: Vec<u64>) -> Self {
        ValidationAtmp { times: vec }
    }
}

impl fmt::Display for ValidationAtmp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "validation::AcceptToMemoryPool({} timings)",
            self.times.len(),
        )
    }
}
