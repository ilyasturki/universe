use adw::prelude::*;

// A builder's `use_markup(false)` reaches the labels after its title and subtitle: text with `&` or `<` fails to parse.

/// The row with its title and subtitle shown as written.
pub fn plain(row: adw::ActionRow, title: impl AsRef<str>, subtitle: impl AsRef<str>) -> adw::ActionRow {
    row.set_use_markup(false);
    row.set_title(title.as_ref());
    row.set_subtitle(subtitle.as_ref());
    row
}

/// The expander with its title and subtitle shown as written.
pub fn plain_expander(row: adw::ExpanderRow, title: impl AsRef<str>, subtitle: impl AsRef<str>) -> adw::ExpanderRow {
    row.set_use_markup(false);
    row.set_title(title.as_ref());
    row.set_subtitle(subtitle.as_ref());
    row
}
