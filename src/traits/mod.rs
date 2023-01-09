use async_trait::async_trait;
use axum::response::IntoResponse;

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;

use self::t_manifest::TManifest;
use self::t_server::State;
use self::{
    t_configurable::TConfigurable, t_macro::TMacro, t_player::TPlayerManagement,
    t_resource::TResourceManagement, t_server::TServer,
};

pub mod t_configurable;
pub mod t_macro;
pub mod t_manifest;
pub mod t_player;
pub mod t_resource;
pub mod t_server;

#[derive(Debug, Serialize, Clone, TS)]
#[ts(export)]
pub enum ErrorInner {
    // IO errors:
    FailedToReadFileOrDir,
    FailedToWriteFileOrDir,
    FailedToCreateFileOrDir,
    FailedToRemoveFileOrDir,
    FileOrDirNotFound,
    FiledOrDirAlreadyExists,
    IOError,

    // Stdin/stdout errors:
    FailedToWriteStdin,
    FailedToReadStdout,
    StdinNotOpen,
    StdoutNotOpen,
    RconNotOpen,
    RconError,
    FailedToAcquireLock,

    // Network errors:
    FailedToUpload,
    FailedToDownload,

    // Instance operation errors
    InvalidInstanceState,
    InstanceNotFound,
    PortInUse,

    // Config file errors:
    MalformedFile,
    FieldNotFound,
    ValueNotFound,
    TypeMismatch,

    // version string errors:
    MalformedVersionString,
    VersionNotFound,

    // Macro errors:
    FailedToRun,
    MacroNotFound,

    // Process errors:
    FailedToExecute,
    FailedToAcquireStdin,
    FailedToAcquireStdout,
    FailedToAcquireStderr,

    // API changed
    APIChanged,

    // Unsupported Op
    UnsupportedOperation,

    // Malformed request
    MalformedRequest,

    // User errors:
    UserNotFound,
    UsernameAlreadyExists,
    Unauthorized,
    PermissionDenied,

    // DB errors:
    DBInitError,
    DBWriteError,
    DBFetchError,
    DBPoolError,

    // Gateway (port forwarding) error
    GatewayError,

    // Generic error
    NotFound,
    InternalError,
}
#[derive(Debug, Serialize, Clone, TS)]
#[serde(rename = "ClientError")]
#[ts(export)]
pub struct Error {
    pub inner: ErrorInner,
    pub detail: String,
}

// implement std error trait
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error: {}", self.detail)
    }
}

impl std::error::Error for Error {}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self.inner {
            ErrorInner::MalformedRequest => (StatusCode::BAD_REQUEST, json!(self).to_string()),
            ErrorInner::PermissionDenied => (StatusCode::FORBIDDEN, json!(self).to_string()),
            ErrorInner::Unauthorized => (StatusCode::UNAUTHORIZED, json!(self).to_string()),
            ErrorInner::FileOrDirNotFound => (StatusCode::NOT_FOUND, json!(self).to_string()),
            ErrorInner::NotFound => (StatusCode::NOT_FOUND, json!(self).to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, json!(self).to_string()),
        };
        (status, error_message).into_response()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub struct InstanceInfo {
    pub uuid: InstanceUuid,
    pub name: String,
    pub flavour: String,
    pub game_type: String,
    pub cmd_args: Vec<String>,
    pub description: String,
    pub port: u32,
    pub min_ram: Option<u32>,
    pub max_ram: Option<u32>,
    pub creation_time: i64,
    pub path: String,
    pub auto_start: bool,
    pub restart_on_crash: bool,
    pub backup_period: Option<u32>,
    pub state: State,
    pub player_count: Option<u32>,
    pub max_player_count: Option<u32>,
}
use crate::minecraft::MinecraftInstance;
use crate::prelude::GameInstance;
use crate::types::InstanceUuid;
#[async_trait]
#[enum_dispatch::enum_dispatch]
pub trait TInstance:
    TConfigurable
    + TMacro
    + TPlayerManagement
    + TResourceManagement
    + TServer
    + TManifest
    + Sync
    + Send
    + Clone
{
    async fn get_instance_info(&self) -> InstanceInfo {
        InstanceInfo {
            uuid: self.uuid().await,
            name: self.name().await,
            flavour: self.flavour().await,
            game_type: self.game_type().await,
            cmd_args: self.cmd_args().await,
            description: self.description().await,
            port: self.port().await,
            min_ram: self.min_ram().await.ok(),
            max_ram: self.max_ram().await.ok(),
            creation_time: self.creation_time().await,
            path: self.path().await.display().to_string(),
            auto_start: self.auto_start().await,
            restart_on_crash: self.restart_on_crash().await,
            backup_period: self.backup_period().await.unwrap_or(None),
            state: self.state().await,
            player_count: self.get_player_count().await.ok(),
            max_player_count: self.get_max_player_count().await.ok(),
        }
    }
}
