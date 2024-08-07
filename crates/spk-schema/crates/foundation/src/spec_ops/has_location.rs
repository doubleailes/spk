// Copyright (c) Contributors to the SPK project.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/spkenv/spk

use std::sync::Arc;

use crate::name::RepositoryName;

/// Some item that has an associated repository
pub trait HasLocation {
    /// The associated repository name
    fn location(&self) -> &RepositoryName;
}

impl<T: HasLocation> HasLocation for Arc<T> {
    fn location(&self) -> &RepositoryName {
        (**self).location()
    }
}

impl<T: HasLocation> HasLocation for &T {
    fn location(&self) -> &RepositoryName {
        (**self).location()
    }
}
