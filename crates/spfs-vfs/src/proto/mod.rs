// Copyright (c) Contributors to the SPK project.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/spkenv/spk

//! Protocol Buffer message formats and conversions for the spfs virtual filesystem.

mod generated {
    #![allow(missing_docs)]
    #![allow(clippy::derive_partial_eq_without_eq)]
    tonic::include_proto!("spfs_vfs");
}

pub use generated::*;
