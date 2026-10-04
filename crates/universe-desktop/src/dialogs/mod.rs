pub mod add_game;
pub mod artwork;
pub mod controller;
pub mod game_settings;
pub mod library_artwork;
pub mod onboarding;
pub mod preferences;
pub mod signin;
pub mod system_check;
pub mod whats_new;

/// A toast whose title shows as written: it reads markup by default, and titles and errors carry `&` and `<`.
pub fn toast(text: &str) -> adw::Toast {
    adw::Toast::builder().use_markup(false).title(text).build()
}
