use async_trait::async_trait;

use crate::{error::Error, traits::t_resource::TResourceManagement};

use super::MinecraftBedrockInstance;

#[async_trait]
impl TResourceManagement for MinecraftBedrockInstance {
    async fn list(&self) -> Vec<serde_json::Value> {
        todo!()
    }

    async fn load(&mut self, _resource: &str) -> Result<(), Error> {
        todo!()
    }

    async fn unload(&mut self, _resource: &str) -> Result<(), Error> {
        todo!()
    }

    async fn delete(&mut self, _resource: &str) -> Result<(), Error> {
        todo!()
    }
}