// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk
use std::path::PathBuf;

use clap::Args;
use spfs::Error;

/// Store an arbitrary blob of data in spfs
#[derive(Debug, Args)]
#[clap(visible_aliases = &["write-file"])]
pub struct CmdWrite {
    /// A human-readable tag for the generated object
    ///
    /// Can be provided more than once.
    #[clap(long = "tag", short)]
    tags: Vec<String>,

    /// Write to a remote repository instead of the local one
    #[clap(long, short)]
    remote: Option<String>,

    /// Store the contents of this file instead of reading from stdin
    #[clap(long, short)]
    file: Option<PathBuf>,
}

impl CmdWrite {
    pub async fn run(&mut self, config: &spfs::Config) -> spfs::Result<i32> {
        let repo = spfs::config::open_repository_from_string(config, self.remote.as_ref()).await?;

        let reader: std::pin::Pin<Box<dyn tokio::io::AsyncBufRead + Sync + Send>> = match &self.file
        {
            Some(file) => Box::pin(tokio::io::BufReader::new(
                tokio::fs::File::open(&file)
                    .await
                    .map_err(|err| Error::RuntimeWriteError(file.clone(), err))?,
            )),
            None => Box::pin(tokio::io::BufReader::new(tokio::io::stdin())),
        };

        // TODO: get permissions from file

        let digest = repo.commit_blob(reader, None).await?;

        tracing::info!(?digest, "created");
        for tag in self.tags.iter() {
            let tag_spec = match spfs::tracking::TagSpec::parse(tag) {
                Ok(tag_spec) => tag_spec,
                Err(err) => {
                    tracing::warn!("cannot set invalid tag '{tag}': {err:?}");
                    continue;
                }
            };
            repo.push_tag(&tag_spec, &digest).await?;
            tracing::info!(?tag, "created");
        }

        Ok(0)
    }
}
