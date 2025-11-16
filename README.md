# Zellij Pane Picker

![GitHub branch check runs](https://img.shields.io/github/check-runs/shihanng/zellij-pane-picker/main)
![GitHub Release](https://img.shields.io/github/v/release/shihanng/zellij-pane-picker)
![GitHub License](https://img.shields.io/github/license/shihanng/zellij-pane-picker)

![Social preview of the project](./assets/social-preview.png)

With [Zellij](https://zellij.dev/) Panes Picker,
you can quickly switch, star, and jump to panes using customizable keyboard shortcuts.

## Installation

Put the following in your
[Zellij configuration](https://zellij.dev/documentation/configuration.html)
`config.kdl`.

<!-- markdownlint-disable MD013 -->

```kdl
keybinds clear-defaults=true {
...
    shared_except "locked" {
        ...
        bind "Alt y" {
            LaunchOrFocusPlugin "zellij-pane-picker" {
                floating true; move_to_focused_tab true;
            }
        }
        ...
    }
...
}

load_plugins {
    zellij-pane-picker
}

plugins {
    ...
    zellij-pane-picker location="https://github.com/shihanng/zellij-pane-picker/releases/download/v0.6.0/zellij-pane-picker.wasm" {
        list_panes ""
        plugin_select_down "Ctrl n"
        plugin_select_up "Ctrl p"
    }
    ...
}
```

<!-- markdownlint-enable MD013 -->

## Usage

In the picker pane, we can

- Select and navigate to a pane
- Filter panes in the list
- Star/unstar a pane

![Select, star, filter panes](./assets/screencast-nav-search-star.gif)

Use `Alt i/u` to cycle between starred panes.
Use `Alt o` to toggle between two panes.

![Toggles](./assets/screencast-navigation.gif)

### Global Keybindings

| Keybinding | Description                                    | Config Key      |
| ---------- | ---------------------------------------------- | --------------- |
| Alt y      | Open plugin pane and lists all available panes | `list_panes`    |
| Alt o      | Toggle between two panes                       | `navigate_back` |
| Alt l      | Star/unstar the focused pane                   | `toggle_star`   |
| Alt i      | Navigate to next starred pane                  | `next_star`     |
| Alt u      | Navigate to previous starred pane              | `previous_star` |

<!-- markdownlint-disable MD013 -->

### Plugin Keybindings

| Keybinding | Description                                   | Config Key                              |
| ---------- | --------------------------------------------- | --------------------------------------- |
| Up/Down    | Move the selection in the list of panes       | `plugin_select_up`/`plugin_select_down` |
| Enter      | Navigate to the selected pane                 | `plugin_navigate_to`                    |
| Esc        | Close the plugin without navigating to a pane | `plugin_hide`                           |
| Space      | Toggle star/unstar the selected pane          | `plugin_toggle_star`                    |

### Customize Keybindings

Use the config key in the plugin configuration to customize the keybindings, e.g.,
the following allows us to use **Alt x** to open the plugin pane and
use **Ctrl p/n** to move the selection in the list of panes.

```kdl
load_plugins {
    "https://github.com/shihanng/zellij-pane-picker/releases/download/v0.6.0/zellij-pane-picker.wasm" {
        list_panes "Alt x"
        plugin_select_down "Ctrl n"
        plugin_select_up "Ctrl p"
    }
}
```

Use empty string to disable a keybinding.

```kdl
load_plugins {
    "https://github.com/shihanng/zellij-pane-picker/releases/download/v0.6.0/zellij-pane-picker.wasm" {
        list_panes ""
    }
}
```

### Configuration Options

#### Show Starred Panes First

Add `show_starred_first "true"` to display starred panes at the top of the picker list:

```kdl
load_plugins {
    "https://github.com/shihanng/zellij-pane-picker/releases/download/v0.6.0/zellij-pane-picker.wasm" {
        show_starred_first "true"
        list_panes "Alt y"
    }
}
```

When enabled, starred panes appear at the top of both the full list (empty search)
and filtered search results. Within each group (starred/unstarred), panes are sorted
by fuzzy match score. This is useful when you frequently access certain panes and want
to eliminate scrolling through the full list.

**Default:** `"false"` (disabled)

<!-- markdownlint-enable MD013 -->

## Development

### Linters and Testing

- Install pre-commit hooks with `pre-commit install`.
- I use [just](https://just.systems/) (command runner) to execute linters
  and run tests but it is optional. See [`justfile`](./justfile) for
  what they actually do.
  - Run linters with `just lint`.
  - Run tests with `just test`.

### (Optional) Run GitHub Actions locally

Use [`act`](https://github.com/nektos/act) to run GitHub Actions locally.

GitHub token in order to avoid hitting the rate limit
when installing the toolings.

```shell
export GITHUB_TOKEN=$(gh auth token)
just run-ci-local
```
