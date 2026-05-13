//! Initial config-file content.

const DEFAULT_TOML: &str = "\
# Superpanels configuration file. See for the full schema.
#
# Defaults are sane; uncomment and edit blocks as needed.

[general]
# default_profile = \"home\"
autostart        = false
notifications    = \"errors\"   # off | errors | all
theme            = \"auto\"     # auto | light | dark

[backend]
prefer           = \"auto\"     # auto | kde | gnome | sway | hyprland | feh | custom
custom_command   = \"\"

[library]
roots            = []
recursive        = true
thumbnail_size   = 320
auto_scan        = true

# Per-monitor physical sizes. The detector gives us pixels; this gives us
# millimetres. Add one block per monitor; match priority is stable_id, then
# name. Use `superpanels monitor configure` to generate these blocks.
#
# [[monitor]]
# stable_id = \"f7f0f124-9e9b-4ef0-91a7-426d58091760\"
# name = \"DP-1\"
# physical_mm = [597, 336]
";

pub(super) fn default_document() -> String {
    DEFAULT_TOML.to_owned()
}
