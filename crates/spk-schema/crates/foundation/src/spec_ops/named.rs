// Copyright (c) Contributors to the SPK project.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/spkenv/spk

use std::sync::Arc;

use crate::name::PkgName;

/// Some item that has an associated package name
#[enum_dispatch::enum_dispatch]
pub trait Named {
    /// The name of the associated package
    fn name(&self) -> &PkgName;
}

impl<T: Named> Named for Arc<T> {
    fn name(&self) -> &PkgName {
        (**self).name()
    }
}

impl<T: Named> Named for Box<T> {
    fn name(&self) -> &PkgName {
        (**self).name()
    }
}

impl<T: Named> Named for &T {
    fn name(&self) -> &PkgName {
        (**self).name()
    }
}
