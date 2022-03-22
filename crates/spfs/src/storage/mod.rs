// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

mod blob;
mod layer;
mod manifest;
mod payload;
mod platform;
mod repository;
mod tag;

mod config;
pub mod fs;
pub mod prelude;
pub mod rpc;
pub mod tar;

pub use self::config::{FromConfig, FromUrl};
pub use blob::BlobStorage;
pub use layer::LayerStorage;
pub use manifest::{ManifestStorage, ManifestViewer};
pub use payload::PayloadStorage;
pub use platform::PlatformStorage;
pub use repository::Repository;
pub use tag::TagStorage;

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum RepositoryHandle {
    FS(fs::FSRepository),
    Tar(tar::TarRepository),
    Rpc(rpc::RpcRepository),
}

impl RepositoryHandle {
    pub fn to_repo(self) -> Box<dyn Repository> {
        match self {
            Self::FS(repo) => Box::new(repo),
            Self::Tar(repo) => Box::new(repo),
            Self::Rpc(repo) => Box::new(repo),
        }
    }
}

impl std::ops::Deref for RepositoryHandle {
    type Target = dyn Repository;

    fn deref(&self) -> &Self::Target {
        match self {
            RepositoryHandle::FS(repo) => repo,
            RepositoryHandle::Tar(repo) => repo,
            RepositoryHandle::Rpc(repo) => repo,
        }
    }
}

impl std::ops::DerefMut for RepositoryHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            RepositoryHandle::FS(repo) => repo,
            RepositoryHandle::Tar(repo) => repo,
            RepositoryHandle::Rpc(repo) => repo,
        }
    }
}

impl From<fs::FSRepository> for RepositoryHandle {
    fn from(repo: fs::FSRepository) -> Self {
        RepositoryHandle::FS(repo)
    }
}
impl From<tar::TarRepository> for RepositoryHandle {
    fn from(repo: tar::TarRepository) -> Self {
        RepositoryHandle::Tar(repo)
    }
}
impl From<rpc::RpcRepository> for RepositoryHandle {
    fn from(repo: rpc::RpcRepository) -> Self {
        RepositoryHandle::Rpc(repo)
    }
}

/// Open the repository at the given url address
#[deprecated(
    since = "0.32.0",
    note = "instead, parse the address into a config and then open it: spfs::config::RemoteConfig::from_str(address).await?.open().await?"
)]
pub async fn open_repository<S: AsRef<str>>(address: S) -> crate::Result<RepositoryHandle> {
    crate::config::RemoteConfig::from_str(address)
        .await?
        .open()
        .await
}
