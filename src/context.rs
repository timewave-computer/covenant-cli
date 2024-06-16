use anyhow::Error;
use reqwest::Client;
use serde::de::DeserializeOwned;

static USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

pub struct Context {
    // API clients
    api: Client,
}

impl Context {
    pub async fn init() -> Result<Context, Error> {
        Ok(Context {
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
