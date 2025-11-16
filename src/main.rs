mod keybind;
mod star;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};
use std::cmp::min;
use std::collections::HashSet;
use std::convert::TryFrom;
use zellij_tile::prelude::*;

use std::collections::BTreeMap;
use std::collections::HashMap;

#[derive(Debug)]
struct Pane {
    tab_name: String,
    pane_id: PaneId,
    pane_title: String,

    search_string: String,
}

impl Pane {
    fn new(tab_name: String, pane_id: PaneId, pane_title: String) -> Self {
        let just_pane_id = match pane_id {
            PaneId::Terminal(id) => id,
            PaneId::Plugin(id) => id,
        };

        let search_string = format!("{}  {} {}", tab_name, just_pane_id, pane_title);
        Self {
            tab_name,
            pane_id,
            pane_title,
            search_string,
        }
    }
}

impl AsRef<str> for Pane {
    fn as_ref(&self) -> &str {
        &self.search_string
    }
}

#[derive(Default)]
struct State {
    tab_infos: Vec<TabInfo>,
    pane_infos: HashMap<usize, Vec<PaneInfo>>,

    panes: Vec<Pane>,

    current_focus: Option<PaneId>,
    previous_focus: Option<PaneId>,
    search_key: String,
    display_panes: Vec<Pane>,
    selected: usize, // selected always operates on display_panes.

    stars: star::Star,

    bound_key: bool,
    keybinds: keybind::Keybinds,
    show_starred_first: bool,

    plugin_id: Option<u32>,
}

impl State {
    /// Compute the current state of panes that are visible on the plugin list,
    /// currently and previously focus panes.
    /// We have to call this method in both TabUpdate and PaneUpdate event because
    /// the ordering of the events is unditerministic.
    fn update_state(&mut self) {
        let mut panes: Vec<Pane> = Vec::new();
        let mut current_focus = None;

        for (tab_id, tab_info) in self.tab_infos.iter().enumerate() {
            if let Some(pane_infos) = self.pane_infos.get(&tab_id) {
                pane_infos.iter().for_each(|pane_info| {
                    if pane_info.is_plugin && Some(pane_info.id) == self.plugin_id {
                        return;
                    }

                    if pane_info.is_suppressed || !pane_info.is_selectable {
                        return;
                    }

                    let pane_id = if pane_info.is_plugin {
                        PaneId::Plugin(pane_info.id)
                    } else {
                        PaneId::Terminal(pane_info.id)
                    };

                    panes.push(Pane::new(
                        tab_info.name.clone(),
                        pane_id,
                        pane_info.title.clone(),
                    ));

                    if pane_info.is_focused && tab_info.active && !pane_info.is_plugin {
                        current_focus = Some(pane_id)
                    }
                });
            }
        }

        // Convert panes to hashset of paneid
        let pane_ids: HashSet<PaneId> = panes.iter().map(|p| p.pane_id).collect();

        if current_focus.is_some() && current_focus != self.current_focus {
            if self
                .current_focus
                .as_ref()
                .is_some_and(|c| pane_ids.contains(c))
            {
                // If the previous current_focus still exists, use that as the
                // next previous_focus.
                self.previous_focus = self.current_focus;
            } else if self
                .previous_focus
                .as_ref()
                .is_some_and(|c| pane_ids.contains(c))
            {
                // Not replacing the old previous_focus because the old
                // current_focus was not found in the current panes (could be already deleted)
                // and the old previous_focus still exists.
            } else {
                self.previous_focus = None;
            }
            self.current_focus = current_focus;
        }

        self.stars.sync(&pane_ids);
        self.panes = panes;
    }

    fn panes_as_table(&mut self, width: usize) -> Table {
        let star = "*";
        let max_tab_col_length = 12;

        // Calculate the width of tab name column.
        let tab_name_width = min(
            self.panes
                .iter()
                .map(|pane| pane.tab_name.len())
                .max()
                .unwrap_or(max_tab_col_length),
            max_tab_col_length,
        );

        // Calculate the width of pane title column.
        let pane_title_width = width - (star.len() + 1 + tab_name_width + 1 + 3);

        let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
        let mut search_result =
            Pattern::parse(&self.search_key, CaseMatching::Ignore, Normalization::Smart)
                .match_list(&self.panes, &mut matcher);

        // Track selected pane before rebuilding display_panes for selection preservation
        let selected_pane_id = self.display_panes.get(self.selected).map(|p| p.pane_id);

        // Sort by starred status if configured
        if self.show_starred_first {
            search_result.sort_by(|(pane_a, score_a), (pane_b, score_b)| {
                let starred_a = self.stars.has(&pane_a.pane_id);
                let starred_b = self.stars.has(&pane_b.pane_id);

                match (starred_a, starred_b) {
                    (true, false) => std::cmp::Ordering::Less, // A starred → A first
                    (false, true) => std::cmp::Ordering::Greater, // B starred → B first
                    _ => score_b.cmp(score_a), // Same status → sort by score (descending)
                }
            });
        }

        self.display_panes = search_result
            .iter()
            .map(|(pane, _)| {
                Pane::new(pane.tab_name.clone(), pane.pane_id, pane.pane_title.clone())
            })
            .collect();

        // Restore selection to same pane after sorting
        if self.display_panes.is_empty() {
            self.selected = 0;
        } else if let Some(pane_id) = selected_pane_id {
            self.selected = self
                .display_panes
                .iter()
                .position(|p| p.pane_id == pane_id)
                .unwrap_or(0); // Fallback if pane disappeared
        }

        let mut table = Table::new().add_row(vec![
            " ",
            &format!("{:<width$}", "Tab", width = tab_name_width),
            " ID",
            &format!("{:<width$}", "Pane Title", width = pane_title_width),
        ]);

        for (i, pane) in self.display_panes.iter().enumerate() {
            let pane_id = match pane.pane_id {
                PaneId::Terminal(id) => id,
                PaneId::Plugin(id) => id,
            };

            let mut star_column = Text::new(if self.stars.has(&pane.pane_id) {
                star
            } else {
                " "
            })
            .color_range(0, ..);
            let mut tab_name_column = Text::new(clip(&pane.tab_name, tab_name_width));
            let mut pane_id_column = Text::new(format!("{:3}", pane_id));
            let mut pane_title_column = Text::new(clip(&pane.pane_title, pane_title_width));

            if i == self.selected {
                star_column = star_column.selected();
                tab_name_column = tab_name_column.selected();
                pane_id_column = pane_id_column.selected();
                pane_title_column = pane_title_column.selected();
            }

            table = table.add_styled_row(vec![
                star_column,
                tab_name_column,
                pane_id_column,
                pane_title_column,
            ]);
        }

        table
    }

    fn select_downward(&mut self) {
        if !self.display_panes.is_empty() {
            self.selected = (self.selected + 1) % self.display_panes.len();
        }
    }

    fn select_upward(&mut self) {
        if !self.display_panes.is_empty() {
            self.selected =
                (self.selected + self.display_panes.len() - 1) % self.display_panes.len();
        }
    }
}

fn clip(string: &str, max_len: usize) -> String {
    let ellipsis = "...";

    if string.len() > max_len {
        if max_len >= ellipsis.len() {
            format!("{}{}", &string[..(max_len - ellipsis.len())], ellipsis)
        } else {
            // return ellipsis to max_len
            ellipsis.chars().take(max_len).collect::<String>()
        }
    } else {
        string.to_string()
    }
}

#[cfg(not(test))]
register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.show_starred_first = configuration
            .get("show_starred_first")
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);

        self.keybinds = keybind::Keybinds::try_from(configuration).unwrap();
        self.plugin_id = Some(get_plugin_ids().plugin_id);

        request_permission(&[
            PermissionType::ChangeApplicationState,
            PermissionType::ReadApplicationState,
            PermissionType::Reconfigure,
        ]);

        subscribe(&[
            EventType::ModeUpdate,
            EventType::Key,
            EventType::PaneUpdate,
            EventType::TabUpdate,
        ]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::ModeUpdate(mode_info) => {
                if let Some(base_mode) = mode_info.base_mode {
                    if !self.bound_key {
                        if let Some(plugin_id) = self.plugin_id {
                            self.keybinds
                                .bind_global_keys(base_mode, plugin_id, reconfigure);
                        }
                    }
                }
            }
            Event::TabUpdate(tab_infos) => {
                self.tab_infos = tab_infos;
                self.update_state();
            }
            Event::PaneUpdate(PaneManifest { panes }) => {
                self.pane_infos = panes;
                self.update_state();
            }
            Event::Key(key) => {
                if Some(key.clone()) == self.keybinds.plugin_select_down {
                    self.select_downward();
                } else if Some(key.clone()) == self.keybinds.plugin_select_up {
                    self.select_upward()
                } else if Some(key.clone()) == self.keybinds.plugin_navigate_to {
                    focus_pane_with_id(self.display_panes[self.selected].pane_id, true);
                    self.search_key.clear();
                    hide_self();
                } else if Some(key.clone()) == self.keybinds.plugin_hide {
                    self.search_key.clear();
                    hide_self();
                } else if Some(key.clone()) == self.keybinds.plugin_toggle_star {
                    let selected_pane_id = self.display_panes[self.selected].pane_id;
                    self.stars.toggle(selected_pane_id);
                } else if let BareKey::Char(c) = key.bare_key {
                    if key.has_no_modifiers() {
                        self.search_key.push(c);
                    }
                } else if let BareKey::Backspace = key.bare_key {
                    self.search_key.pop();
                }
            }
            _ => {}
        }
        true
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        if pipe_message.source == PipeSource::Keybind && pipe_message.is_private {
            if pipe_message.name == keybind::LIST_PANES {
                show_self(true);
            } else if pipe_message.name == keybind::NAVIGATE_BACK {
                if let Some(id) = self.previous_focus {
                    focus_pane_with_id(id, true);
                }
            } else if pipe_message.name == keybind::TOGGLE_STAR {
                if let Some(pane_id) = self.current_focus {
                    self.stars.toggle(pane_id);
                }
            } else if pipe_message.name == keybind::NEXT_STAR {
                if let Some(pane_id) = self.current_focus {
                    if let Some(id) = self.stars.next(&pane_id) {
                        focus_pane_with_id(*id, true);
                    }
                }
            } else if pipe_message.name == keybind::PREV_STAR {
                if let Some(pane_id) = self.current_focus {
                    if let Some(id) = self.stars.previous(&pane_id) {
                        focus_pane_with_id(*id, true);
                    }
                }
            }
            return true;
        }
        false
    }

    fn render(&mut self, rows: usize, cols: usize) {
        print_text_with_coordinates(
            Text::new(format!("[SEARCH] {}", self.search_key))
                .color_range(1, 0..=8)
                .color_range(3, 9..),
            1,
            1,
            Some(cols - 1),
            Some(1),
        );

        let nested_list = self.panes_as_table(cols - 4);
        print_table_with_coordinates(nested_list, 1, 3, Some(cols - 1), Some(rows - 2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[test]
    fn panes_as_table() {
        let mut state = State {
            tab_infos: vec![
                TabInfo {
                    name: String::from("Tab 1"),
                    ..Default::default()
                },
                TabInfo {
                    name: String::from("Tab 2"),
                    ..Default::default()
                },
            ],
            pane_infos: HashMap::from([
                (
                    // This will be the last because it is the last tab.
                    1,
                    vec![PaneInfo {
                        id: 55,
                        title: String::from("Pane 55"),
                        is_selectable: true,
                        ..Default::default()
                    }],
                ),
                (
                    0,
                    vec![
                        PaneInfo {
                            id: 1,
                            title: String::from("Pane 1"),
                            is_selectable: true,
                            ..Default::default()
                        },
                        PaneInfo {
                            id: 2,
                            title: String::from("Pane 2"),
                            is_selectable: true,
                            ..Default::default()
                        },
                        // Hidden because is not selectable
                        PaneInfo {
                            id: 3,
                            title: String::from("Pane 3 (not selectable)"),
                            ..Default::default()
                        },
                        // Hidden because is suppressed
                        PaneInfo {
                            id: 4,
                            title: String::from("Pane 4 (suppressed)"),
                            is_selectable: true,
                            is_suppressed: true,
                            ..Default::default()
                        },
                        // Hidden because is the plugin
                        PaneInfo {
                            id: 4,
                            title: String::from("Pane 4 (suppressed)"),
                            is_selectable: true,
                            is_plugin: true,
                            ..Default::default()
                        },
                    ],
                ),
                (
                    // The following tab does not exist.
                    2,
                    vec![PaneInfo {
                        id: 99,
                        title: String::from("Pane 99 on non existing tab"),
                        is_selectable: true,
                        ..Default::default()
                    }],
                ),
            ]),
            selected: 1,
            plugin_id: Some(4),
            ..Default::default()
        };

        state.stars.toggle(PaneId::Terminal(2));
        state.update_state();

        insta::assert_snapshot!(format!(
            "\u{1b}Pztable;{}",
            state.panes_as_table(20).serialize()
        ));
    }

    #[test]
    fn select_downward_without_panes() {
        let mut state = State::default();
        state.select_downward();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn select_upward_without_panes() {
        let mut state = State::default();
        state.select_upward();
        assert_eq!(state.selected, 0);
    }

    #[fixture]
    fn display_panes() -> Vec<Pane> {
        vec![
            Pane::new(
                String::from("Tab"),
                PaneId::Terminal(1),
                String::from("Pane 1"),
            ),
            Pane::new(
                String::from("Tab"),
                PaneId::Terminal(2),
                String::from("Pane 2"),
            ),
        ]
    }

    #[rstest]
    fn select_downward(display_panes: Vec<Pane>) {
        let mut state = State {
            display_panes,
            selected: 0,
            ..Default::default()
        };

        state.select_downward();
        assert_eq!(state.selected, 1);
    }

    #[rstest]
    fn select_upward(display_panes: Vec<Pane>) {
        let mut state = State {
            display_panes,
            selected: 1,
            ..Default::default()
        };

        state.select_upward();
        assert_eq!(state.selected, 0);
    }

    #[rstest]
    fn select_downward_overflow(display_panes: Vec<Pane>) {
        let mut state = State {
            display_panes,
            selected: 0,
            ..Default::default()
        };

        state.select_downward();
        state.select_downward();
        assert_eq!(state.selected, 0);
    }

    #[rstest]
    fn select_upward_overflow(display_panes: Vec<Pane>) {
        let mut state = State {
            display_panes,
            selected: 1,
            ..Default::default()
        };

        state.select_upward();
        state.select_upward();
        assert_eq!(state.selected, 1);
    }

    #[fixture]
    fn tab(#[default("Tab")] name: &str) -> TabInfo {
        TabInfo {
            name: String::from(name),
            ..Default::default()
        }
    }

    #[fixture]
    fn active_tab(#[default("Tab")] name: &str) -> TabInfo {
        TabInfo {
            name: String::from(name),
            active: true,
            ..Default::default()
        }
    }

    #[fixture]
    fn pane(#[default(0)] id: u32) -> PaneInfo {
        PaneInfo {
            id,
            is_selectable: true,
            ..Default::default()
        }
    }

    #[fixture]
    fn focus_pane(#[default(0)] id: u32) -> PaneInfo {
        PaneInfo {
            id,
            is_focused: true,
            is_selectable: true,
            ..Default::default()
        }
    }

    #[fixture]
    fn focus_plugin_pane(#[default(0)] id: u32) -> PaneInfo {
        PaneInfo {
            id,
            is_focused: true,
            is_selectable: true,
            is_plugin: true,
            ..Default::default()
        }
    }

    #[fixture]
    fn current_focus() -> PaneId {
        PaneId::Terminal(10)
    }

    #[fixture]
    fn previous_focus() -> PaneId {
        PaneId::Terminal(11)
    }

    #[rstest]
    #[case::no_panes_in_tab(vec![ active_tab("Tab") ], HashMap::from([(1, vec![focus_pane(1)])]))]
    #[case::no_tabs(vec![], HashMap::from([(1, vec![focus_pane(1)])]))]
    #[case::focus_pane_is_plugin(vec![ active_tab("Tab") ], HashMap::from([(0, vec![focus_plugin_pane(1)])]))]
    #[case::focus_pane_is_current(vec![ active_tab("Tab") ], HashMap::from([(0, vec![focus_pane(10)])]))]
    fn no_changes_in_focus(
        #[case] tab_infos: Vec<TabInfo>,
        #[case] pane_infos: HashMap<usize, Vec<PaneInfo>>,
        current_focus: PaneId,
        previous_focus: PaneId,
    ) {
        let mut state = State {
            tab_infos,
            pane_infos,
            current_focus: Some(current_focus),
            previous_focus: Some(previous_focus),
            ..Default::default()
        };

        state.update_state();

        assert_eq!(state.current_focus, Some(current_focus));
        assert_eq!(state.previous_focus, Some(previous_focus));
    }

    #[rstest]
    #[case::current_focus_exists(vec![ active_tab("Tab") ], HashMap::from([(0, vec![focus_pane(1), pane(10)])]), PaneId::Terminal(1), Some(PaneId::Terminal(10)))]
    #[case::current_focus_not_exists_previous_exists(vec![ active_tab("Tab") ], HashMap::from([(0, vec![focus_pane(1), pane(11)])]), PaneId::Terminal(1), Some(PaneId::Terminal(11)))]
    #[case::current_and_previous_focus_not_exists(vec![ active_tab("Tab") ], HashMap::from([(0, vec![focus_pane(1)])]), PaneId::Terminal(1), None)]
    fn change_in_focus(
        #[case] tab_infos: Vec<TabInfo>,
        #[case] pane_infos: HashMap<usize, Vec<PaneInfo>>,
        #[case] new_current_focus: PaneId,
        #[case] new_previous_focus: Option<PaneId>,
    ) {
        let mut state = State {
            tab_infos,
            pane_infos,
            current_focus: Some(PaneId::Terminal(10)),
            previous_focus: Some(PaneId::Terminal(11)),
            ..Default::default()
        };

        state.update_state();

        assert_eq!(state.current_focus, Some(new_current_focus));
        assert_eq!(state.previous_focus, new_previous_focus);
    }

    #[rstest]
    #[case("Lorem ipsum dolor sit amet", 100, "Lorem ipsum dolor sit amet".to_string())]
    #[case("Lorem ipsum dolor sit amet", 5, "Lo...".to_string())]
    #[case("Lorem ipsum dolor sit amet", 2, "..".to_string())]
    #[case("Lorem ipsum dolor sit amet", 0, "".to_string())]
    fn clip_text(#[case] text: &str, #[case] max_len: usize, #[case] expected: String) {
        let got = clip(text, max_len);
        assert_eq!(expected, got);
    }

    // Tests for show_starred_first feature

    #[fixture]
    fn three_pane_state(#[default(false)] show_starred_first: bool) -> State {
        State {
            tab_infos: vec![TabInfo {
                name: String::from("Tab"),
                ..Default::default()
            }],
            pane_infos: HashMap::from([(
                0,
                vec![
                    PaneInfo {
                        id: 1,
                        title: String::from("Pane 1"),
                        is_selectable: true,
                        ..Default::default()
                    },
                    PaneInfo {
                        id: 2,
                        title: String::from("Pane 2"),
                        is_selectable: true,
                        ..Default::default()
                    },
                    PaneInfo {
                        id: 3,
                        title: String::from("Pane 3"),
                        is_selectable: true,
                        ..Default::default()
                    },
                ],
            )]),
            show_starred_first,
            ..Default::default()
        }
    }

    #[rstest]
    #[case::disabled(false, 2, vec![1, 2, 3])]
    #[case::enabled(true, 3, vec![3, 1, 2])]
    fn starred_pane_ordering(
        #[case] show_starred_first: bool,
        #[case] pane_to_star: u32,
        #[case] expected_order: Vec<u32>,
    ) {
        let mut state = three_pane_state(show_starred_first);
        state.stars.toggle(PaneId::Terminal(pane_to_star));
        state.update_state();
        state.panes_as_table(80);

        assert_eq!(state.display_panes.len(), expected_order.len());
        for (i, expected_id) in expected_order.iter().enumerate() {
            assert_eq!(
                state.display_panes[i].pane_id,
                PaneId::Terminal(*expected_id)
            );
        }
    }

    #[test]
    fn starred_first_with_search() {
        let mut state = State {
            tab_infos: vec![TabInfo {
                name: String::from("Tab"),
                ..Default::default()
            }],
            pane_infos: HashMap::from([(
                0,
                vec![
                    PaneInfo {
                        id: 1,
                        title: String::from("vim editor"),
                        is_selectable: true,
                        ..Default::default()
                    },
                    PaneInfo {
                        id: 2,
                        title: String::from("neovim"),
                        is_selectable: true,
                        ..Default::default()
                    },
                    PaneInfo {
                        id: 3,
                        title: String::from("bash"),
                        is_selectable: true,
                        ..Default::default()
                    },
                ],
            )]),
            show_starred_first: true,
            search_key: String::from("vim"),
            ..Default::default()
        };

        // Star the neovim pane (id 2)
        state.stars.toggle(PaneId::Terminal(2));
        state.update_state();
        state.panes_as_table(80);

        // Verify: Starred match appears before unstarred matches
        // Both "vim editor" and "neovim" match "vim", but neovim is starred
        assert_eq!(state.display_panes.len(), 2);
        assert_eq!(state.display_panes[0].pane_id, PaneId::Terminal(2)); // neovim (starred)
        assert_eq!(state.display_panes[1].pane_id, PaneId::Terminal(1)); // vim editor (not starred)
    }

    #[rstest]
    fn selection_preserved_after_toggle(#[with(true)] three_pane_state: State) {
        let mut state = three_pane_state;
        state.update_state();
        state.panes_as_table(80);

        // Select pane 2 (index 1)
        state.selected = 1;
        assert_eq!(
            state.display_panes[state.selected].pane_id,
            PaneId::Terminal(2)
        );

        // Toggle star on selected pane - it should move to index 0
        state.stars.toggle(PaneId::Terminal(2));
        state.panes_as_table(80);

        // Verify: Selection followed the pane to new position
        assert_eq!(
            state.display_panes[state.selected].pane_id,
            PaneId::Terminal(2)
        );
        assert_eq!(state.selected, 0); // Moved to first position
    }

    #[test]
    fn selection_reset_when_display_panes_empty() {
        let mut state = State {
            tab_infos: vec![],
            pane_infos: HashMap::new(),
            show_starred_first: true,
            selected: 5, // Simulate out-of-bounds selection
            ..Default::default()
        };

        state.update_state();
        state.panes_as_table(80);

        // Verify: Selection is reset to 0 when display_panes is empty
        assert_eq!(state.display_panes.len(), 0);
        assert_eq!(state.selected, 0);
    }
}
