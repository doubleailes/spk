// Copyright (c) Contributors to the SPK project.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/spkenv/spk

mod error;
mod graph;

pub use error::{
    Error,
    GetCurrentResolveError,
    GetCurrentResolveResult,
    GetMergedRequestError,
    GetMergedRequestResult,
    Result,
};
pub use graph::{
    CachedHash,
    Change,
    Decision,
    Graph,
    GraphError,
    Node,
    Note,
    RequestPackage,
    RequestVar,
    SetOptions,
    SkipPackageNote,
    State,
    StepBack,
    DEAD_STATE,
    DUPLICATE_REQUESTS_COUNT,
    REQUESTS_FOR_SAME_PACKAGE_COUNT,
};
