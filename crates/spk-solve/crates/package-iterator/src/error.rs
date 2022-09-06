// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    SpkNameError(#[from] spk_schema::foundation::name::Error),
    #[error(transparent)]
    SpkStorageError(#[from] spk_storage::Error),
    #[error(transparent)]
    SpkValidatorsError(#[from] spk_schema::validators::Error),
    #[error(transparent)]
    SpkVersionRangeError(#[from] spk_schema::foundation::version_range::Error),
    #[error("Error: {0}")]
    String(String),
}