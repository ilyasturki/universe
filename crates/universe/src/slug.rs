use unicode_normalization::UnicodeNormalization;

/// Port of sanitizeGameName (game-session.mjs): the key that already names the recording and journal folders.
pub fn slug(name: &str) -> String {
    if name.is_empty() {
        return "unknown".into();
    }
    let lowered = name.to_lowercase();
    let stripped: String = lowered
        .nfkd()
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .filter(|c| *c != '\'' && *c != '\u{2019}')
        .collect();
    let mut out = String::with_capacity(stripped.len());
    let mut last_dash = false;
    for c in stripped.chars() {
        let keep = c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
        let ch = if keep { c } else { '-' };
        if ch == '-' {
            if !last_dash {
                out.push('-');
            }
            last_dash = true;
        } else {
            out.push(ch);
            last_dash = false;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "unknown".into()
    } else {
        trimmed
    }
}

/// Obsidian-safe note name (gameNoteName): keeps accents and apostrophes, drops / \ : # ^ [ ] |.
pub fn note_name(title: &str) -> String {
    let replaced: String = title
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '#' | '^' | '[' | ']' | '|' => ' ',
            c => c,
        })
        .collect();
    let squeezed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    if squeezed.is_empty() {
        "Journal".into()
    } else {
        squeezed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_game_session_mjs() {
        assert_eq!(slug("The Technomancer"), "the-technomancer");
        assert_eq!(slug("Assassin's Creed Odyssey"), "assassins-creed-odyssey");
        assert_eq!(slug("Pokémon HeartGold"), "pokemon-heartgold");
        assert_eq!(slug("Resident Evil 4"), "resident-evil-4");
        assert_eq!(slug("Mount & Blade II: Bannerlord"), "mount-blade-ii-bannerlord");
        assert_eq!(slug("KINGDOM HEARTS HD 1.5+2.5 ReMIX (All Games)"), "kingdom-hearts-hd-1-5-2-5-remix-all-games");
        assert_eq!(slug(""), "unknown");
        assert_eq!(slug("’’"), "unknown");
    }

    #[test]
    fn note_names() {
        assert_eq!(note_name("Pokémon: HeartGold"), "Pokémon HeartGold");
        assert_eq!(note_name("  "), "Journal");
    }
}
