// Copyright (c) Contributors to the SPK project.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/spkenv/spk

use clap::Args;
use miette::Result;
use spfs_cli_common as cli;

/// Pull one or more objects to the local repository
#[derive(Debug, Args)]
pub struct CmdPull {
    #[clap(flatten)]
    sync: cli::Sync,

    #[clap(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// The name or address of the remote server to pull from
    ///
    /// Defaults to searching all configured remotes
    #[clap(long, short)]
    remote: Option<String>,

    /// The reference(s) to pull/localize
    ///
    /// These can be individual tags or digests, or they may also
    /// be a collection of items joined by a '+'
    #[clap(value_name = "REF", required = true)]
    refs: Vec<spfs::tracking::EnvSpec>,
}

impl CmdPull {
    pub async fn run(&mut self, config: &spfs::Config) -> Result<i32> {
        let (repo, remote) = tokio::try_join!(
            config.get_local_repository_handle(),
            spfs::config::open_repository_from_string(config, self.remote.as_ref())
        )?;

        let env_spec = self.refs.iter().cloned().collect();
        let summary = self
            .sync
            .get_syncer(&remote, &repo)
            .sync_env(env_spec)
            .await?
            .summary();

        tracing::info!("{}", spfs::io::format_sync_summary(&summary));

        Ok(0)
    }
}
