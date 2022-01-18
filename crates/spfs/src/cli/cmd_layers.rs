// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use structopt::StructOpt;
use tokio_stream::StreamExt;

#[derive(Debug, StructOpt)]
pub struct CmdLayers {
    #[structopt(
        long = "remote",
        short = "r",
        about = "Show layers from remote repository instead of the local one"
    )]
    remote: Option<String>,
}

impl CmdLayers {
    pub async fn run(&mut self, config: &spfs::Config) -> spfs::Result<i32> {
        let repo = match &self.remote {
            Some(remote) => config.get_remote(remote).await?,
            None => config.get_repository()?.into(),
        };
        let mut layers = repo.iter_layers();
        while let Some(layer) = layers.next().await {
            let (digest, _) = layer?;
            println!(
                "{}",
                spfs::io::format_digest(&digest.to_string(), Some(&repo)).await?
            );
        }
        Ok(0)
    }
}
