use serde::{Deserialize, Serialize};

/// Requests sent from client to daemon over the Unix socket
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    Stop,
    Status,
    ParamList,
    ParamGet { name: String },
    ParamSet { name: String, value: String },
    LogList,
    LogStart { variables: Vec<String>, rate_hz: u64 },
    LogStop,
}

/// Responses sent from daemon to client over the Unix socket
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    Ok { message: String },
    Error { message: String },
    Status {
        connected: bool,
        uri: String,
        firmware_version: Option<String>,
    },
    ParamList { params: Vec<ParamInfo> },
    ParamValue { name: String, value: String },
    LogList { variables: Vec<LogVarInfo> },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParamInfo {
    pub name: String,
    pub value_type: String,
    pub access: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogVarInfo {
    pub name: String,
    pub value_type: String,
}
