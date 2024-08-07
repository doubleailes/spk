// Copyright (c) Contributors to the SPK project.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/spkenv/spk

use clap::Args;
use miette::Result;
use spk_cli_common::{CommandArgs, Run, VERSION};

/// Print the spk version information
#[derive(Args)]
pub struct Version {}

#[async_trait::async_trait]
impl Run for Version {
    type Output = i32;

    async fn run(&mut self) -> Result<Self::Output> {
        println!(" spk {VERSION}");
        println!("spfs {}", spfs::VERSION);
        Ok(0)
    }
}

impl CommandArgs for Version {
    fn get_positional_args(&self) -> Vec<String> {
        // There are no important positional args for a version command
        vec![]
    }
}
