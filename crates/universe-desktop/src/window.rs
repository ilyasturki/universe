use std::cell::RefCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::app::Application;
use crate::script;
use crate::state::State;

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/ilyasturki/UniverseDesktop/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub toasts: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub navigation: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub split_view: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub sidebar: TemplateChild<adw::Sidebar>,
        #[template_child]
        pub content_page: TemplateChild<adw::NavigationPage>,
        #[template_child]
        pub stack: TemplateChild<adw::ViewStack>,
        #[template_child]
        pub failed_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub search: TemplateChild<gtk::SearchEntry>,
        pub state: RefCell<State>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "UniverseWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            let state = State::load();
            obj.set_default_size(state.width, state.height);
            obj.set_maximized(state.maximized);
            self.state.replace(state);
            obj.setup_actions();
        }
    }

    impl WidgetImpl for Window {}

    impl WindowImpl for Window {
        fn close_request(&self) -> glib::Propagation {
            let obj = self.obj();
            if !obj.app().scripted() {
                obj.save_state();
            }
            self.parent_close_request()
        }
    }

    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    pub fn new(app: &Application) -> Self {
        let win: Self = glib::Object::builder().property("application", app).build();
        win.follow_core();
        win
    }

    fn setup_actions(&self) {
        let show_sidebar = gio::ActionEntry::builder("show-sidebar")
            .activate(|win: &Self, _, _| {
                let split = &win.imp().split_view;
                split.set_show_sidebar(!split.shows_sidebar());
            })
            .build();
        let search = gio::ActionEntry::builder("search")
            .activate(|win: &Self, _, _| {
                win.imp().search.grab_focus();
            })
            .build();
        self.add_action_entries([show_sidebar, search]);
    }

    fn save_state(&self) {
        let imp = self.imp();
        let mut state = imp.state.borrow_mut();
        state.maximized = self.is_maximized();
        if !state.maximized {
            let (width, height) = self.default_size();
            state.width = width;
            state.height = height;
        }
        state.save();
    }

    pub fn toast(&self, title: &str) {
        self.imp().toasts.add_toast(adw::Toast::new(title));
    }

    pub fn app(&self) -> Application {
        self.application().and_downcast::<Application>().expect("a Universe window belongs to the Universe application")
    }

    fn follow_core(&self) {
        let app = self.app();
        if app.scripted() {
            self.set_maximized(false);
            self.set_default_size(1280, 800);
        }
        if app.is_ready() {
            self.core_ready();
        } else if let Some(failure) = app.failure() {
            self.core_failed(&failure);
        } else {
            let win = self.downgrade();
            app.connect_local("core-ready", false, move |_| {
                win.upgrade().inspect(|win| win.core_ready());
                None
            });
            let win = self.downgrade();
            app.connect_local("core-failed", false, move |args| {
                let failure: String = args[1].get().unwrap_or_default();
                win.upgrade().inspect(|win| win.core_failed(&failure));
                None
            });
        }
    }

    fn core_ready(&self) {
        self.imp().stack.set_visible_child_name("library");
        if let Some(steps) = self.app().take_script() {
            glib::spawn_future_local(script::run(self.clone().upcast(), steps));
        }
    }

    fn core_failed(&self, failure: &str) {
        self.imp().failed_page.set_description(Some(&glib::markup_escape_text(failure)));
        self.imp().stack.set_visible_child_name("failed");
        if let Some(steps) = self.app().take_script() {
            glib::spawn_future_local(script::run(self.clone().upcast(), steps));
        }
    }
}
