// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk
use std::sync::{Arc, RwLock};

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use storage::{FromConfig, FromUrl};
use tokio_stream::StreamExt;

use crate::{runtime, storage, Result};
use std::path::PathBuf;

#[cfg(test)]
#[path = "./config_test.rs"]
mod config_test;

static DEFAULT_STORAGE_ROOT: &str = "~/.local/share/spfs";
static FALLBACK_STORAGE_ROOT: &str = "/tmp/spfs";

lazy_static! {
    static ref CONFIG: RwLock<Option<Arc<Config>>> = RwLock::new(None);
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct User {
    pub name: String,
    pub domain: String,
}

impl Default for User {
    fn default() -> Self {
        Self {
            name: whoami::username(),
            domain: whoami::hostname(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Filesystem {
    pub max_layers: usize,
}

impl Default for Filesystem {
    fn default() -> Self {
        Self { max_layers: 40 }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Storage {
    pub root: PathBuf,
}

impl Default for Storage {
    fn default() -> Self {
        Self {
            root: expanduser::expanduser(DEFAULT_STORAGE_ROOT)
                .unwrap_or_else(|_| PathBuf::from(FALLBACK_STORAGE_ROOT)),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum Remote {
    Address(RemoteAddress),
    Config(RemoteConfig),
}

impl<'de> serde::de::Deserialize<'de> for Remote {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde_json::{Map, Value};
        let data = Map::deserialize(deserializer)?;
        if data.contains_key(&String::from("scheme")) {
            Ok(Self::Config(
                RemoteConfig::deserialize(Value::Object(data)).map_err(serde::de::Error::custom)?,
            ))
        } else {
            Ok(Self::Address(
                RemoteAddress::deserialize(Value::Object(data))
                    .map_err(serde::de::Error::custom)?,
            ))
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RemoteAddress {
    pub address: url::Url,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "scheme", rename_all = "lowercase")]
pub enum RemoteConfig {
    Fs(storage::fs::Config),
    Grpc(storage::rpc::Config),
    Tar(storage::tar::Config),
    Proxy(storage::proxy::Config),
}

impl RemoteConfig {
    /// Parse a complete repository connection config from a url
    pub async fn from_address(url: url::Url) -> Result<Self> {
        Ok(match url.scheme() {
            "tar" => Self::Tar(storage::tar::Config::from_url(&url).await?),
            "file" | "" => Self::Fs(storage::fs::Config::from_url(&url).await?),
            "http2" | "grpc" => Self::Grpc(storage::rpc::Config::from_url(&url).await?),
            "proxy" => Self::Proxy(storage::proxy::Config::from_url(&url).await?),
            scheme => return Err(format!("Unsupported repository scheme: '{scheme}'").into()),
        })
    }

    /// Parse a complete repository connection from an address string
    pub async fn from_str<S: AsRef<str>>(address: S) -> Result<Self> {
        let url = match url::Url::parse(address.as_ref()) {
            Ok(url) => url,
            Err(err) => return Err(format!("invalid repository url: {:?}", err).into()),
        };

        Self::from_address(url).await
    }

    /// Open a handle to a repository using this configuration
    pub async fn open(&self) -> Result<storage::RepositoryHandle> {
        Ok(match self.clone() {
            Self::Fs(config) => storage::fs::FSRepository::from_config(config).await?.into(),
            Self::Tar(config) => storage::tar::TarRepository::from_config(config)
                .await?
                .into(),
            Self::Grpc(config) => storage::rpc::RpcRepository::from_config(config)
                .await?
                .into(),
            Self::Proxy(config) => storage::proxy::ProxyRepository::from_config(config)
                .await?
                .into(),
        })
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub user: User,
    pub storage: Storage,
    pub filesystem: Filesystem,
    pub remote: std::collections::HashMap<String, Remote>,
}

impl Config {
    /// Get the current loaded config, loading it if needed
    pub fn current() -> Result<Arc<Self>> {
        get_config()
    }

    /// Load the config from disk, even if it's already been loaded before
    pub fn load() -> Result<Self> {
        load_config()
    }

    /// Make this config the current global one
    pub fn make_current(self) -> Result<Arc<Self>> {
        let mut lock = CONFIG.write().map_err(|err| {
            crate::Error::String(format!(
                "Cannot load config, lock has been poisoned: {:?}",
                err
            ))
        })?;
        Ok(lock.insert(Arc::new(self)).clone())
    }

    /// Load the given string as a config
    #[deprecated(
        since = "0.32.0",
        note = "use the appropriate serde crate to deserialize a Config directly"
    )]
    pub fn load_string<S: AsRef<str>>(conf: S) -> Result<Self> {
        let mut s = config::Config::default();
        #[allow(deprecated)]
        s.merge(config::File::from_str(
            conf.as_ref(),
            config::FileFormat::Ini,
        ))?;
        Ok(Config::deserialize(s)?)
    }

    /// List the names of all configured remote repositories.
    pub fn list_remote_names(&self) -> Vec<String> {
        self.remote.keys().map(|s| s.to_string()).collect()
    }

    /// Open a connection to all remote repositories
    pub async fn list_remotes(&self) -> Result<Vec<storage::RepositoryHandle>> {
        let futures: futures::stream::FuturesUnordered<_> =
            self.remote.keys().map(|s| self.get_remote(s)).collect();
        futures.collect().await
    }

    /// Get the local repository instance as configured.
    pub async fn get_repository(&self) -> Result<storage::fs::FSRepository> {
        storage::fs::FSRepository::create(&self.storage.root).await
    }

    /// Get the local runtime storage, as configured.
    pub async fn get_runtime_storage(&self) -> Result<runtime::Storage> {
        Ok(runtime::Storage::new(storage::RepositoryHandle::from(
            self.get_repository().await?,
        )))
    }

    /// Get a remote repostory by name or address.
    pub async fn get_remote<S: AsRef<str>>(
        &self,
        name_or_address: S,
    ) -> Result<storage::RepositoryHandle> {
        let res = match self.remote.get(name_or_address.as_ref()) {
            Some(Remote::Address(remote)) => {
                let config = RemoteConfig::from_address(remote.address.clone()).await?;
                tracing::debug!(?config, "opening repository");
                config.open().await
            }
            Some(Remote::Config(config)) => {
                tracing::debug!(?config, "opening repository");
                config.open().await
            }
            None => {
                let addr = match url::Url::parse(name_or_address.as_ref()) {
                    Ok(addr) => addr,
                    Err(_) => {
                        url::Url::parse(format!("file:{}", name_or_address.as_ref()).as_str())?
                    }
                };
                let config = RemoteConfig::from_address(addr).await?;
                tracing::debug!(?config, "opening repository");
                config.open().await
            }
        };
        match res {
            Ok(repo) => Ok(repo),
            err @ Err(crate::Error::FailedToOpenRepository { .. }) => err,
            Err(err) => Err(crate::Error::FailedToOpenRepository {
                reason: String::from("error"),
                source: Box::new(err),
            }),
        }
    }
}

/// Get the current spfs config, fetching it from disk if needed.
pub fn get_config() -> Result<Arc<Config>> {
    let lock = CONFIG.read().map_err(|err| {
        crate::Error::String(format!(
            "Cannot load config, lock has been poisoned: {:?}",
            err
        ))
    })?;
    if let Some(config) = &*lock {
        return Ok(config.clone());
    }
    drop(lock);

    // there is still a possible race condition here
    // where someone loads the config between the first check and
    // aquiring this lock, but the redundant work is still
    // less than not having a cache at all
    let config = load_config()?;
    config.make_current()
}

/// Load the spfs configuration from disk, even if it's already been loaded.
///
/// This includes the default, user and system configurations, if they exist.
pub fn load_config() -> Result<Config> {
    use config::{Config as RawConfig, Environment, File, FileFormat::Ini};

    let user_config = expanduser::expanduser("~/.config/spfs/spfs")?;

    let mut builder = RawConfig::builder()
        // for backwards compatibility we also support .conf as an ini extension
        .add_source(File::new("/etc/spfs.conf", Ini).required(false))
        // the system config can also be in any support format: toml, yaml, json, ini, etc
        .add_source(File::with_name("/etc/spfs").required(false))
        // for backwards compatibility we also support .conf as an ini extension
        .add_source(File::new(&format!("{}.conf", user_config.display()), Ini).required(false))
        // the user config can also be in any support format: toml, yaml, json, ini, etc
        .add_source(File::with_name(&format!("{}", user_config.display())).required(false))
        .add_source(Environment::with_prefix("SPFS").separator("_"));

    let base = builder.build_cloned()?;

    // unfortunately, we need to load the config twice, because
    // the initial one may load values from the environment and
    // place them into the wrong structure if the target field
    // name also includes an underscore
    if let Ok(v) = base.get_string("filesystem.max.layers") {
        builder = builder.set_override("filesystem.max_layers", v)?;
    }

    let config = builder.build()?;

    Ok(Config::deserialize(config)?)
}

/// Open the repository at the given url address
pub async fn open_repository<S: AsRef<str>>(
    address: S,
) -> crate::Result<storage::RepositoryHandle> {
    match RemoteConfig::from_str(address).await?.open().await {
        Ok(repo) => Ok(repo),
        err @ Err(crate::Error::FailedToOpenRepository { .. }) => err,
        Err(err) => Err(crate::Error::FailedToOpenRepository {
            reason: String::from("error"),
            source: Box::new(err),
        }),
    }
}
