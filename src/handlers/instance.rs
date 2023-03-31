use axum::routing::{delete, get, post};
use axum::Router;
use axum::{extract::Path, Json};
use axum_auth::AuthBearer;

use color_eyre::eyre::{eyre, Context};
use serde::Deserialize;

use crate::auth::user::UserAction;
use crate::error::{Error, ErrorKind};
use crate::events::{
    CausedBy, Event, EventInner, ProgressionEndValue, ProgressionEvent, ProgressionEventInner,
    ProgressionStartValue,
};

use crate::implementations::generic;
use crate::traits::t_configurable::GameType;

use minecraft::FlavourKind;

use crate::implementations::minecraft::MinecraftInstance;
use crate::prelude::PATH_TO_INSTANCES;
use crate::traits::t_configurable::manifest::ManifestValue;
use crate::traits::{t_configurable::TConfigurable, t_server::TServer, InstanceInfo, TInstance};

use crate::types::{DotLodestoneConfig, InstanceUuid, Snowflake};
use crate::{implementations::minecraft, traits::t_server::State, AppState};

use super::instance_setup_configs::HandlerGameType;

pub async fn get_instance_list(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<InstanceInfo>>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    let mut list_of_configs: Vec<InstanceInfo> = Vec::new();

    let instances = state.instances.lock().await;
    for instance in instances.values() {
        if requester.can_perform_action(&UserAction::ViewInstance(instance.uuid().await)) {
            list_of_configs.push(instance.get_instance_info().await);
        }
    }

    list_of_configs.sort_by(|a, b| a.creation_time.cmp(&b.creation_time));

    Ok(Json(list_of_configs))
}

pub async fn get_instance_info(
    Path(uuid): Path<InstanceUuid>,
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<InstanceInfo>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;

    let instances = state.instances.lock().await;

    let instance = instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;

    requester.try_action(&UserAction::ViewInstance(uuid.clone()))?;
    Ok(Json(instance.get_instance_info().await))
}

pub async fn create_minecraft_instance(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
    Path(game_type): Path<HandlerGameType>,
    Json(manifest_value): Json<ManifestValue>,
) -> Result<Json<InstanceUuid>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::CreateInstance)?;

    let mut instance_uuid = InstanceUuid::default();

    for uuid in state.instances.lock().await.keys() {
        if let Some(uuid) = uuid.as_ref().get(0..8) {
            if uuid == &instance_uuid.no_prefix()[0..8] {
                instance_uuid = InstanceUuid::default();
            }
        }
    }

    let instance_uuid = instance_uuid;

    let flavour = match game_type {
        HandlerGameType::MinecraftJavaVanilla => FlavourKind::Vanilla,
        HandlerGameType::MinecraftForge => FlavourKind::Forge,
        HandlerGameType::MinecraftFabric => FlavourKind::Fabric,
        HandlerGameType::MinecraftPaper => FlavourKind::Paper,
    };

    let setup_config = MinecraftInstance::construct_setup_config(manifest_value, flavour).await?;

    let setup_path = PATH_TO_INSTANCES.with(|path| {
        path.join(format!(
            "{}-{}",
            setup_config.name,
            &instance_uuid.no_prefix()[0..8]
        ))
    });

    tokio::fs::create_dir_all(&setup_path)
        .await
        .context("Failed to create instance directory")?;

    let dot_lodestone_config = DotLodestoneConfig::new(instance_uuid.clone(), game_type.into());

    // write dot lodestone config

    tokio::fs::write(
        setup_path.join(".lodestone_config"),
        serde_json::to_string_pretty(&dot_lodestone_config).unwrap(),
    )
    .await
    .context("Failed to write .lodestone_config file")?;

    tokio::task::spawn({
        let uuid = instance_uuid.clone();
        let instance_name = setup_config.name.clone();
        let event_broadcaster = state.event_broadcaster.clone();
        let port = setup_config.port;
        let flavour = setup_config.flavour.clone();
        let caused_by = CausedBy::User {
            user_id: requester.uid.clone(),
            user_name: requester.username.clone(),
        };
        async move {
            let progression_event_id = Snowflake::default();
            event_broadcaster.send(Event {
                event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                    event_id: progression_event_id,
                    progression_event_inner: ProgressionEventInner::ProgressionStart {
                        progression_name: format!("Setting up Minecraft server {}", instance_name),
                        producer_id: Some(uuid.clone()),
                        total: Some(10.0),
                        inner: Some(ProgressionStartValue::InstanceCreation {
                            instance_uuid: uuid.clone(),
                            instance_name: instance_name.clone(),
                            port,
                            flavour: flavour.to_string(),
                            game_type: "minecraft".to_string(),
                        }),
                    },
                }),
                details: "".to_string(),
                snowflake: Snowflake::default(),
                caused_by: caused_by.clone(),
            });
            let minecraft_instance = match minecraft::MinecraftInstance::new(
                setup_config.clone(),
                dot_lodestone_config,
                setup_path.clone(),
                progression_event_id,
                state.event_broadcaster.clone(),
                state.macro_executor.clone(),
            )
            .await
            {
                Ok(v) => {
                    event_broadcaster.send(Event {
                        event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                            event_id: progression_event_id,
                            progression_event_inner: ProgressionEventInner::ProgressionEnd {
                                success: true,
                                message: Some("Instance creation success".to_string()),
                                inner: Some(ProgressionEndValue::InstanceCreation(
                                    v.get_instance_info().await,
                                )),
                            },
                        }),
                        details: "".to_string(),
                        snowflake: Snowflake::default(),
                        caused_by: caused_by.clone(),
                    });
                    v
                }
                Err(e) => {
                    event_broadcaster.send(Event {
                        event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                            event_id: progression_event_id,
                            progression_event_inner: ProgressionEventInner::ProgressionEnd {
                                success: false,
                                message: Some(format!("Instance creation failed: {:?}", e)),
                                inner: None,
                            },
                        }),
                        details: "".to_string(),
                        snowflake: Snowflake::default(),
                        caused_by: caused_by.clone(),
                    });
                    crate::util::fs::remove_dir_all(setup_path)
                        .await
                        .context("Failed to remove directory after instance creation failed")
                        .unwrap();
                    return;
                }
            };
            let mut port_manager = state.port_manager.lock().await;
            port_manager.add_port(setup_config.port);
            state
                .instances
                .lock()
                .await
                .insert(uuid.clone(), minecraft_instance.into());
        }
    });
    Ok(Json(instance_uuid))
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenericSetupConfig {
    url: String,
    setup_value: ManifestValue,
}

pub async fn create_generic_instance(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
    Json(setup_config): Json<GenericSetupConfig>,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::CreateInstance)?;
    let mut instance_uuid = InstanceUuid::default();
    for uuid in state.instances.lock().await.keys() {
        if let Some(uuid) = uuid.as_ref().get(0..8) {
            if uuid == &instance_uuid.no_prefix()[0..8] {
                instance_uuid = InstanceUuid::default();
            }
        }
    }

    let instance_uuid = instance_uuid;

    let setup_path = PATH_TO_INSTANCES.with(|path| {
        path.join(format!(
            "{}-{}",
            "generic",
            &instance_uuid.no_prefix()[0..8]
        ))
    });

    tokio::fs::create_dir_all(&setup_path)
        .await
        .context("Failed to create instance directory")?;

    let dot_lodestone_config = DotLodestoneConfig::new(instance_uuid.clone(), GameType::Generic);

    // write dot lodestone config

    tokio::fs::write(
        setup_path.join(".lodestone_config"),
        serde_json::to_string_pretty(&dot_lodestone_config).unwrap(),
    )
    .await
    .context("Failed to write .lodestone_config file")?;

    let instance = generic::GenericInstance::new(
        setup_config.url,
        setup_path,
        dot_lodestone_config,
        setup_config.setup_value,
        state.event_broadcaster.clone(),
        state.macro_executor.clone(),
    )
    .await?;

    state
        .instances
        .lock()
        .await
        .insert(instance_uuid.clone(), instance.into());
    Ok(Json(()))
}

pub async fn delete_instance(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::DeleteInstance)?;
    let mut instances = state.instances.lock().await;
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    if let Some(instance) = instances.get(&uuid) {
        if !(instance.state().await == State::Stopped) {
            Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Instance must be stopped before deletion"),
            })
        } else {
            let progression_id = Snowflake::default();
            let event_broadcaster = state.event_broadcaster.clone();
            event_broadcaster.send(Event {
                event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                    event_id: progression_id,
                    progression_event_inner: ProgressionEventInner::ProgressionStart {
                        progression_name: format!("Deleting instance {}", instance.name().await),
                        producer_id: Some(uuid.clone()),
                        total: Some(10.0),
                        inner: None,
                    },
                }),
                details: "".to_string(),
                snowflake: Snowflake::default(),
                caused_by: caused_by.clone(),
            });
            tokio::fs::remove_file(instance.path().await.join(".lodestone_config"))
                .await
                .map_err(|e| {
                    event_broadcaster.send(Event {
                        event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                            event_id: Snowflake::default(),
                            progression_event_inner: ProgressionEventInner::ProgressionEnd {
                                success: false,
                                message: Some(
                                    "Failed to delete .lodestone_config. Instance not deleted"
                                        .to_string(),
                                ),
                                inner: None,
                            },
                        }),
                        details: "".to_string(),
                        snowflake: Snowflake::default(),
                        caused_by: caused_by.clone(),
                    });
                    Err::<(), std::io::Error>(e)
                        .context("Failed to delete .lodestone_config file. Instance not deleted")
                        .unwrap_err()
                })?;
            state
                .port_manager
                .lock()
                .await
                .deallocate(instance.port().await);
            let instance_path = instance.path().await;
            instances.remove(&uuid);
            drop(instances);
            let res = crate::util::fs::remove_dir_all(instance_path).await;

            if res.is_ok() {
                event_broadcaster.send(Event {
                    event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                        event_id: progression_id,
                        progression_event_inner: ProgressionEventInner::ProgressionEnd {
                            success: true,
                            message: Some("Deleted instance".to_string()),
                            inner: Some(ProgressionEndValue::InstanceDelete {
                                instance_uuid: uuid.clone(),
                            }),
                        },
                    }),
                    details: "".to_string(),
                    snowflake: Snowflake::default(),
                    caused_by: caused_by.clone(),
                });
            } else {
                event_broadcaster.send(Event {
                    event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                        event_id: progression_id,
                        progression_event_inner: ProgressionEventInner::ProgressionEnd {
                            success: false,
                            message: Some(
                                "Could not delete some or all of instance's files".to_string(),
                            ),
                            inner: None,
                        },
                    }),
                    details: "".to_string(),
                    snowflake: Snowflake::default(),
                    caused_by: caused_by.clone(),
                });
            }
            res.map(|_| Json(()))
        }
    } else {
        Err(Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })
    }
}

pub fn get_instance_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/list", get(get_instance_list))
        .route(
            "/instance/create/:game_type",
            post(create_minecraft_instance),
        )
        .route("/instance/create_generic", post(create_generic_instance))
        .route("/instance/:uuid", delete(delete_instance))
        .route("/instance/:uuid/info", get(get_instance_info))
        .with_state(state)
}
