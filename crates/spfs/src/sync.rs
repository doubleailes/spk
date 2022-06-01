// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use futures::stream::{FuturesUnordered, TryStreamExt};
use tokio::sync::Semaphore;

use crate::{encoding, prelude::*};
use crate::{graph, storage, tracking, Error, Result};

#[cfg(test)]
#[path = "./sync_test.rs"]
mod sync_test;

/// Handles the syncing of data between repositories
pub struct Syncer<'src, 'dst, Reporter: SyncReporter = ConsoleSyncReporter> {
    src: &'src storage::RepositoryHandle,
    dest: &'dst storage::RepositoryHandle,
    reporter: Option<Reporter>,
    skip_existing_tags: bool,
    skip_existing_objects: bool,
    skip_existing_payloads: bool,
    manifest_semaphore: Semaphore,
    payload_semaphore: Semaphore,
}

impl<'src, 'dst, Reporter> Syncer<'src, 'dst, Reporter>
where
    Reporter: SyncReporter,
{
    pub fn new(
        src: &'src storage::RepositoryHandle,
        dest: &'dst storage::RepositoryHandle,
    ) -> Self {
        Self {
            src,
            dest,
            reporter: None,
            skip_existing_tags: true,
            skip_existing_objects: true,
            skip_existing_payloads: true,
            manifest_semaphore: Semaphore::new(50),
            payload_semaphore: Semaphore::new(50),
        }
    }

    /// When true, also sync any tag that already exists in the destination repo.
    ///
    /// This is off by default, but can be enabled in order to retrieve updated tag
    /// information from the source repo.
    pub fn with_sync_existing_tags(&mut self, sync_existing: bool) -> &mut Self {
        self.skip_existing_tags = !sync_existing;
        if !sync_existing {
            self.skip_existing_payloads = true;
        }
        self
    }

    /// When true, also sync any object that already exists in the destination repo.
    ///
    /// This is off by default, but can be enabled in order to repair a corrupt
    /// repository that has some parent object but is missing one or more children.
    ///
    /// Setting this to false will also disable [`Self::with_sync_existing_payload`].
    pub fn with_sync_existing_objects(&mut self, sync_existing: bool) -> &mut Self {
        self.skip_existing_objects = !sync_existing;
        if !sync_existing {
            self.skip_existing_payloads = true;
        }
        self
    }

    /// When true, also sync any payload that already exists in the destination repo.
    ///
    /// This is off by default, but can be enabled in order to repair a corrupt
    /// repository. Setting this to true, also implies [`Self::with_sync_existing_object`].
    pub fn with_sync_existing_payloads(&mut self, sync_existing: bool) -> &mut Self {
        self.skip_existing_payloads = !sync_existing;
        if sync_existing {
            self.skip_existing_objects = false;
        }
        self
    }

    /// Set how many manifests/layers can be processed at once.
    ///
    /// The possible total concurrent sync tasks will be the
    /// layer concurrency plus the payload concurrency.
    pub fn with_max_concurrent_manifests(&mut self, concurrency: usize) -> &mut Self {
        self.manifest_semaphore = Semaphore::new(concurrency);
        self
    }

    /// Set how many payloads/files can be processed at once.
    ///
    /// The possible total concurrent sync tasks will be the
    /// layer concurrency plus the payload concurrency.
    pub fn with_max_payload_concurrency(&mut self, concurrency: usize) -> &mut Self {
        self.payload_semaphore = Semaphore::new(concurrency);
        self
    }

    /// Report progress to the given instnace, replacing any existing one
    pub fn with_reporter(&mut self, reporter: Reporter) -> &mut Self {
        self.reporter = Some(reporter);
        self
    }

    /// Sync the object(s) referenced by the given string.
    ///
    /// Any valid [`spfs::tracking::EnvSpec`] is accepted as a reference.
    pub async fn sync_ref<R: AsRef<str>>(&self, reference: R) -> Result<SyncEnvResult> {
        let env_spec = reference.as_ref().parse()?;
        self.sync_env(env_spec).await
    }

    /// Sync all of the objects identified by the given env.
    pub async fn sync_env(&self, env: tracking::EnvSpec) -> Result<SyncEnvResult> {
        self.reporter.visit_env(&env);
        let mut futures = FuturesUnordered::new();
        for item in env.iter().cloned() {
            futures.push(self.sync_env_item(item));
        }
        let mut results = Vec::with_capacity(env.len());
        while let Some(result) = futures.try_next().await? {
            results.push(result);
        }
        let res = SyncEnvResult { env, results };
        self.reporter.synced_env(&res);
        Ok(res)
    }

    /// Sync one environment item and any associated data.
    pub async fn sync_env_item(&self, item: tracking::EnvSpecItem) -> Result<SyncEnvItemResult> {
        self.reporter.visit_env_item(&item);
        let res = match item {
            tracking::EnvSpecItem::Digest(digest) => self
                .sync_digest(digest)
                .await
                .map(SyncEnvItemResult::Object)?,
            tracking::EnvSpecItem::PartialDigest(digest) => self
                .sync_partial_digest(digest)
                .await
                .map(SyncEnvItemResult::Object)?,
            tracking::EnvSpecItem::TagSpec(tag_spec) => {
                self.sync_tag(tag_spec).await.map(SyncEnvItemResult::Tag)?
            }
        };
        self.reporter.synced_env_item(&res);
        Ok(res)
    }

    /// Sync the identified tag instance and its target.
    pub async fn sync_tag(&self, tag: tracking::TagSpec) -> Result<SyncTagResult> {
        if self.skip_existing_tags && self.dest.resolve_tag(&tag).await.is_ok() {
            return Ok(SyncTagResult::Skipped);
        }
        self.reporter.visit_tag(&tag);
        let resolved = self.src.resolve_tag(&tag).await?;
        let result = self.sync_digest(resolved.target).await?;
        self.dest.push_raw_tag(&resolved).await?;
        let res = SyncTagResult::Synced { tag, result };
        self.reporter.synced_tag(&res);
        Ok(res)
    }

    pub async fn sync_partial_digest(
        &self,
        partial: encoding::PartialDigest,
    ) -> Result<SyncObjectResult> {
        let digest = self.src.resolve_full_digest(&partial).await?;
        let obj = self.src.read_object(digest).await?;
        self.sync_object(obj).await
    }

    pub async fn sync_digest(&self, digest: encoding::Digest) -> Result<SyncObjectResult> {
        let obj = self.src.read_object(digest).await?;
        self.sync_object(obj).await
    }

    #[async_recursion::async_recursion]
    pub async fn sync_object(&self, obj: graph::Object) -> Result<SyncObjectResult> {
        use graph::Object;
        self.reporter.visit_object(&obj);
        let res = match obj {
            Object::Layer(obj) => SyncObjectResult::Layer(self.sync_layer(obj).await?),
            Object::Platform(obj) => SyncObjectResult::Platform(self.sync_platform(obj).await?),
            Object::Blob(obj) => SyncObjectResult::Blob(self.sync_blob(obj).await?),
            Object::Manifest(obj) => SyncObjectResult::Manifest(self.sync_manifest(obj).await?),
            Object::Tree(obj) => SyncObjectResult::Tree(obj),
            Object::Mask => SyncObjectResult::Mask,
        };
        self.reporter.synced_object(&res);
        Ok(res)
    }

    pub async fn sync_platform(&self, platform: graph::Platform) -> Result<SyncPlatformResult> {
        let digest = platform.digest()?;
        if self.skip_existing_objects && self.dest.has_platform(digest).await {
            return Ok(SyncPlatformResult::Skipped);
        }
        self.reporter.visit_platform(&platform);

        let mut futures = FuturesUnordered::new();
        for digest in &platform.stack {
            futures.push(self.sync_digest(*digest));
        }
        let mut results = Vec::with_capacity(futures.len());
        while let Some(result) = futures.try_next().await? {
            results.push(result);
        }

        let platform = self.dest.create_platform(platform.stack).await?;

        let res = SyncPlatformResult::Synced { platform, results };
        self.reporter.synced_platform(&res);
        Ok(res)
    }

    pub async fn sync_layer(&self, layer: graph::Layer) -> Result<SyncLayerResult> {
        let layer_digest = layer.digest()?;
        if self.skip_existing_objects && self.dest.has_layer(layer_digest).await {
            return Ok(SyncLayerResult::Skipped);
        }

        self.reporter.visit_layer(&layer);
        let manifest = self.src.read_manifest(layer.manifest).await?;
        let result = self.sync_manifest(manifest).await?;
        self.dest
            .write_object(&graph::Object::Layer(layer.clone()))
            .await?;
        let res = SyncLayerResult::Synced { layer, result };
        self.reporter.synced_layer(&res);
        Ok(res)
    }

    pub async fn sync_manifest(&self, manifest: graph::Manifest) -> Result<SyncManifestResult> {
        let manifest_digest = manifest.digest()?;
        if self.skip_existing_objects && self.dest.has_manifest(manifest_digest).await {
            return Ok(SyncManifestResult::Skipped);
        }
        self.reporter.visit_manifest(&manifest);
        let _permit = self.manifest_semaphore.acquire().await;

        let entries: Vec<_> = manifest
            .list_entries()
            .into_iter()
            .cloned()
            .filter(|e| e.kind.is_blob())
            .collect();
        let mut results = Vec::with_capacity(entries.len());
        let mut futures = FuturesUnordered::new();
        for entry in entries {
            futures.push(self.sync_entry(entry));
        }
        while let Some(res) = futures.try_next().await? {
            results.push(res);
        }

        self.dest
            .write_object(&graph::Object::Manifest(manifest.clone()))
            .await?;

        let res = SyncManifestResult::Synced { manifest, results };
        self.reporter.synced_manifest(&res);
        Ok(res)
    }

    async fn sync_entry(&self, entry: graph::Entry) -> Result<SyncEntryResult> {
        if !entry.kind.is_blob() {
            return Ok(SyncEntryResult::Skipped);
        }
        self.reporter.visit_entry(&entry);
        let blob = graph::Blob {
            payload: entry.object,
            size: entry.size,
        };
        let result = self.sync_blob(blob).await?;
        let res = SyncEntryResult::Synced { entry, result };
        self.reporter.synced_entry(&res);
        Ok(res)
    }

    async fn sync_blob(&self, blob: graph::Blob) -> Result<SyncBlobResult> {
        if self.skip_existing_objects && self.dest.has_blob(blob.digest()).await {
            return Ok(SyncBlobResult::Skipped);
        }
        self.reporter.visit_blob(&blob);
        let result = self.sync_payload(blob.payload).await?;
        self.dest.write_blob(blob.clone()).await?;
        let res = SyncBlobResult::Synced { blob, result };
        self.reporter.synced_blob(&res);
        Ok(res)
    }

    async fn sync_payload(&self, digest: encoding::Digest) -> Result<SyncPayloadResult> {
        if self.skip_existing_payloads && self.dest.has_payload(digest).await {
            return Ok(SyncPayloadResult::Skipped);
        }

        self.reporter.visit_payload(digest);
        let _permit = self.payload_semaphore.acquire().await;
        let payload = self.src.open_payload(digest).await?;
        let (created_digest, size) = self.dest.write_data(payload).await?;
        if digest != created_digest {
            return Err(Error::String(format!(
                "Source repository provided blob that did not match the requested digest: wanted {digest}, got {created_digest}",
            )));
        }
        let res = SyncPayloadResult::Synced { size };
        self.reporter.synced_payload(&res);
        Ok(res)
    }
}

/// Receives updates from a sync process to be reported.
///
/// Unless the sync runs into errors, ever call to visit_* is
/// followed up by a call to the corresponding synced_*.
pub trait SyncReporter: Send + Sync {
    /// Called when an environment has been identified to sync
    fn visit_env(&self, _env: &tracking::EnvSpec) {}

    /// Called when a environment has finished syncing
    fn synced_env(&self, _result: &SyncEnvResult) {}

    /// Called when an environment item has been identified to sync
    fn visit_env_item(&self, _item: &tracking::EnvSpecItem) {}

    /// Called when a environment item has finished syncing
    fn synced_env_item(&self, _result: &SyncEnvItemResult) {}

    /// Called when a tag has been identified to sync
    fn visit_tag(&self, _tag: &tracking::TagSpec) {}

    /// Called when a tag has finished syncing
    fn synced_tag(&self, _result: &SyncTagResult) {}

    /// Called when an object has been identified to sync
    fn visit_object(&self, _obj: &graph::Object) {}

    /// Called when a object has finished syncing
    fn synced_object(&self, _result: &SyncObjectResult) {}

    /// Called when a platform has been identified to sync
    fn visit_platform(&self, _platform: &graph::Platform) {}

    /// Called when a platform has finished syncing
    fn synced_platform(&self, _result: &SyncPlatformResult) {}

    /// Called when an environment has been identified to sync
    fn visit_layer(&self, _layer: &graph::Layer) {}

    /// Called when a layer has finished syncing
    fn synced_layer(&self, _result: &SyncLayerResult) {}

    /// Called when a manifest has been identified to sync
    fn visit_manifest(&self, _manifest: &graph::Manifest) {}

    /// Called when a manifest has finished syncing
    fn synced_manifest(&self, _result: &SyncManifestResult) {}

    /// Called when an entry has been identified to sync
    fn visit_entry(&self, _entry: &graph::Entry) {}

    /// Called when an entry has finished syncing
    fn synced_entry(&self, _result: &SyncEntryResult) {}

    /// Called when a blob has been identified to sync
    fn visit_blob(&self, _blob: &graph::Blob) {}

    /// Called when a blob has finished syncing
    fn synced_blob(&self, _result: &SyncBlobResult) {}

    /// Called when a payload has been identified to sync
    fn visit_payload(&self, _digest: encoding::Digest) {}

    /// Called when a payload has finished syncing
    fn synced_payload(&self, _result: &SyncPayloadResult) {}
}

impl<T: SyncReporter> SyncReporter for Option<T> {
    fn visit_env(&self, env: &tracking::EnvSpec) {
        if let Some(ref r) = self {
            r.visit_env(env)
        }
    }

    fn synced_env(&self, result: &SyncEnvResult) {
        if let Some(ref r) = self {
            r.synced_env(result)
        }
    }

    fn visit_env_item(&self, item: &tracking::EnvSpecItem) {
        if let Some(ref r) = self {
            r.visit_env_item(item)
        }
    }

    fn synced_env_item(&self, result: &SyncEnvItemResult) {
        if let Some(ref r) = self {
            r.synced_env_item(result)
        }
    }

    fn visit_tag(&self, tag: &tracking::TagSpec) {
        if let Some(ref r) = self {
            r.visit_tag(tag)
        }
    }

    fn synced_tag(&self, result: &SyncTagResult) {
        if let Some(ref r) = self {
            r.synced_tag(result)
        }
    }

    fn visit_object(&self, obj: &graph::Object) {
        if let Some(ref r) = self {
            r.visit_object(obj)
        }
    }

    fn synced_object(&self, result: &SyncObjectResult) {
        if let Some(ref r) = self {
            r.synced_object(result)
        }
    }

    fn visit_platform(&self, platform: &graph::Platform) {
        if let Some(ref r) = self {
            r.visit_platform(platform)
        }
    }

    fn synced_platform(&self, result: &SyncPlatformResult) {
        if let Some(ref r) = self {
            r.synced_platform(result)
        }
    }

    fn visit_layer(&self, layer: &graph::Layer) {
        if let Some(ref r) = self {
            r.visit_layer(layer)
        }
    }

    fn synced_layer(&self, result: &SyncLayerResult) {
        if let Some(ref r) = self {
            r.synced_layer(result)
        }
    }

    fn visit_manifest(&self, manifest: &graph::Manifest) {
        if let Some(ref r) = self {
            r.visit_manifest(manifest)
        }
    }

    fn synced_manifest(&self, result: &SyncManifestResult) {
        if let Some(ref r) = self {
            r.synced_manifest(result)
        }
    }

    fn visit_entry(&self, entry: &graph::Entry) {
        if let Some(ref r) = self {
            r.visit_entry(entry)
        }
    }

    fn synced_entry(&self, result: &SyncEntryResult) {
        if let Some(ref r) = self {
            r.synced_entry(result)
        }
    }

    fn visit_blob(&self, blob: &graph::Blob) {
        if let Some(ref r) = self {
            r.visit_blob(blob)
        }
    }

    fn synced_blob(&self, result: &SyncBlobResult) {
        if let Some(ref r) = self {
            r.synced_blob(result)
        }
    }

    fn visit_payload(&self, digest: encoding::Digest) {
        if let Some(ref r) = self {
            r.visit_payload(digest)
        }
    }

    fn synced_payload(&self, result: &SyncPayloadResult) {
        if let Some(ref r) = self {
            r.synced_payload(result)
        }
    }
}

/// Reports sync progress to an interactive console via progress bars
pub struct ConsoleSyncReporter {
    renderer: Option<std::thread::JoinHandle<()>>,
    manifests: indicatif::ProgressBar,
    bytes: indicatif::ProgressBar,
}

impl Default for ConsoleSyncReporter {
    fn default() -> Self {
        let layers_style = indicatif::ProgressStyle::default_bar()
            .template("      {msg:<16.green} [{bar:40.cyan/dim}] {pos:>8}/{len:7}")
            .progress_chars("=>-");
        let payloads_style = indicatif::ProgressStyle::default_bar()
            .template("      {msg:<16.green} [{bar:40.cyan/dim}] {bytes:>8}/{total_bytes:7}")
            .progress_chars("=>-");
        let bars = indicatif::MultiProgress::new();
        let manifests = bars.add(
            indicatif::ProgressBar::new(0)
                .with_style(layers_style)
                .with_message("syncing layers"),
        );
        let bytes = bars.add(
            indicatif::ProgressBar::new(0)
                .with_style(payloads_style)
                .with_message("syncing payloads"),
        );
        // the progress bar must be constantly updated from another thread
        // or nothing will be shown in the terminal
        let renderer = Some(std::thread::spawn(move || {
            if let Err(err) = bars.join() {
                tracing::error!("Failed to render sync progress: {err}");
            }
        }));
        Self {
            renderer,
            manifests,
            bytes,
        }
    }
}

impl Drop for ConsoleSyncReporter {
    fn drop(&mut self) {
        self.bytes.finish_and_clear();
        self.manifests.finish_and_clear();
        if let Some(r) = self.renderer.take() {
            let _ = r.join();
        }
    }
}

impl SyncReporter for ConsoleSyncReporter {
    fn visit_manifest(&self, manifest: &graph::Manifest) {
        let total_bytes = manifest
            .list_entries()
            .into_iter()
            .cloned()
            .filter(|e| e.kind.is_blob())
            .fold(0, |cur, next| cur + next.size);
        self.manifests.inc_length(1);
        self.bytes.inc_length(total_bytes);
    }

    fn synced_manifest(&self, _result: &SyncManifestResult) {
        self.manifests.inc(1);
    }

    fn synced_blob(&self, result: &SyncBlobResult) {
        self.bytes.inc(result.summary().synced_payload_bytes);
    }
}

#[derive(Default, Debug)]
pub struct SyncSummary {
    pub skipped_tags: usize,
    pub synced_tags: usize,
    pub skipped_objects: usize,
    pub synced_objects: usize,
    pub skipped_payloads: usize,
    pub synced_payloads: usize,
    pub synced_payload_bytes: u64,
}

impl SyncSummary {
    fn skipped_one_object() -> Self {
        Self {
            skipped_objects: 1,
            ..Default::default()
        }
    }

    fn synced_one_object() -> Self {
        Self {
            synced_objects: 1,
            ..Default::default()
        }
    }
}

impl std::ops::AddAssign for SyncSummary {
    fn add_assign(&mut self, rhs: Self) {
        self.skipped_tags += rhs.skipped_tags;
        self.synced_tags += rhs.synced_tags;
        self.skipped_objects += rhs.skipped_objects;
        self.synced_objects += rhs.synced_objects;
        self.skipped_payloads += rhs.skipped_payloads;
        self.synced_payloads += rhs.synced_payloads;
        self.synced_payload_bytes += rhs.synced_payload_bytes;
    }
}

impl std::iter::Sum<SyncSummary> for SyncSummary {
    fn sum<I: Iterator<Item = SyncSummary>>(iter: I) -> Self {
        iter.fold(Default::default(), |mut cur, next| {
            cur += next;
            cur
        })
    }
}

pub struct SyncEnvResult {
    pub env: tracking::EnvSpec,
    pub results: Vec<SyncEnvItemResult>,
}

impl SyncEnvResult {
    pub fn summary(&self) -> SyncSummary {
        self.results.iter().map(|r| r.summary()).sum()
    }
}

pub enum SyncEnvItemResult {
    Tag(SyncTagResult),
    Object(SyncObjectResult),
}

impl SyncEnvItemResult {
    pub fn summary(&self) -> SyncSummary {
        match self {
            Self::Tag(r) => r.summary(),
            Self::Object(r) => r.summary(),
        }
    }
}

pub enum SyncTagResult {
    /// The tag did not need to be synced
    Skipped,
    /// The tag was synced
    Synced {
        tag: tracking::TagSpec,
        result: SyncObjectResult,
    },
}

impl SyncTagResult {
    pub fn summary(&self) -> SyncSummary {
        match self {
            Self::Skipped => SyncSummary {
                skipped_tags: 1,
                ..Default::default()
            },
            Self::Synced { result, .. } => {
                let mut summary = result.summary();
                summary.synced_tags += 1;
                summary
            }
        }
    }
}

pub enum SyncObjectResult {
    Platform(SyncPlatformResult),
    Layer(SyncLayerResult),
    Blob(SyncBlobResult),
    Manifest(SyncManifestResult),
    Tree(graph::Tree),
    Mask,
}

impl SyncObjectResult {
    pub fn summary(&self) -> SyncSummary {
        use SyncObjectResult::*;
        match self {
            Platform(res) => res.summary(),
            Layer(res) => res.summary(),
            Blob(res) => res.summary(),
            Manifest(res) => res.summary(),
            Mask | Tree(_) => SyncSummary::default(),
        }
    }
}

pub enum SyncPlatformResult {
    /// The platform did not need to be synced
    Skipped,
    /// The platform was at least partially synced
    Synced {
        platform: graph::Platform,
        results: Vec<SyncObjectResult>,
    },
}

impl SyncPlatformResult {
    pub fn summary(&self) -> SyncSummary {
        match self {
            Self::Skipped => SyncSummary::skipped_one_object(),
            Self::Synced { results, .. } => {
                let mut summary = results.iter().map(|r| r.summary()).sum();
                summary += SyncSummary::synced_one_object();
                summary
            }
        }
    }
}

pub enum SyncLayerResult {
    /// The layer did not need to be synced
    Skipped,
    /// The layer was synced
    Synced {
        layer: graph::Layer,
        result: SyncManifestResult,
    },
}

impl SyncLayerResult {
    pub fn summary(&self) -> SyncSummary {
        match self {
            Self::Skipped => SyncSummary::skipped_one_object(),
            Self::Synced { result, .. } => {
                let mut summary = result.summary();
                summary += SyncSummary::synced_one_object();
                summary
            }
        }
    }
}

pub enum SyncManifestResult {
    /// The manifest did not need to be synced
    Skipped,
    /// The manifest was at least partially synced
    Synced {
        manifest: graph::Manifest,
        results: Vec<SyncEntryResult>,
    },
}

impl SyncManifestResult {
    pub fn summary(&self) -> SyncSummary {
        match self {
            Self::Skipped => SyncSummary::skipped_one_object(),
            Self::Synced { results, .. } => {
                let mut summary = results.iter().map(|r| r.summary()).sum();
                summary += SyncSummary::synced_one_object();
                summary
            }
        }
    }
}

pub enum SyncEntryResult {
    /// The entry did not need to be synced
    Skipped,
    /// The entry was synced
    Synced {
        entry: graph::Entry,
        result: SyncBlobResult,
    },
}

impl SyncEntryResult {
    pub fn summary(&self) -> SyncSummary {
        match self {
            Self::Skipped => SyncSummary::skipped_one_object(),
            Self::Synced { result, .. } => {
                let mut summary = result.summary();
                summary += SyncSummary::synced_one_object();
                summary
            }
        }
    }
}

pub enum SyncBlobResult {
    /// The blob did not need to be synced
    Skipped,
    /// The blob was synced
    Synced {
        blob: graph::Blob,
        result: SyncPayloadResult,
    },
}

impl SyncBlobResult {
    pub fn summary(&self) -> SyncSummary {
        match self {
            Self::Skipped => SyncSummary::skipped_one_object(),
            Self::Synced { result, .. } => {
                let mut summary = result.summary();
                summary += SyncSummary::synced_one_object();
                summary
            }
        }
    }
}

pub enum SyncPayloadResult {
    /// The payload did not need to be synced
    Skipped,
    /// The payload was synced
    Synced { size: u64 },
}

impl SyncPayloadResult {
    pub fn summary(&self) -> SyncSummary {
        match self {
            Self::Skipped => SyncSummary {
                skipped_payloads: 1,
                ..Default::default()
            },
            Self::Synced { size } => SyncSummary {
                synced_payloads: 1,
                synced_payload_bytes: *size,
                ..Default::default()
            },
        }
    }
}
