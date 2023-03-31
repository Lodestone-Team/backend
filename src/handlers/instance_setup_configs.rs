use crate::error::Error;
use crate::implementations::generic;
use crate::implementations::minecraft;
use crate::minecraft::FlavourKind;
use crate::traits::t_configurable::manifest::SectionManifestValue;
use crate::traits::t_configurable::manifest::SetupManifest;
use crate::traits::t_configurable::GameType;
use crate::AppState;
use axum::extract::Path;
use axum::routing::get;
use axum::routing::put;
use axum::Json;
use axum::Router;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[allow(clippy::enum_variant_names)]
#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub enum HandlerGameType {
    MinecraftJavaVanilla,
    MinecraftFabric,
    MinecraftForge,
    MinecraftPaper,
}

impl From<HandlerGameType> for GameType {
    fn from(value: HandlerGameType) -> Self {
        match value {
            HandlerGameType::MinecraftJavaVanilla => Self::MinecraftJava,
            HandlerGameType::MinecraftFabric => Self::MinecraftJava,
            HandlerGameType::MinecraftForge => Self::MinecraftJava,
            HandlerGameType::MinecraftPaper => Self::MinecraftJava,
        }
    }
}

impl From<HandlerGameType> for FlavourKind {
    fn from(value: HandlerGameType) -> Self {
        match value {
            HandlerGameType::MinecraftJavaVanilla => Self::Vanilla,
            HandlerGameType::MinecraftFabric => Self::Fabric,
            HandlerGameType::MinecraftForge => Self::Forge,
            HandlerGameType::MinecraftPaper => Self::Paper,
        }
    }
}

pub async fn get_available_games() -> Json<Vec<HandlerGameType>> {
    Json(vec![
        HandlerGameType::MinecraftJavaVanilla,
        HandlerGameType::MinecraftFabric,
        HandlerGameType::MinecraftForge,
        HandlerGameType::MinecraftPaper,
    ])
}

pub async fn get_setup_manifest(
    Path(game_type): Path<HandlerGameType>,
) -> Result<Json<SetupManifest>, Error> {
    minecraft::MinecraftInstance::setup_manifest(&game_type.into())
        .await
        .map(Json)
}

#[derive(Deserialize)]
pub struct GenericSetupManifestBody {
    pub url: String,
}

pub async fn get_generic_setup_manifest(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(body): Json<GenericSetupManifestBody>,
) -> Result<Json<SetupManifest>, Error> {
    generic::GenericInstance::setup_manifest(&body.url, state.macro_executor)
        .await
        .map(Json)
}

pub async fn validate_section(
    Path((game_type, section_id)): Path<(HandlerGameType, String)>,
    Json(section): Json<SectionManifestValue>,
) -> Result<Json<()>, Error> {
    Ok(Json(
        minecraft::MinecraftInstance::validate_section(&game_type.into(), &section_id, &section)
            .await?,
    ))
}

pub fn get_instance_setup_config_routes(appstate: AppState) -> Router {
    Router::new()
        .route("/games", get(get_available_games))
        .route("/setup_manifest/:game_type", get(get_setup_manifest))
        .route(
            "/generic_setup_manifest/generic",
            put(get_generic_setup_manifest),
        )
        .route(
            "/setup_manifest/:game_type/:section_id",
            put(validate_section),
        )
        .with_state(appstate)
}
