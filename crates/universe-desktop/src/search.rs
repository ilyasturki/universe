use std::collections::HashMap;

use adw::prelude::*;
use gtk::{gio, glib};

use crate::app::Application;
use crate::config;
use crate::format;
use crate::game::{removal_key, Row};
use crate::library;

const PATH: &str = "/io/github/ilyasturki/UniverseDesktop/SearchProvider";
const INTERFACE: &str = "org.gnome.Shell.SearchProvider2";
const XML: &str = r#"<node>
  <interface name="org.gnome.Shell.SearchProvider2">
    <method name="GetInitialResultSet">
      <arg type="as" name="terms" direction="in"/>
      <arg type="as" name="results" direction="out"/>
    </method>
    <method name="GetSubsearchResultSet">
      <arg type="as" name="previous_results" direction="in"/>
      <arg type="as" name="terms" direction="in"/>
      <arg type="as" name="results" direction="out"/>
    </method>
    <method name="GetResultMetas">
      <arg type="as" name="identifiers" direction="in"/>
      <arg type="aa{sv}" name="metas" direction="out"/>
    </method>
    <method name="ActivateResult">
      <arg type="s" name="identifier" direction="in"/>
      <arg type="as" name="terms" direction="in"/>
      <arg type="u" name="timestamp" direction="in"/>
    </method>
    <method name="LaunchSearch">
      <arg type="as" name="terms" direction="in"/>
      <arg type="u" name="timestamp" direction="in"/>
    </method>
  </interface>
</node>"#;

/// GNOME Shell's search in the app's games: a result opens the window and starts the game from it.
pub fn register(app: &Application, connection: &gio::DBusConnection) -> Result<gio::RegistrationId, glib::Error> {
    let node = gio::DBusNodeInfo::for_xml(XML)?;
    let interface = node.lookup_interface(INTERFACE).expect("the XML declares it");
    let app = app.downgrade();
    connection
        .register_object(PATH, &interface)
        .method_call(move |_, _, _, _, method, params, invocation| {
            let (app, method) = (app.clone(), method.to_string());
            invocation.return_future_local(async move {
                let app = app.upgrade().ok_or_else(|| glib::Error::new(gio::IOErrorEnum::Closed, "Universe Desktop is quitting"))?;
                answer(&app, &method, &params).await
            });
        })
        .build()
}

async fn answer(app: &Application, method: &str, params: &glib::Variant) -> Result<Option<glib::Variant>, glib::Error> {
    let _held = app.hold();
    app.opened().await;
    let invalid = || glib::Error::new(gio::DBusError::InvalidArgs, &format!("{method} takes no {}", params.type_()));
    match method {
        "GetInitialResultSet" => {
            let (terms,) = params.get::<(Vec<String>,)>().ok_or_else(invalid)?;
            Ok(Some(found(app, &terms, None).to_variant()))
        }
        "GetSubsearchResultSet" => {
            let (previous, terms) = params.get::<(Vec<String>, Vec<String>)>().ok_or_else(invalid)?;
            Ok(Some(found(app, &terms, Some(&previous)).to_variant()))
        }
        "GetResultMetas" => {
            let (ids,) = params.get::<(Vec<String>,)>().ok_or_else(invalid)?;
            Ok(Some(ids.iter().filter_map(|id| meta(app, id)).collect::<Vec<_>>().to_variant()))
        }
        "ActivateResult" => {
            let (id, _, _) = params.get::<(String, Vec<String>, u32)>().ok_or_else(invalid)?;
            app.play_from_search(&id);
            Ok(None)
        }
        "LaunchSearch" => {
            let (terms, _) = params.get::<(Vec<String>, u32)>().ok_or_else(invalid)?;
            app.search_from_shell(&terms.join(" "));
            Ok(None)
        }
        _ => Err(glib::Error::new(gio::DBusError::UnknownMethod, &format!("no method {method}"))),
    }
}

/// The games shown in the library that the terms find, `within` the ones found before when the shell narrows a search.
fn found(app: &Application, terms: &[String], within: Option<&[String]>) -> Vec<String> {
    let rows: Vec<Row> = app
        .library()
        .games()
        .iter()
        .map(|g| g.row().clone())
        .filter(|r| !r.hidden && !app.is_deferred(&removal_key(&r.id)) && within.is_none_or(|w| w.contains(&r.id)))
        .collect();
    library::rank(&rows, &library::search_words(&terms.join(" ")))
}

fn meta(app: &Application, id: &str) -> Option<HashMap<String, glib::Variant>> {
    let game = app.library().get(id)?;
    let row = game.row();
    let about: Vec<String> = [library::platform_name(&row.platform), format::played(row.hours)].into_iter().filter(|s| !s.is_empty()).collect();
    let icon: gio::Icon = match crate::covers::on_disk(&row.cover) {
        Some(file) => gio::FileIcon::new(&gio::File::for_path(file)).upcast(),
        None => gio::ThemedIcon::new(config::APP_ID).upcast(),
    };
    let mut meta = HashMap::from([
        ("id".to_string(), id.to_variant()),
        ("name".to_string(), row.title.to_variant()),
        ("description".to_string(), about.join(" · ").to_variant()),
    ]);
    if let Some(icon) = icon.serialize() {
        meta.insert("icon".into(), icon);
    }
    Some(meta)
}
