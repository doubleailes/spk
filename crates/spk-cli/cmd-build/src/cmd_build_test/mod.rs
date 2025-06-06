// Copyright (c) Contributors to the SPK project.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/spkenv/spk

use clap::Parser;
use rstest::rstest;
use spfs::storage::prelude::*;
use spk_schema::RuntimeEnvironment;
use spk_schema::foundation::fixtures::*;
use spk_schema::foundation::option_map;
use spk_schema::ident::version_ident;
use spk_schema::ident_component::Component;
use spk_schema::option_map::HOST_OPTIONS;
use spk_storage::fixtures::*;

use super::Build;
use crate::{build_package, try_build_package};

mod environment;
mod variant_filter;

#[derive(Parser)]
struct Opt {
    #[clap(flatten)]
    build: Build,
}

#[rstest]
#[case::cli("cli")]
#[case::checks("checks")]
#[case::resolvo("resolvo")]
#[tokio::test]
async fn test_variant_options_contribute_to_build_hash(
    tmpdir: tempfile::TempDir,
    #[case] solver_to_run: &str,
) {
    // A var that appears in the variant list and doesn't appear in the
    // build.options list should still affect the build hash / produce a
    // unique build.
    let rt = spfs_runtime().await;

    build_package!(
        tmpdir,
        "three-variants.spk.yaml",
        br#"
api: v0/package
pkg: three-variants/1.0.0

build:
  variants:
    - { python.abi: cp27mu }
    - { python.abi: cp37m }
    - { python.abi: cp39 }
  script:
    - "true"
"#,
        solver_to_run
    );

    let ident = version_ident!("three-variants/1.0.0");

    let non_src_builds = rt
        .tmprepo
        .list_package_builds(&ident)
        .await
        .unwrap()
        .into_iter()
        .filter(|b| !b.is_source());

    assert_eq!(non_src_builds.count(), 3, "Expected three distinct builds");
}

#[rstest]
#[case::cli("cli")]
#[case::checks("checks")]
#[case::resolvo("resolvo")]
#[tokio::test]
async fn test_build_hash_not_affected_by_dependency_version(
    tmpdir: tempfile::TempDir,
    #[case] solver_to_run: &str,
) {
    // The same recipe should produce the same build hash even if there is a
    // change in its dependencies (at resolve time).
    let rt = spfs_runtime().await;

    // Build a version 1.0.0 of some package.
    build_package!(
        tmpdir,
        "dependency.spk.yaml",
        br#"
api: v0/package
pkg: dependency/1.0.0

build:
  script:
    - "true"
"#,
        solver_to_run
    );

    // Build a package that depends on "dependency".
    let package_filename = build_package!(
        tmpdir,
        "package.spk.yaml",
        br#"
api: v0/package
pkg: package/1.0.0

build:
  options:
    - pkg: dependency
  script:
    - "true"
"#,
        solver_to_run
    );

    // Now build a newer version of the dependency.
    build_package!(
        tmpdir,
        "dependency.spk.yaml",
        br#"
api: v0/package
pkg: dependency/1.0.1

build:
  script:
    - "true"
"#,
        solver_to_run
    );

    // And build the other package again.
    build_package!(tmpdir, package_filename, solver_to_run);

    // The second time building "package" we expect it to build something with
    // the _same_ build digest (e.g., the change in version of one of its
    // dependencies shouldn't affect the build digest). Verify this by checking
    // that there is still only one build of this package.

    let ident = version_ident!("package/1.0.0");

    let non_src_builds = rt
        .tmprepo
        .list_package_builds(&ident)
        .await
        .unwrap()
        .into_iter()
        .filter(|b| !b.is_source());

    assert_eq!(non_src_builds.count(), 1, "Expected one build");
}

#[rstest]
#[case::cli("cli")]
#[case::checks("checks")]
#[case::resolvo("resolvo")]
#[tokio::test]
async fn test_build_with_circular_dependency_allow_with_validation(
    tmpdir: tempfile::TempDir,
    #[case] solver_to_run: &str,
) {
    // The system should not allow a package to be built that has a circular
    // dependency.
    let _rt = spfs_runtime().await;

    // Start out with a package with no dependencies.
    let r = try_build_package!(
        tmpdir,
        "one.spk.yaml",
        br#"
api: v0/package
pkg: one/1.0.0

build:
  script:
    - "true"
"#,
        solver_to_run
    );

    r.1.expect("Expected initial build of one to succeed");

    // Build a package that depends on "one".
    let r = try_build_package!(
        tmpdir,
        "two.spk.yaml",
        br#"
api: v0/package
pkg: two/1.0.0

build:
  options:
    - pkg: one
  script:
    - "true"

install:
  requirements:
    - pkg: one
      fromBuildEnv: true
"#,
        solver_to_run
    );

    r.1.expect("Expected build of two to succeed");

    // Now build a newer version of "one" that depends on "two".
    let r = try_build_package!(
        tmpdir,
        "one.spk.yaml",
        br#"
api: v0/package
pkg: one/1.0.0

build:
  options:
    - pkg: two
  script:
    - "true"
  validation:
    rules:
      - allow: RecursiveBuild

install:
  requirements:
    - pkg: two
      fromBuildEnv: true
"#,
        solver_to_run
    );

    r.1.expect("Expected build of one to succeed");
}

#[rstest]
#[case::cli("cli")]
#[case::checks("checks")]
#[case::resolvo("resolvo")]
#[tokio::test]
async fn test_package_with_circular_dep_can_modify_files(
    tmpdir: tempfile::TempDir,
    #[case] solver_to_run: &str,
) {
    // A package that depends on itself should be able to modify files
    // belonging to itself.
    let _rt = spfs_runtime().await;

    build_package!(
        tmpdir,
        "other.spk.yaml",
        br#"
api: v0/package
pkg: other/1.0.0
build:
  script:
    - echo "1.0.0" > $PREFIX/a.txt
    - echo "1.0.0" > $PREFIX/z.txt
"#,
        solver_to_run
    );

    build_package!(
        tmpdir,
        "circ.spk.yaml",
        br#"
api: v0/package
pkg: circ/1.0.0
build:
  script:
    - echo "1.0.0" > $PREFIX/version.txt
"#,
        solver_to_run
    );

    // Force middle to pick up exactly 1.0.0 so for the multiple builds below
    // it doesn't pick up an already-built 1.0.1 of circ and the contents of
    // version.txt will still be "1.0.0" during the build of circ.
    build_package!(
        tmpdir,
        "middle.spk.yaml",
        br#"
api: v0/package
pkg: middle/1.0.0
build:
  options:
    - pkg: circ/=1.0.0
  script:
    - "true"
install:
  requirements:
    - pkg: circ/=1.0.0
"#,
        solver_to_run
    );

    // Attempt to build a newer version of circ, but now it depends on `middle`
    // creating a circular dependency. This build should succeed even though it
    // modifies a file belonging to "existing files" because the file it
    // modifies belongs to [a different version of] the same package as is
    // being built.
    build_package!(
        tmpdir,
        "circ.spk.yaml",
        br#"
api: v0/package
pkg: circ/1.0.1
build:
  options:
    - pkg: middle
  script:
    # this test is only valid if $PREFIX/version.txt exists already
    - test -f $PREFIX/version.txt
    - echo "1.0.1" > $PREFIX/version.txt
  validation:
    rules:
      - allow: RecursiveBuild
"#,
        solver_to_run
    );

    for other_file in ["a", "z"] {
        // Attempt to build a new version of circ but also modify a file belonging
        // to some other package. This should still be caught as an illegal
        // operation.
        //
        // We attempt this twice with two different filenames, one that sorts
        // before "version.txt" and one that sorts after, to exercise the case
        // where modifying the file from our own package is encountered first,
        // to prove that even though it allows the first modification, it still
        // checks for more.
        try_build_package!(
            tmpdir,
            "circ.spk.yaml",
            format!(
                r#"
pkg: circ/1.0.1
build:
  options:
    - pkg: middle
    - pkg: other
  script:
    # this test is only valid if $PREFIX/version.txt exists already
    - test -f $PREFIX/version.txt
    - echo "1.0.1" > $PREFIX/version.txt
    # try to modify a file belonging to 'other' too
    - echo "1.0.1" > $PREFIX/{other_file}.txt
"#,
            ),
            solver_to_run
        )
        .1
        .expect_err("Expected build to fail");
    }
}

#[rstest]
#[case::cli("cli")]
#[case::checks("checks")]
#[case::resolvo("resolvo")]
#[tokio::test]
async fn test_package_with_circular_dep_can_build_major_version_change(
    tmpdir: tempfile::TempDir,
    #[case] solver_to_run: &str,
) {
    // A package that depends on itself should be able to build a new major
    // version of itself, as in something not compatible with the version
    // being brought in via the circular dependency.
    let _rt = spfs_runtime().await;

    build_package!(
        tmpdir,
        "circ.spk.yaml",
        br#"
api: v0/package
pkg: circ/1.0.0
build:
  script:
    - echo "1.0.0" > $PREFIX/version.txt
"#,
        solver_to_run
    );

    build_package!(
        tmpdir,
        "middle.spk.yaml",
        br#"
api: v0/package
pkg: middle/1.0.0
build:
  options:
    - pkg: circ
  script:
    - "true"
install:
  requirements:
    - pkg: circ
      fromBuildEnv: true
"#,
        solver_to_run
    );

    // Attempt to build a 2.0.0 version of circ, which shouldn't prevent
    // middle from being able to resolve the 1.0.0 version of circ.
    build_package!(
        tmpdir,
        "circ.spk.yaml",
        br#"
api: v0/package
pkg: circ/2.0.0
build:
  options:
    - pkg: middle
  script:
    - echo "2.0.0" > $PREFIX/version.txt
  validation:
    rules:
      - allow: RecursiveBuild
"#,
        solver_to_run
    );
}

#[rstest]
#[case::cli("cli")]
#[case::checks("checks")]
#[case::resolvo("resolvo")]
#[tokio::test]
async fn test_package_with_circular_dep_collects_all_files(
    tmpdir: tempfile::TempDir,
    #[case] solver_to_run: &str,
) {
    // Building a new version of a package that depends on itself should
    // produce a package containing all the expected files, even if the new
    // build creates files with the same content as the previous build.
    let rt = spfs_runtime().await;

    build_package!(
        tmpdir,
        "circ.spk.yaml",
        br#"
api: v0/package
pkg: circ/1.0.0
build:
  script:
    - echo "1.0.0" > $PREFIX/version.txt
    - echo "hello world" > $PREFIX/hello.txt
    - echo "unchanged" > $PREFIX/unchanged.txt
"#,
        solver_to_run
    );

    build_package!(
        tmpdir,
        "middle.spk.yaml",
        br#"
api: v0/package
pkg: middle/1.0.0
build:
  options:
    - pkg: circ
  script:
    - "true"
install:
  requirements:
    - pkg: circ
      fromBuildEnv: true
"#,
        solver_to_run
    );

    // This build overwrites a file from the previous build, but it has the same
    // contents. It should still be detected as a file that needs to be part of
    // the newly made package.
    build_package!(
        tmpdir,
        "circ.spk.yaml",
        br#"
api: v0/package
pkg: circ/2.0.0
build:
  options:
    - pkg: middle
  script:
    - echo "2.0.0" > $PREFIX/version.txt
    - echo "hello world" > $PREFIX/hello.txt
  validation:
    rules:
      - allow: RecursiveBuild
"#,
        solver_to_run
    );

    let build = rt
        .tmprepo
        .list_package_builds(&version_ident!("circ/2.0.0"))
        .await
        .unwrap()
        .into_iter()
        .find(|b| !b.is_source())
        .unwrap();

    let digest = *rt
        .tmprepo
        .read_components(&build)
        .await
        .unwrap()
        .get(&Component::Run)
        .unwrap();

    let spk_storage::RepositoryHandle::SPFS(repo) = &*rt.tmprepo else {
        panic!("Expected SPFS repo");
    };

    let layer = repo.read_layer(digest).await.unwrap();

    let manifest = repo
        .read_manifest(
            *layer
                .manifest()
                .expect("Layer should have a manifest in this test"),
        )
        .await
        .unwrap()
        .to_tracking_manifest();

    let entry = manifest.get_path("hello.txt");
    assert!(
        entry.is_some(),
        "should capture file created in build but unmodified from previous build"
    );
    let entry = manifest.get_path("unchanged.txt");
    assert!(
        entry.is_none(),
        "should not capture file from old build that was not modified in new build"
    );
}

#[rstest]
#[case::cli("cli")]
#[case::checks("checks")]
#[case::resolvo("resolvo")]
#[tokio::test]
async fn test_package_with_circular_dep_does_not_collect_file_removals(
    tmpdir: tempfile::TempDir,
    #[case] solver_to_run: &str,
) {
    // Building a new version of a package that depends on itself should not
    // collect "negative files" (e.g., files that were removed in the new
    // build).
    let rt = spfs_runtime().await;

    // This extra package "empty" helps create the situation that ends up with
    // file removals getting collected in a real world example.
    build_package!(
        tmpdir,
        "empty.spk.yaml",
        br#"
api: v0/package
pkg: empty/1.0.0
build:
  script:
    - mkdir $PREFIX/subdir
"#,
        solver_to_run
    );

    build_package!(
        tmpdir,
        "circ.spk.yaml",
        br#"
api: v0/package
pkg: circ/1.0.0
build:
  script:
    - echo "1.0.0" > $PREFIX/version.txt
    - mkdir -p $PREFIX/subdir/v1
    - echo "hello world" > $PREFIX/subdir/v1/hello.txt
"#,
        solver_to_run
    );

    // This build deletes a subdir that is owned by the previous build. It should
    // not be collected (as a mask) as part of the new build.
    build_package!(
        tmpdir,
        "circ.spk.yaml",
        br#"
api: v0/package
pkg: circ/2.0.0
build:
  options:
    - pkg: circ
    - pkg: empty
  script:
    - echo "2.0.0" > $PREFIX/version.txt
    - rm -rf $PREFIX/subdir/v1
    - mkdir -p $PREFIX/subdir/v2
    - echo "hello world" > $PREFIX/subdir/v2/hello.txt
  validation:
    rules:
      - allow: RecursiveBuild
"#,
        solver_to_run
    );

    let build = rt
        .tmprepo
        .list_package_builds(&version_ident!("circ/2.0.0"))
        .await
        .unwrap()
        .into_iter()
        .find(|b| !b.is_source())
        .unwrap();

    let digest = *rt
        .tmprepo
        .read_components(&build)
        .await
        .unwrap()
        .get(&Component::Run)
        .unwrap();

    let spk_storage::RepositoryHandle::SPFS(repo) = &*rt.tmprepo else {
        panic!("Expected SPFS repo");
    };

    let layer = repo.read_layer(digest).await.unwrap();

    let manifest = repo
        .read_manifest(
            *layer
                .manifest()
                .expect("Layer should have a manifest in this test"),
        )
        .await
        .unwrap()
        .to_tracking_manifest();

    let entry = manifest.get_path("subdir/v1");
    assert!(
        entry.is_none(),
        "should not capture entry deleted in new build"
    );
}

#[allow(clippy::too_many_arguments)]
mod workaround_rstest_not_preserving_attrs {
    use super::*;

    #[rstest]
    // cases not involving host options
    #[should_panic]
    #[case::empty_value_fails("varname", "", &["yes", "no"], true)]
    #[case::non_empty_value_succeeds("varname/yes", "", &["yes", "no"], true)]
    #[should_panic]
    #[case::non_empty_value_bad_value_fails("varname/what", "", &["yes", "no"], true)]
    // cases involving host options
    #[case::empty_value_for_host_option_succeeds("os", "", &["linux", "windows"], true)]
    #[case::non_empty_value_for_host_option_succeeds("os", "linux", &["linux", "windows"], true)]
    #[should_panic]
    #[case::empty_value_for_host_option_fails_if_host_options_disabled("os", "", &["linux", "windows"], false)]
    // this case verifies that the --no-host option is respected
    #[case::non_empty_value_for_host_option_good_value_succeeds_with_host_options_disabled("os", "beos", &["beos"], false)]
    #[should_panic]
    #[case::non_empty_value_for_host_option_bad_value_fails_with_host_options_disabled("os", "beos", &["linux", "windows"], false)]
    // this case passes because host options override default values, and the
    // provided host option value of "linux" is a valid choice.
    #[case::non_empty_value_for_host_option_bad_value_succeeds_with_host_options_enabled("os", "beos", &["linux", "windows"], true)]
    #[tokio::test]
    #[serial_test::serial(host_options)]
    async fn test_options_with_choices_and_empty_values(
        tmpdir: tempfile::TempDir,
        #[case] name: &'static str,
        #[case] value: &'static str,
        #[case] choices: &'static [&'static str],
        #[case] host_options_enabled: bool,
        #[values("cli", "checks", "resolvo")] solver_to_run: &'static str,
    ) {
        let _rt = spfs_runtime().await;

        // Force "os" host option to "linux" to make this test pass on any OS.
        HOST_OPTIONS
            .scoped_options(Ok(option_map! { "os" => "linux" }), async move {
                let name_maybe_value = if value.is_empty() {
                    name.to_string()
                } else {
                    format!("{name}/{value}")
                };
                let generated_spec = format!(
                    r#"
pkg: dummy/1.0.0
api: v0/package
build:
    options:
        - var: {name_maybe_value}
          choices: [{choices}]
    script:
        - "true"
"#,
                    choices = choices.join(", ")
                );

                if !host_options_enabled {
                    build_package!(
                        tmpdir,
                        "dummy.spk.yaml",
                        generated_spec,
                        solver_to_run,
                        "--no-host"
                    );
                } else {
                    build_package!(tmpdir, "dummy.spk.yaml", generated_spec, solver_to_run);
                }

                Ok::<_, ()>(())
            })
            .await
            .unwrap();
    }
}

/// A package may contain files/directories with a leading dot
#[rstest]
#[case::cli("cli")]
#[case::checks("checks")]
#[case::resolvo("resolvo")]
#[tokio::test]
async fn test_dot_files_are_collected(tmpdir: tempfile::TempDir, #[case] solver_to_run: &str) {
    let rt = spfs_runtime().await;

    build_package!(
        tmpdir,
        "dot.spk.yaml",
        br#"
api: v0/package
pkg: dot/1.0.0
build:
  script:
    - touch /spfs/no_dot
    - touch /spfs/.dot
    - mkdir /spfs/dot
    - touch /spfs/dot/.dot
    - mkdir /spfs/.dot2
    - touch /spfs/.dot2/dot
    - touch /spfs/.dot2/.dot
    - ln -s .dot2 /spfs/.dot3
"#,
        solver_to_run
    );

    let build = rt
        .tmprepo
        .list_package_builds(&version_ident!("dot/1.0.0"))
        .await
        .unwrap()
        .into_iter()
        .find(|b| !b.is_source())
        .unwrap();

    let digest = *rt
        .tmprepo
        .read_components(&build)
        .await
        .unwrap()
        .get(&Component::Run)
        .unwrap();

    let spk_storage::RepositoryHandle::SPFS(repo) = &*rt.tmprepo else {
        panic!("Expected SPFS repo");
    };

    let layer = repo.read_layer(digest).await.unwrap();

    let manifest = repo
        .read_manifest(
            *layer
                .manifest()
                .expect("Layer should have a manifest in this test"),
        )
        .await
        .unwrap()
        .to_tracking_manifest();

    for path in &[
        "/no_dot",
        "/.dot",
        "/dot/.dot",
        "/.dot2",
        "/.dot2/dot",
        "/.dot2/.dot",
        "/.dot3",
    ] {
        let entry = manifest.get_path(path);
        assert!(
            entry.is_some(),
            "should capture file/directory with leading dot: {path}"
        );
    }
}

#[rstest]
#[case::cli("cli")]
#[case::checks("checks")]
#[case::resolvo("resolvo")]
#[tokio::test]
async fn test_package_with_environment_ops_preserves_ops_in_recipe(
    tmpdir: tempfile::TempDir,
    #[case] solver_to_run: &str,
) {
    let rt = spfs_runtime().await;

    build_package!(
        tmpdir,
        "env-ops.spk.yaml",
        br#"
api: v0/package
pkg: env-ops/1.0.0
build:
  script:
    - true
install:
  environment:
    - set: FOO
      value: bar
"#,
        solver_to_run
    );

    let recipe = rt
        .tmprepo
        .read_recipe(&version_ident!("env-ops/1.0.0"))
        .await
        .unwrap();

    assert!(
        !recipe.runtime_environment().is_empty(),
        "should have environment ops"
    );
}
