// Copyright (c) Contributors to the SPK project.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/spkenv/spk

use relative_path::RelativePathBuf;

pub trait MetadataPath {
    /// Return the relative path for package metadata for an ident.
    ///
    /// Package metadata is stored on disk within each package, for example:
    ///     /spfs/spk/pkg/pkg-name/1.0.0/CU7ZWOIF
    ///
    /// This method should return only the ident part:
    ///     pkg-name/1.0.0/CU7ZWOIF
    fn metadata_path(&self) -> RelativePathBuf;
}
