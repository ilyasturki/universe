pub mod add_game;
pub mod artwork;
pub mod game_settings;
pub mod onboarding;
pub mod preferences;
pub mod signin;

/// A toast whose title shows as written: it reads markup by default, and titles and errors carry `&` and `<`.
pub fn toast(text: &str) -> adw::Toast {
    adw::Toast::builder().title(text).use_markup(false).build()
}
