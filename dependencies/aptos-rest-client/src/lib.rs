use error::RestError;
use gaptos::move_core_types::account_address::AccountAddress;
use serde::de::DeserializeOwned;
use url::Url;
use reqwest::Client as ReqwestClient;

#[derive(Clone, Debug)]
pub struct Client {
    inner: ReqwestClient,
    base_url: Url,
    version_path_base: String,
}

#[derive(Debug)]
pub struct Response<T> {
    inner: T,
}

impl<T> Response<T> {
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl Client {
    pub fn new(base_url: Url) -> Self {
        todo!()
    }

    pub async fn get_account_resource_bcs<T: DeserializeOwned>(
        &self,
        address: AccountAddress,
        resource_type: &str,
    ) -> Result<Response<T>, RestError> {
        todo!()
    }
}

pub mod error {
    use std::fmt::Display;
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum RestError {
        Err1,
    }

    impl Display for RestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "RestError")
        }
    }
}