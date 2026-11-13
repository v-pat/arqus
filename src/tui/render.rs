//! Per-block render cache. Streaming re-renders only the block that changed.

use ratatui::text::Line;

use super::app::{App, CacheKey};
use super::widgets::transcript;

#[derive(Debug, Clone)]
pub struct BlockCache {
    pub key: Option<CacheKey>,
    pub lines: Vec<Line<'static>>,
}

impl BlockCache {
    pub(super) fn empty() -> BlockCache {
        BlockCache {
            key: None,
            lines: Vec::new(),
        }
    }
}

impl App {
    /// Re-render only the blocks whose cache key changed, then flatten.
    pub fn refresh_render(&mut self) {
        let width = if self.wrap_width == 0 {
            80
        } else {
            self.wrap_width
        };
        let theme = self.theme.clone();
        let expanded = self.expanded;
        let show_thinking = self.show_thinking;
        if self.cache.len() != self.blocks.len() {
            self.cache.resize_with(self.blocks.len(), BlockCache::empty);
        }
        for (block, cache) in self.blocks.iter().zip(self.cache.iter_mut()) {
            let key = CacheKey {
                width: self.wrap_width,
                content: block.content_len(),
                expanded,
                show_thinking,
            };
            if cache.key != Some(key) {
                cache.lines =
                    transcript::render_block(block, width, &theme, expanded, show_thinking);
                cache.key = Some(key);
            }
        }
        self.wrapped = self
            .cache
            .iter()
            .flat_map(|cache| cache.lines.iter().cloned())
            .collect();
    }

    /// `Ctrl+O`: expand or collapse every tool output.
    pub fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
        self.refresh_render();
        self.follow_bottom();
    }

    /// `Ctrl+T`: show or hide thinking text.
    pub fn toggle_thinking(&mut self) {
        self.show_thinking = !self.show_thinking;
        self.refresh_render();
        self.follow_bottom();
    }

    /// Block and line inside it at the top of the viewport, for a resize.
    pub fn top_anchor(&self) -> Option<(usize, usize)> {
        let mut start = 0;
        for (index, cache) in self.cache.iter().enumerate() {
            let next = start + cache.lines.len();
            if self.scroll < next {
                return Some((index, self.scroll - start));
            }
            start = next;
        }
        None
    }

    /// Put the same block line back at the top after a re-wrap.
    pub fn scroll_to_anchor(&mut self, anchor: (usize, usize)) {
        let (block, inside) = anchor;
        let mut start = 0;
        for (index, cache) in self.cache.iter().enumerate() {
            if index == block {
                self.follow = false;
                self.scroll = start + inside.min(cache.lines.len().saturating_sub(1));
                self.scroll = self.scroll.min(self.bottom_scroll());
                return;
            }
            start += cache.lines.len();
        }
        self.follow_bottom();
    }
}
