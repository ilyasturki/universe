use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use crate::app::Application;
use crate::media::Kind;
use crate::widgets::MediaGrid;

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/ilyasturki/UniverseDesktop/ui/media-page.ui")]
    pub struct MediaPage {
        #[template_child]
        pub sidebar_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub kinds: TemplateChild<adw::ToggleGroup>,
        #[template_child]
        pub menu_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub grid: TemplateChild<MediaGrid>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MediaPage {
        const NAME: &'static str = "UniverseMediaPage";
        type Type = super::MediaPage;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            MediaGrid::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MediaPage {
        fn constructed(&self) {
            self.parent_constructed();
            self.menu_button.set_menu_model(Some(&crate::menus::main(None)));
            let grid = self.grid.get();
            self.kinds.connect_active_name_notify(move |kinds| {
                grid.set_kinds(match kinds.active_name().as_deref() {
                    Some("shots") => &[Kind::Shot],
                    Some("recordings") => &[Kind::Recording],
                    Some("journal") => &[Kind::Journal],
                    _ => &[Kind::Shot, Kind::Recording, Kind::Journal],
                });
            });
        }
    }

    impl WidgetImpl for MediaPage {}
    impl BinImpl for MediaPage {}
}

glib::wrapper! {
    /// Every game's shots, recordings and journal entries, newest first, one kind or all.
    pub struct MediaPage(ObjectSubclass<imp::MediaPage>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl MediaPage {
    pub fn sidebar_button(&self) -> gtk::Button {
        self.imp().sidebar_button.get()
    }

    pub fn follow(&self, app: &Application) {
        self.imp().grid.follow(app, "");
    }
}

/// A game's screenshots or recordings on a page of their own.
pub fn open_game(win: &crate::window::Window, game: &str, kind: Kind) {
    let grid = MediaGrid::default();
    grid.follow(&win.app(), game);
    grid.set_kinds(&[kind]);
    let title = match kind {
        Kind::Shot => gettextrs::gettext("Screenshots"),
        Kind::Recording => gettextrs::gettext("Recordings"),
        Kind::Journal => gettextrs::gettext("Journal"),
    };
    let toolbar = adw::ToolbarView::builder().content(&grid).build();
    toolbar.add_top_bar(&crate::pages::game_header(win, &title, game));
    let page = adw::NavigationPage::builder().child(&toolbar).title(title).tag(format!("media:{game}:{kind:?}")).build();
    win.push_page(&page);
}
