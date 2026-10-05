pub const API: u32 = 2;
pub const MIN_API: u32 = 2;

/// Why a manifest's `api` rules it out; `None` when this Universe runs it.
pub fn unsupported(api: u32) -> Option<String> {
    let reads = if MIN_API == API { API.to_string() } else { format!("{MIN_API} to {API}") };
    match api {
        0 => Some(format!("declares no extension api: this Universe reads api {reads}")),
        a if (MIN_API..=API).contains(&a) => None,
        a => Some(format!("written for extension api {a}: this Universe reads api {reads}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_api_out_of_range_or_absent_is_unsupported() {
        assert_eq!(unsupported(API), None);
        assert!(unsupported(0).is_some_and(|w| w.contains("no extension api")));
        assert!(unsupported(API + 1).is_some_and(|w| w.contains(&format!("api {}", API + 1))));
        assert!(unsupported(MIN_API - 1).is_some());
    }
}
