use anyhow::Error;
use reqwest::Client;
use serde::de::DeserializeOwned;

static USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[derive(Clone, Debug, Default)]
pub struct CliContext {
    // API clients
    api: Client,
}

impl CliContext {
    pub async fn init() -> Result<CliContext, Error> {
        Ok(CliContext {
            api: Client::builder().user_agent(USER_AGENT).build()?,
        })
    }

    #[allow(dead_code)]
    pub async fn api_get<T>(&self, url: &str) -> Result<T, Error>
    where
        T: core::fmt::Debug + DeserializeOwned,
    {
        let response = self.api.get(url).send().await?;
        response.json().await.map_err(Error::from)
    }
}
