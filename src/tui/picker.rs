//! A filterable list (provider picker, model picker, sessions, command palette ...).
//! Pure state machine: it knows nothing about the terminal. The renderer calls `window(height)`
//! and draws the rows; the key handler maps real key events to `PKey`.

use super::fuzzy::{filter_sort, Match};

#[derive(Clone, Debug)]
pub struct Item<T: Clone> {
    pub label: String,
    pub detail: String, // dim text shown to the right (price, date, keybind ...)
    pub group: Option<String>, // header shown while the query is empty
    pub current: bool,  // marks the active choice (a check mark in the UI)
    pub value: T,
}

impl<T: Clone> Item<T> {
    pub fn new(label: impl Into<String>, value: T) -> Self {
        Item {
            label: label.into(),
            detail: String::new(),
            group: None,
            current: false,
            value,
        }
    }
    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = d.into();
        self
    }
    pub fn group(mut self, g: impl Into<String>) -> Self {
        self.group = Some(g.into());
        self
    }
    pub fn current(mut self, c: bool) -> Self {
        self.current = c;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PKey {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Char(char),
    Backspace,
    ClearQuery, // Ctrl+U
    Enter,
    Esc,
}

#[derive(Debug, PartialEq)]
pub enum PickerEvent<T> {
    None,
    Selected(T),
    Cancel,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    Header(String),
    Item { index: usize, positions: Vec<usize> }, // index into Picker::items; positions = matched chars in the label
}

#[derive(Clone, Debug)]
pub struct Picker<T: Clone> {
    pub title: String,
    pub items: Vec<Item<T>>,
    pub query: String,
    pub status: Option<String>, // e.g. "Loading models..." or an error line, drawn by the renderer
    pub page: usize,            // rows per page for PageUp/PageDown (renderer sets it)
    cursor: usize,              // index into `matches`
    scroll: usize,              // first visible row
    matches: Vec<(usize, Match)>,
}

impl<T: Clone> Picker<T> {
    pub fn new(title: impl Into<String>, items: Vec<Item<T>>) -> Self {
        let mut p = Picker {
            title: title.into(),
            items,
            query: String::new(),
            status: None,
            page: 8,
            cursor: 0,
            scroll: 0,
            matches: vec![],
        };
        p.refilter(true);
        p
    }

    /// Replace the items (for example when an async model list arrives). Keeps the query.
    pub fn set_items(&mut self, items: Vec<Item<T>>) {
        self.items = items;
        self.refilter(self.query.is_empty());
    }

    fn refilter(&mut self, jump_to_current: bool) {
        self.matches = filter_sort(&self.query, &self.items, |it| it.label.clone());
        self.cursor = 0;
        if jump_to_current {
            if let Some(pos) = self
                .matches
                .iter()
                .position(|(i, _)| self.items[*i].current)
            {
                self.cursor = pos;
            }
        }
        self.scroll = 0;
    }

    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// Index into `items` of the highlighted row.
    pub fn selected_index(&self) -> Option<usize> {
        self.matches.get(self.cursor).map(|(index, _)| *index)
    }

    pub fn selected_item(&self) -> Option<&Item<T>> {
        self.matches.get(self.cursor).map(|(i, _)| &self.items[*i])
    }

    /// 1-based position of the selection and the number of matches, for "3/120".
    pub fn position(&self) -> (usize, usize) {
        if self.matches.is_empty() {
            (0, 0)
        } else {
            (self.cursor + 1, self.matches.len())
        }
    }

    pub fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        let grouped = self.query.is_empty() && self.items.iter().any(|i| i.group.is_some());
        let mut last_group: Option<String> = None;
        for (idx, m) in &self.matches {
            if grouped {
                let g = self.items[*idx].group.clone();
                if g != last_group {
                    if let Some(name) = &g {
                        rows.push(Row::Header(name.clone()));
                    }
                    last_group = g;
                }
            }
            rows.push(Row::Item {
                index: *idx,
                positions: m.positions.clone(),
            });
        }
        rows
    }

    /// The rows to draw in a box `height` rows tall; keeps the selected row visible.
    pub fn window(&mut self, height: usize) -> Vec<Row> {
        let rows = self.rows();
        if height == 0 || rows.is_empty() {
            return vec![];
        }
        let sel_item = self.matches.get(self.cursor).map(|(i, _)| *i);
        let sel_row = rows
            .iter()
            .position(|r| matches!(r, Row::Item { index, .. } if Some(*index) == sel_item))
            .unwrap_or(0);
        // show the group header together with the first item of the group
        let want_top = if sel_row > 0 && matches!(rows[sel_row - 1], Row::Header(_)) {
            sel_row - 1
        } else {
            sel_row
        };
        if want_top < self.scroll {
            self.scroll = want_top;
        }
        if sel_row >= self.scroll + height {
            self.scroll = sel_row + 1 - height;
        }
        if self.scroll + height > rows.len() {
            self.scroll = rows.len().saturating_sub(height);
        }
        rows[self.scroll..(self.scroll + height).min(rows.len())].to_vec()
    }

    pub fn handle_key(&mut self, key: PKey) -> PickerEvent<T> {
        let n = self.matches.len();
        match key {
            PKey::Up => {
                if n > 0 {
                    self.cursor = if self.cursor == 0 {
                        n - 1
                    } else {
                        self.cursor - 1
                    };
                }
            }
            PKey::Down => {
                if n > 0 {
                    self.cursor = (self.cursor + 1) % n;
                }
            }
            PKey::PageUp => self.cursor = self.cursor.saturating_sub(self.page.max(1)),
            PKey::PageDown => {
                if n > 0 {
                    self.cursor = (self.cursor + self.page.max(1)).min(n - 1);
                }
            }
            PKey::Home => self.cursor = 0,
            PKey::End => self.cursor = n.saturating_sub(1),
            PKey::Char(c) => {
                self.query.push(c);
                self.refilter(false);
            }
            PKey::Backspace => {
                self.query.pop();
                self.refilter(self.query.is_empty());
            }
            PKey::ClearQuery => {
                self.query.clear();
                self.refilter(true);
            }
            PKey::Enter => {
                if let Some(it) = self.selected_item() {
                    return PickerEvent::Selected(it.value.clone());
                }
            }
            PKey::Esc => {
                if !self.query.is_empty() {
                    self.query.clear();
                    self.refilter(true);
                } else {
                    return PickerEvent::Cancel;
                }
            }
        }
        PickerEvent::None
    }
}

#[cfg(test)]
#[path = "picker_tests.rs"]
mod tests;
