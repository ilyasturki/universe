use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::{gettext, ngettext};

use crate::dialogs::preferences::ListPage;
use crate::game::Row;
use crate::window::Window;

/// What a listed game's row offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    Update,
    /// Once the player says it is, `mark_current` takes the installed build as upstream's newest.
    Mark,
    ByHand,
}

/// The games not hidden that upstream is ahead of, each with its offer, then the ones Universe can't check that a mark
/// would give a baseline; both by title.
pub fn listing(rows: &[Row]) -> (Vec<(&Row, Offer)>, Vec<&Row>) {
    let mut shown: Vec<&Row> = rows.iter().filter(|r| !r.hidden).collect();
    shown.sort_by_cached_key(|r| crate::library::fold(&r.title));
    let behind = shown.iter().filter(|r| r.check.status == "update").map(|r| (*r, offer(r))).collect();
    let unsure = shown.into_iter().filter(|r| r.check.status == "unknown" && r.check.markable).collect();
    (behind, unsure)
}

fn offer(row: &Row) -> Offer {
    if row.updatable() {
        Offer::Update
    } else if row.markable() {
        Offer::Mark
    } else {
        Offer::ByHand
    }
}

struct Updates {
    list: Rc<ListPage>,
    /// Kept through a rebuild as the player left it.
    unsure_open: Cell<bool>,
}

/// Game Defaults' Game Updates, filled again on every library change while it lives.
pub fn page(win: &Window) -> adw::NavigationPage {
    let title = gettext("Game Updates");
    let updates = Rc::new(Updates { list: ListPage::new("game-updates", &title, ""), unsure_open: Cell::new(false) });
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&updates.list.page));
    let page = adw::NavigationPage::new(&toolbar, &title);
    fill(&updates, win);
    let (weak, weak_win) = (Rc::downgrade(&updates), win.downgrade());
    win.app().library().connect_updated(move || {
        if let (Some(updates), Some(win)) = (weak.upgrade(), weak_win.upgrade()) {
            fill(&updates, &win);
        }
    });
    page.connect_hidden(move |_| {
        let _ = &updates;
    });
    page
}

fn fill(updates: &Rc<Updates>, win: &Window) {
    let list = &updates.list;
    let focus = crate::components::take_focus(&list.rows.borrow());
    list.clear();
    let rows: Vec<Row> = win.app().library().games().iter().map(|g| g.row().clone()).collect();
    let (behind, unsure) = listing(&rows);
    let count = ngettext("{} game", "{} games", behind.len() as u32).replace("{}", &behind.len().to_string());
    let group = list.group(&gettext("Updates Available"), &if behind.is_empty() { gettext("No update available") } else { count });
    for (row, offer) in behind {
        let shown = game_row(win, row, offer);
        list.keep(&row.id, &shown);
        group.add(&shown);
    }
    if !unsure.is_empty() {
        let expander =
            crate::rows::plain_expander(adw::ExpanderRow::new(), gettext("Games Universe Can't Check"), gettext("No version to compare with upstream's"));
        expander.set_expanded(updates.unsure_open.get());
        let weak = Rc::downgrade(updates);
        expander.connect_expanded_notify(move |expander| {
            weak.upgrade().inspect(|u| u.unsure_open.set(expander.is_expanded()));
        });
        list.keep("unsure", &expander);
        for row in unsure {
            let shown = game_row(win, row, Offer::Mark);
            list.keep(&row.id, &shown);
            expander.add_row(&shown);
        }
        list.group("", "").add(&expander);
    }
    crate::components::refocus(&list.rows.borrow(), focus);
}

fn game_row(win: &Window, row: &Row, offer: Offer) -> adw::ActionRow {
    let shown = crate::rows::plain(adw::ActionRow::new(), &row.title, &row.installed_version);
    let button = |label: String| gtk::Button::builder().label(label).use_underline(true).valign(gtk::Align::Center).build();
    let (id, weak) = (row.id.clone(), win.downgrade());
    match offer {
        Offer::Update => {
            let update = button(gettext("_Update"));
            update.connect_clicked(move |_| {
                let Some(win) = weak.upgrade() else { return };
                if let Some(game) = win.app().library().get(&id) {
                    win.game_action("update", &game);
                }
            });
            shown.add_suffix(&update);
        }
        Offer::Mark => {
            let mark = button(gettext("I Have the _Latest Version"));
            let row = row.clone();
            mark.connect_clicked(move |mark| {
                if let Some(win) = weak.upgrade() {
                    confirm_mark(&win, &row, mark);
                }
            });
            shown.add_suffix(&mark);
        }
        Offer::ByHand => {
            shown.add_suffix(&gtk::Label::builder().label(gettext("Update it by hand")).valign(gtk::Align::Center).css_classes(["dimmed"]).build());
        }
    }
    shown
}

fn confirm_mark(win: &Window, row: &Row, over: &impl IsA<gtk::Widget>) {
    let mut body = vec![if row.installed_version.is_empty() {
        gettext("Your files don't say which version they are.")
    } else {
        gettext("Your files say {}.").replace("{}", &row.installed_version)
    }];
    if !row.check.latest_date.is_empty() {
        let upstream = if row.channel == "switch" { "titledb" } else { "Steam" };
        let day = crate::format::day(&row.check.latest_date);
        body.push(gettext("{}'s newest build came out on {}.").replacen("{}", upstream, 1).replacen("{}", &day, 1));
    }
    let dialog = adw::AlertDialog::new(Some(&gettext("Is {} on Its Latest Version?").replace("{}", &row.title)), Some(&body.join(" ")));
    dialog.add_responses(&[("cancel", &gettext("_Not Sure")), ("mark", &gettext("_It's the Latest"))]);
    dialog.set_response_appearance("mark", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("mark"));
    dialog.set_close_response("cancel");
    let (id, weak) = (row.id.clone(), win.downgrade());
    dialog.connect_response(Some("mark"), move |_, _| {
        let Some(win) = weak.upgrade() else { return };
        if let Some(game) = win.app().library().get(&id) {
            win.mark_current(&game);
        }
    });
    dialog.present(Some(over));
}

#[cfg(test)]
mod tests {
    use universe::upstream::Check;

    use super::*;

    fn game(title: &str, hidden: bool, check: Check) -> Row {
        let mut r = universe::library::Resolved { game: universe::game::Game::new(title), check, ..Default::default() };
        r.game.hidden = hidden;
        Row::of(&r, std::slice::from_ref(&r), &universe::config::Config::default())
    }

    #[test]
    fn game_updates_lists_the_games_behind_by_title_and_the_ones_it_cant_check_apart() {
        let store = |status, how, reason| Check { status, how, reason, store: "gog".into(), store_id: "1".into(), ..Default::default() };
        let steam = |status, markable| Check { status, how: "manual", markable, ..Default::default() };
        let rows = [
            game("Zelda", false, steam("update", true)),
            game("ábzû", false, store("update", "apply", "")),
            game("Hades", false, store("update", "manual", "not-owned")),
            game("Hidden", true, steam("update", true)),
            game("Celeste", false, steam("current", true)),
            game("Doom", false, steam("unknown", true)),
            game("Fez", false, steam("unknown", false)),
            game("Hollow", true, steam("unknown", true)),
            game("Tetris", false, steam("untracked", false)),
        ];
        let (behind, unsure) = listing(&rows);
        let behind: Vec<(&str, Offer)> = behind.iter().map(|(r, o)| (r.id.as_str(), *o)).collect();
        assert_eq!(behind, [("abzu", Offer::Update), ("hades", Offer::ByHand), ("zelda", Offer::Mark)], "accents fold before they sort");
        assert_eq!(unsure.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["doom"], "only a game a mark would give a baseline");
    }
}
