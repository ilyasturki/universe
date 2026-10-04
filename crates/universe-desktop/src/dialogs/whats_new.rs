use adw::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use universe::changelog::Release;

use crate::pages::journal::markup;

fn date(release: &Release) -> String {
    chrono::NaiveDate::parse_from_str(&release.date, "%Y-%m-%d").map(|d| d.format("%-d %B %Y").to_string()).unwrap_or_default()
}

fn label(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::builder().label(text).xalign(0.0).wrap(true).build();
    if !class.is_empty() {
        label.add_css_class(class);
    }
    label
}

fn release_box(release: &Release) -> gtk::Box {
    let column = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(6).build();
    column.append(&label(&release.version, "title-2"));
    column.append(&label(&date(release), "dim-label"));
    for section in &release.sections {
        let title = label(&section.title, "heading");
        title.set_margin_top(12);
        column.append(&title);
        for item in &section.items {
            let entry = label("", "body");
            entry.set_hexpand(true);
            match markup(item) {
                Some(text) => entry.set_markup(&text),
                None => entry.set_text(item),
            }
            let bullet = label("•", "body");
            bullet.set_valign(gtk::Align::Start);
            let line = gtk::Box::builder().spacing(8).build();
            line.append(&bullet);
            line.append(&entry);
            column.append(&line);
        }
    }
    column
}

pub fn present(parent: &impl IsA<gtk::Widget>, releases: &[Release]) {
    let notes =
        gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(36).margin_top(12).margin_bottom(24).margin_start(12).margin_end(12).build();
    for release in releases {
        notes.append(&release_box(release));
    }
    let clamp = adw::Clamp::builder().maximum_size(640).child(&notes).build();
    let scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&clamp).build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&scroll));
    let dialog = adw::Dialog::builder().title(gettext("What's New")).content_width(640).content_height(720).child(&view).build();
    dialog.present(Some(parent));
}

fn appstream_text(text: &str) -> String {
    let mut out = String::new();
    for (n, part) in text.split('`').enumerate() {
        let part = glib::markup_escape_text(part);
        if n % 2 == 1 {
            out.push_str(&format!("<code>{part}</code>"));
        } else {
            out.push_str(&part);
        }
    }
    out
}

/// The changelog from `version` down as the AppStream markup the About dialog's What's New reads: the dialog heads it with
/// `version`, each older release gets a line of its own, each section is a paragraph over its list.
pub fn about_notes(releases: &[Release], version: &str) -> String {
    let mut out = String::new();
    for (n, release) in releases.iter().skip_while(|r| r.version != version).enumerate() {
        if n > 0 {
            let head = gettext("Version {}").replace("{}", &release.version);
            out.push_str(&format!("<p><em>{}</em></p>", appstream_text(&format!("{head} · {}", date(release)))));
        }
        for section in &release.sections {
            out.push_str(&format!("<p>{}</p><ul>", appstream_text(&section.title)));
            for item in &section.items {
                out.push_str(&format!("<li>{}</li>", appstream_text(item)));
            }
            out.push_str("</ul>");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use universe::changelog::Section;

    #[test]
    fn the_about_notes_run_from_this_version_down_in_appstream_markup() {
        let release = |version: &str, item: &str| Release {
            version: version.into(),
            date: "2026-09-30".into(),
            sections: vec![Section { title: "Fixed".into(), items: vec![item.into()] }],
        };
        let releases = [release("0.1.0", "Not out yet."), release("0.0.9", "`universe doctor` & <friends>"), release("0.0.8", "Older.")];
        assert_eq!(
            about_notes(&releases, "0.0.9"),
            "<p>Fixed</p><ul><li><code>universe doctor</code> &amp; &lt;friends&gt;</li></ul>\
             <p><em>Version 0.0.8 · 30 September 2026</em></p><p>Fixed</p><ul><li>Older.</li></ul>"
        );
        assert_eq!(about_notes(&releases, "0.0.7"), "", "a build missing from the changelog shows none");
    }
}
