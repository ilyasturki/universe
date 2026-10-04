pub mod achievements;
mod game;
pub mod game_data;
pub mod journal;
mod library;
pub mod media;
pub mod player;
pub mod sessions;
pub mod storage;
mod store;
pub mod viewer;

pub use game::GamePage;
pub use library::LibraryPage;
pub use media::MediaPage;
pub use store::StorePage;

pub fn game_header(win: &crate::window::Window, title: &str, game: &str) -> adw::HeaderBar {
    let name = win.app().library().get(game).map(|g| g.title()).unwrap_or_default();
    adw::HeaderBar::builder().title_widget(&adw::WindowTitle::new(title, &name)).build()
}
