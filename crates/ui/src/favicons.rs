//! Shared website favicons for transcript web rows (Perplexity-style site
//! identity): one process-wide cache keyed by domain, fetched once in the
//! background, letter avatars until (or unless) the icon lands.
//!
//! The cache is a gpui [`Global`] so the main transcript, subagent transcripts
//! and every window share downloads. Entries are tiny PNGs; failures stick as
//! failures for the process lifetime so offline stays fast.
//!
//! Icons come from Google's favicon service (reliable PNGs at any size, stock
//! globe for unknown domains). That leaks visited domains to Google — the
//! same trade the in-app browser already makes fetching the pages themselves.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{App, Global, Image};

/// Favicon PNG URL for a domain. `sz=64` keeps entries small; rows render
/// them at 16px.
fn favicon_url(domain: &str) -> String {
    format!("https://www.google.com/s2/favicons?domain={domain}&sz=64")
}

const FAVICON_MAX_ENTRIES: usize = 256;
const FAVICON_MAX_BYTES: usize = 128 * 1024;
const FAVICON_TIMEOUT_SECS: u64 = 5;
/// A stuck download (dropped view, cancelled task) must not poison its
/// domain forever: Loading older than this is claimable again.
const FAVICON_STALE_SECS: u64 = 30;

#[derive(Clone)]
enum FaviconEntry {
    Loading { since: Instant },
    Ready(Arc<Image>),
    Failed,
}

#[derive(Default)]
pub struct FaviconCache {
    entries: HashMap<String, FaviconEntry>,
}

impl Global for FaviconCache {}

impl FaviconCache {
    /// Ready image for a domain, if already downloaded.
    pub fn get(cx: &App, domain: &str) -> Option<Arc<Image>> {
        match cx.try_global::<Self>()?.entries.get(domain)? {
            FaviconEntry::Ready(image) => Some(image.clone()),
            _ => None,
        }
    }

    /// Mark a domain as loading. Returns true when the caller must start the
    /// download — exactly once per domain per process (dedupe across views).
    /// Stops admitting new domains past the cap; letter avatars cover those.
    /// Mark a domain as loading. Returns true when the caller must start the
    /// download — exactly once per domain per process (dedupe across views),
    /// plus a retry when a previous flight went stale (dropped owner,
    /// cancelled task). Stops admitting new domains past the cap; letter
    /// avatars cover those.
    pub fn claim(cx: &mut App, domain: &str) -> bool {
        if cx.try_global::<Self>().is_none() {
            cx.set_global(Self::default());
        }
        let cache = cx.global_mut::<Self>();
        match cache.entries.get(domain) {
            None => {}
            Some(FaviconEntry::Loading { since })
                if since.elapsed() > Duration::from_secs(FAVICON_STALE_SECS) => {}
            _ => return false,
        }
        if cache.entries.len() >= FAVICON_MAX_ENTRIES && !cache.entries.contains_key(domain) {
            return false;
        }
        cache.entries.insert(
            domain.to_string(),
            FaviconEntry::Loading {
                since: Instant::now(),
            },
        );
        true
    }

    /// Settle a download. Returns true when views should repaint (an image
    /// landed; failures keep the letter avatars already on screen).
    pub fn finish(cx: &mut App, domain: &str, image: Option<Arc<Image>>) -> bool {
        if cx.try_global::<Self>().is_none() {
            return false;
        }
        let ready = image.is_some();
        cx.global_mut::<Self>().entries.insert(
            domain.to_string(),
            match image {
                Some(image) => FaviconEntry::Ready(image),
                None => FaviconEntry::Failed,
            },
        );
        ready
    }
}

/// Download + decode one favicon. Pure future: run it on the tokio runtime
/// (`gpui_tokio::Tokio::spawn`) — the gpui executor has no reactor for
/// reqwest. Mirrors the browser surface's favicon fetch (timeout, redirect
/// cap, byte cap, thumbnail decode).
pub async fn download_favicon(domain: String) -> Option<Arc<Image>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(FAVICON_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .ok()?;
    let response = client
        .get(favicon_url(&domain))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    if response
        .content_length()
        .is_some_and(|n| n > FAVICON_MAX_BYTES as u64)
    {
        return None;
    }
    let bytes = response.bytes().await.ok()?.to_vec();
    if bytes.len() > FAVICON_MAX_BYTES {
        return None;
    }
    let media = crate::image_media::decode_raster_image(bytes, FAVICON_MAX_BYTES).ok()?;
    Some(media.image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favicon_urls_pin_domain_and_size() {
        assert_eq!(
            favicon_url("example.com"),
            "https://www.google.com/s2/favicons?domain=example.com&sz=64"
        );
    }
}
