//! Embedded icon assets + the gpui [`AssetSource`] that serves them.
//!
//! The set mirrors the icon usage of the Zeron desktop releases:
//! - Most glyphs come from the **Solar Icons** set (Linear weight) by 480 Design,
//!   the same set the Electron app used via `@solar-icons/react`. Solar Icons is
//!   licensed under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/);
//!   attribution: "Solar Icons by 480 Design".
//! - The terminal tab glyphs (`terminal`, `plus`, `close`) and the stop square
//!   are ports of the hand-drawn inline SVGs in Zeron's `terminal-panel.tsx` /
//!   `composer-actions.tsx`.
//! - The harness brand marks are ports of Zeron's `icons.tsx` (no icon set
//!   ships brand icons, so these stay).
//! - Clyra keeps its own logo + wordmark, the three Linux caption glyphs, and
//!   three Clyra-only glyphs (`chart-column`, `diff`, `undo`) from **Lucide**
//!   (ISC, https://lucide.dev, lucide-static 0.456.0).
//!
//! Icons render via [`icon`]: `icon(icons::PLUS).size(px(16.)).text_color(…)`.

use std::borrow::Cow;

use gpui::{AssetSource, Hsla, Result, SharedString, Styled as _, Svg, svg};

macro_rules! icon_assets {
    ($(($const_name:ident, $path:literal)),+ $(,)?) => {
        $(pub const $const_name: &str = concat!("icons/", $path, ".svg");)+

        /// Serves the embedded control icons to gpui's SVG renderer.
        struct ControlAssets;

        impl AssetSource for ControlAssets {
            fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
                Ok(match path {
                    $(concat!("icons/", $path, ".svg") => Some(Cow::Borrowed(
                        include_bytes!(concat!("../assets/icons/", $path, ".svg")).as_slice(),
                    )),)+
                    _ => None,
                })
            }

            fn list(&self, path: &str) -> Result<Vec<SharedString>> {
                let all = [$(concat!("icons/", $path, ".svg")),+];
                Ok(all
                    .iter()
                    .filter(|p| p.starts_with(path))
                    .map(|p| SharedString::from(*p))
                    .collect())
            }
        }
    };
}

icon_assets![
    (MICROPHONE, "microphone"),
    (PROJECT_DEFAULT, "project-default"),
    (REMOTE_SERVER, "remote-server"),
    (FAST_TIER, "fast-tier"),
    (FAST_TIER_BOLD, "fast-tier-bold"),
    (MONITOR, "monitor"),
    (SUN, "sun"),
    (MOON, "moon"),
    (GLOBE, "globe"),
    (LAPTOP, "laptop"),
    (PEN_NEW_SQUARE, "pen-new-square"),
    (SORT, "sort"),
    (MORE_HORIZONTAL, "more-horizontal"),
    (SORT_VERTICAL, "sort-vertical"),
    // Compact six-dot grip used to reorder queued prompts.
    (DRAG_HANDLE, "drag-handle"),
    (QUEUE_DRAG_HANDLE, "queue-drag-handle"),
    (QUEUE_SEND, "queue-send"),
    (QUEUE_CHECK, "queue-check"),
    (QUEUE_CLOSE, "queue-close"),
    (QUEUE_PAPERCLIP, "queue-paperclip"),
    (CLOCK_CIRCLE, "clock-circle"),
    (CALENDAR, "calendar"),
    (LIST, "list"),
    (FOLDER_WITH_FILES, "folder-with-files"),
    (FILE_TREE, "file-tree"),
    (FOLDER, "folder"),
    (FLOPPY_DISK, "floppy-disk"),
    (GIT_BRANCH, "git-branch"),
    // Provider-neutral pull-request glyph.
    (PULL_REQUEST, "pull-request"),
    (CLOUD, "cloud"),
    (TAG, "tag"),
    (SIDEBAR_MINIMALISTIC, "sidebar-minimalistic"),
    // Mirrored variant: the LEFT sidebar toggle shows the panel line on
    // the left; gpui divs have no scale transform at the pinned rev, so
    // the flip stays baked into the asset.
    (SIDEBAR_MINIMALISTIC_LEFT, "sidebar-minimalistic-left"),
    (KEY_MINIMALISTIC, "key-minimalistic"),
    (KEYBOARD, "keyboard"),
    (ARROW_LEFT, "arrow-left"),
    (ARROW_RIGHT, "arrow-right"),
    (ARROW_UP, "arrow-up"),
    (ARROW_DOWN, "arrow-down"),
    (ARROW_UP_RIGHT, "arrow-up-right"),
    (RETURN, "return"),
    (ALT_ARROW_DOWN, "alt-arrow-down"),
    (ALT_ARROW_UP, "alt-arrow-up"),
    (EXPAND_ARROWS, "expand-arrows"),
    // Inward-pointing companion used to restore an expanded pane.
    (COLLAPSE_ARROWS, "collapse-arrows"),
    (FOLD_VERTICAL, "fold-vertical"),
    // The changes pane's unified/split toggle: a rounded frame halved by a
    // centre rule.
    (SPLIT_COLUMNS, "split-columns"),
    // Long-line wrapping toggle shared by changes and agent Markdown fences.
    (WRAP_TEXT, "wrap-text"),
    (ALT_ARROW_LEFT, "alt-arrow-left"),
    (ALT_ARROW_RIGHT, "alt-arrow-right"),
    (SMARTPHONE, "smartphone"),
    (ARCHIVE_UP_MINIMALISTIC, "archive-up-minimalistic"),
    (REFRESH, "refresh"),
    (RESTART, "restart"),
    (UNDO, "undo"),
    (DIFF, "diff"),
    (CHART_COLUMN, "chart-column"),
    (ADD_CIRCLE, "add-circle"),
    (TUNING, "tuning"),
    (EYE, "eye"),
    (EYE_CLOSED, "eye-closed"),
    (PAPERCLIP, "paperclip"),
    (PIN, "pin"),
    (PEN, "pen"),
    (ARCHIVE_MINIMALISTIC, "archive-minimalistic"),
    (TRASH_BIN_MINIMALISTIC, "trash-bin-minimalistic"),
    // Shared settings glyph: horizontal sliders.
    (SETTINGS, "settings"),
    (SETTINGS_MINIMALISTIC, "settings-minimalistic"),
    (LOGOUT_2, "logout-2"),
    (MAGNIFER, "magnifer"),
    (PALETTE_SEARCH, "palette-search"),
    (COMMAND, "command"),
    (DOCUMENT, "document"),
    (DOCUMENT_ADD, "document-add"),
    // File-kind glyphs for transcript badges.
    (FILE_CODE, "file-code"),
    (FILE_STYLE, "file-style"),
    (FILE_DATA, "file-data"),
    (FILE_MARKDOWN, "file-markdown"),
    (FILE_IMAGE, "file-image"),
    (GLOBAL, "global"),
    (CHECKLIST, "checklist"),
    (WIDGET, "widget"),
    (MAGIC_STICK_3, "magic-stick-3"),
    (WIFI_OFF, "wifi-off"),
    (CLOSE_CIRCLE, "close-circle"),
    (INFO_CIRCLE, "info-circle"),
    (DANGER_TRIANGLE, "danger-triangle"),
    (CHAT_ROUND_LINE, "chat-round-line"),
    (BOT, "bot"),
    (BELL, "bell"),
    (VOLUME_LOUD, "volume-loud"),
    (TERMINAL, "terminal"),
    (PLUS, "plus"),
    (CLOSE, "close"),
    // Hand-drawn Linux caption glyphs (minimize dash, maximize square,
    // restore stacked squares) — kept bespoke: Lucide has no window-control
    // set and `copy` misreads as restore at caption sizes.
    (WINDOW_MINIMIZE, "window-minimize"),
    (WINDOW_MAXIMIZE, "window-maximize"),
    (WINDOW_RESTORE, "window-restore"),
    (HARD_DRIVE, "hard-drive"),
    (HOME, "home"),
    (STOP, "stop"),
    (CHECK, "check"),
    (COPY, "copy"),
    // Project Action icon family.
    (ACTION_PLAY, "action-play"),
    (ACTION_TEST, "action-test"),
    (ACTION_LINT, "action-lint"),
    (ACTION_CONFIGURE, "action-configure"),
    (ACTION_BUILD, "action-build"),
    (ACTION_DEBUG, "action-debug"),
    // Star outline for the favorite affordance, solid for the favorited
    // state and the picker's favorites rail tab.
    (STAR, "star"),
    (STAR_BOLD, "star-bold"),
    (CLYRA_LOGO, "clyra-logo"),
    (CLYRA_WORDMARK, "clyra-wordmark"),
    // Harness brand marks (icons.tsx).
    (CLAUDE_MARK, "claude-mark"),
    (OPENAI_MARK, "openai-mark"),
    (CURSOR_MARK, "cursor-mark"),
    (DEVIN_MARK, "devin-mark"),
    (GROK_MARK, "grok-mark"),
    (HERMES_MARK, "hermes-mark"),
    (PI_MARK, "pi-mark"),
    (OPENCODE_MARK, "opencode-mark"),
    (ANTIGRAVITY_MARK, "antigravity-mark"),
];

/// Serves both the compact control-icon set and the complete file-identity
/// icon theme through the single asset source registered at app startup.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(asset) = ControlAssets.load(path)? {
            return Ok(Some(asset));
        }
        crate::file_icons::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets = ControlAssets.list(path)?;
        assets.extend(crate::file_icons::Assets.list(path)?);
        Ok(assets)
    }
}

/// The Claude mark's brand orange (`#D97757`) — clyra keeps it even on the
/// monochrome surface.
pub fn claude_brand() -> Hsla {
    gpui::rgb(0xD97757).into()
}

/// An icon element for an embedded asset path. Size and colour are set by the
/// caller (`.size(..)`, `.text_color(..)`), matching the web app's
/// `[&_svg]:size-4` idiom.
pub fn icon(path: &'static str) -> Svg {
    svg().path(path).flex_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_icon_loads_and_parses() {
        let assets = Assets;
        for path in assets.list("icons/").unwrap() {
            let bytes = assets
                .load(&path)
                .unwrap()
                .unwrap_or_else(|| panic!("missing asset {path}"));
            let text = std::str::from_utf8(&bytes).expect("icon svg is utf-8");
            assert!(text.contains("<svg"), "{path} is not an svg");
            assert!(text.contains("viewBox"), "{path} lacks a viewBox");
        }
    }

    #[test]
    fn unknown_paths_are_none() {
        assert!(Assets.load("icons/nope.svg").unwrap().is_none());
    }

    #[test]
    fn list_filters_by_prefix() {
        assert!(!Assets.list("icons/").unwrap().is_empty());
        assert!(Assets.list("fonts/").unwrap().is_empty());
    }
}
