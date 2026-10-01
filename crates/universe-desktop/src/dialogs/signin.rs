use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};
use serde_json::Value;

use crate::backend;
use crate::qr;

pub type Say = Rc<dyn Fn(&str)>;

fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
}

pub fn status(source: &Value) -> String {
    let missing: Vec<String> = source["missing"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
    if source["available"].as_bool() == Some(false) {
        return gettext("Missing {}").replace("{}", &missing.join(", "));
    }
    if source["enabled"].as_bool() != Some(true) {
        return gettext("Off");
    }
    match (source["logged_in"].as_bool() == Some(true), text(source, "user")) {
        (true, user) if !user.is_empty() => gettext("Signed in as {}").replace("{}", &user),
        (true, _) => gettext("Signed in"),
        (false, _) => gettext("Not signed in"),
    }
}

/// A store's sign-in: its link, to open here or scan from a phone, then the code the store shows after, or the API key
/// it makes, as the source's `login` says. `say` puts a line in front of the player; `signed_in` hears the account's name.
pub fn account_group(source: &Value, say: Say, signed_in: Say) -> adw::PreferencesGroup {
    let id = text(source, "id");
    let key = source["login"]["kind"] == "key";
    let group = adw::PreferencesGroup::builder().title(gettext("Account")).build();
    let state = crate::rows::plain(adw::ActionRow::builder().build(), status(source), "");
    state.add_prefix(&gtk::Image::from_icon_name("avatar-default-symbolic"));
    group.add(&state);
    let get_link = adw::ButtonRow::builder()
        .title(if key { gettext("Get an API Key") } else { gettext("Get a Sign-In Link") })
        .start_icon_name("web-browser-symbolic")
        .build();
    group.add(&get_link);
    let hint =
        Some(text(&source["login"], "hint")).filter(|h| !h.is_empty()).unwrap_or_else(|| gettext("Open the link, sign in, then enter the code it shows"));
    let link = crate::rows::plain(adw::ActionRow::builder().visible(false).build(), hint, "");
    let open = gtk::Button::builder().label(gettext("Open")).valign(gtk::Align::Center).build();
    link.add_suffix(&open);
    group.add(&link);
    let qr = gtk::Picture::builder().can_shrink(false).halign(gtk::Align::Center).margin_top(12).margin_bottom(12).visible(false).build();
    qr.set_tooltip_text(Some(&gettext("Scan it to sign in from a phone")));
    group.add(&qr);
    let code = adw::EntryRow::builder().title(if key { gettext("API Key") } else { gettext("Code") }).show_apply_button(true).visible(false).build();
    group.add(&code);

    let url = Rc::new(RefCell::new(String::new()));
    let (sid, say_link) = (id.clone(), say.clone());
    let (link_row, qr_pic, code_row, url_cell) = (link.clone(), qr.clone(), code.clone(), url.clone());
    get_link.connect_activated(move |row| {
        row.set_sensitive(false);
        let (sid, row, say) = (sid.clone(), row.clone(), say_link.clone());
        let (link_row, qr_pic, code_row, url_cell) = (link_row.clone(), qr_pic.clone(), code_row.clone(), url_cell.clone());
        glib::spawn_future_local(async move {
            let result = backend::pinned(move |core| async move { core.source_login_url(&sid).await }).await;
            row.set_sensitive(true);
            match result {
                Ok(url) => {
                    link_row.set_subtitle(&url);
                    link_row.set_visible(true);
                    qr_pic.set_paintable(qr::texture(&url, 4).as_ref());
                    qr_pic.set_visible(true);
                    code_row.set_visible(true);
                    url_cell.replace(url);
                }
                Err(e) => say(&e.to_string()),
            }
        });
    });
    open.connect_clicked(move |button| {
        let launcher = gtk::UriLauncher::new(&url.borrow());
        launcher.launch(button.root().and_downcast::<gtk::Window>().as_ref(), gio::Cancellable::NONE, |_| {});
    });
    code.connect_apply(move |row| {
        let (entered, sid) = (row.text().trim().to_string(), id.clone());
        if entered.is_empty() {
            return;
        }
        row.set_sensitive(false);
        let (row, state, say, signed_in) = (row.clone(), state.clone(), say.clone(), signed_in.clone());
        glib::spawn_future_local(async move {
            let result = backend::pinned(move |core| async move { core.source_login(&sid, &entered).await }).await;
            row.set_sensitive(true);
            match result {
                Ok(user) => {
                    let title = if user.is_empty() { gettext("Signed in") } else { gettext("Signed in as {}").replace("{}", &user) };
                    state.set_title(&title);
                    say(&title);
                    signed_in(&user);
                }
                Err(e) => say(&e.to_string()),
            }
        });
    });
    group
}
