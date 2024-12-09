use std::sync::Arc;

use api_types::ExecutionApiV2;
use async_trait::async_trait;


#[async_trait]
pub trait IServer : Send + Sync {
    async fn start(&self, addr: &str) -> tokio::io::Result<()>;

    fn execution_client(&self) -> Arc<dyn ExecutionApiV2>;
}