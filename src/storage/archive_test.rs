// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk
use rstest::rstest;

use super::{export_package, import_package};
use crate::{api, build, fixtures::*, storage};

#[rstest]
fn test_archive_io(tmpdir: tempdir::TempDir) {
    // spec = api.Spec.from_dict(
    //     {
    //         "pkg": "spk-archive-test/0.0.1",
    //         "build": {"script": "touch /spfs/file.txt"},
    //     }
    // )
    // repo = storage.local_repository()
    // repo.publish_spec(spec)
    // builder = build.BinaryPackageBuilder.from_spec(spec).with_source(".")
    // spec = builder.build()
    // filename = tmpdir.join("achive.spk").ensure().strpath
    // export_package(spec.pkg, filename)
    // actual: List[str] = []
    // with tarfile.open(filename) as tar:
    //     for file in tar:
    //         actual.append(file.name)
    // actual.sort()
    // top_level_and_tags = list(filter(lambda p: "/" not in p or "tags" in p, actual))
    // assert top_level_and_tags == [
    //     ".",
    //     "VERSION",
    //     "objects",
    //     "payloads",
    //     "renders",
    //     "tags",
    //     "tags/spk",
    //     "tags/spk/pkg",
    //     "tags/spk/pkg/spk-archive-test",
    //     "tags/spk/pkg/spk-archive-test/0.0.1",
    //     "tags/spk/pkg/spk-archive-test/0.0.1/3I42H3S6",
    //     "tags/spk/pkg/spk-archive-test/0.0.1/3I42H3S6.tag",
    //     "tags/spk/pkg/spk-archive-test/0.0.1/3I42H3S6/build.tag",
    //     "tags/spk/pkg/spk-archive-test/0.0.1/3I42H3S6/run.tag",
    //     "tags/spk/spec",
    //     "tags/spk/spec/spk-archive-test",
    //     "tags/spk/spec/spk-archive-test/0.0.1",
    //     "tags/spk/spec/spk-archive-test/0.0.1.tag",
    //     "tags/spk/spec/spk-archive-test/0.0.1/3I42H3S6.tag",
    // ]
    // import_package(filename)
    todo!()
}

#[rstest]
fn test_archive_create_parents(tmpdir: tempdir::TempDir) {
    // spec = api.Spec.from_dict(
    //     {
    //         "pkg": "spk-archive-test/0.0.1",
    //         "build": {"script": "touch /spfs/file.txt"},
    //     }
    // )
    // repo = storage.local_repository()
    // repo.publish_spec(spec)
    // builder = build.BinaryPackageBuilder.from_spec(spec).with_source(".")
    // spec = builder.build()
    // filename = tmpdir.join("deep/nested/path/archive.spk").strpath
    // export_package(spec.pkg, filename)
    todo!()
}
