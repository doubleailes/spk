// Copyright (c) Contributors to the SPK project.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/spkenv/spk

use std::collections::BTreeMap;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use spk_config::Metadata;
use spk_schema_foundation::IsDefault;

use crate::{Error, Result};

#[cfg(test)]
#[path = "./meta_test.rs"]
mod meta_test;

#[derive(Default, Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Meta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
}

impl Meta {
    pub fn has_label_with_value(&self, label: &str, value: &str) -> bool {
        if let Some(label_value) = self.labels.get(label) {
            return *label_value == value;
        }
        false
    }

    pub fn update_metadata(&mut self, global_config: &Metadata) -> Result<i32> {
        for config in global_config.global.iter() {
            let cmd = &config.command;
            let Some(executable) = cmd.first() else {
                tracing::warn!("Empty command in global metadata config");
                continue;
            };

            let args = &cmd[1..];

            let mut command = Command::new(executable);
            command.args(args);
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());

            match command
                .spawn()
                .map_err(|err| {
                    Error::ProcessSpawnError(
                        format!("error running configured metadata command: {err}").into(),
                    )
                })?
                .wait_with_output()
            {
                Ok(out) => {
                    let json: serde_json::Value = match serde_json::from_reader(&*out.stdout) {
                        Ok(j) => j,
                        Err(e) => {
                            return Err(Error::String(format!("Unable to read json output: {e}")));
                        }
                    };

                    if let Some(map) = json.as_object() {
                        for (k, v) in map {
                            v.as_str()
                                .and_then(|val| self.labels.insert(k.clone(), val.to_string()));
                        }
                    }
                }
                Err(e) => return Err(Error::String(format!("Failed to execute command: {e}"))),
            }
        }
        Ok(0)
    }
}

impl IsDefault for Meta {
    fn is_default(&self) -> bool {
        self == &Self::default()
    }
}
