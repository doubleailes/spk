// Copyright (c) Contributors to the SPK project.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/spkenv/spk

mod error;
mod exec;

pub use error::{Error, Result};
pub use exec::{
    pull_resolved_runtime_layers,
    resolve_runtime_layers,
    setup_current_runtime,
    setup_runtime,
    solution_to_resolved_runtime_layers,
    ConflictingPackagePair,
    ResolvedLayer,
    ResolvedLayers,
};
