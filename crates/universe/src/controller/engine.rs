//! The press / hold / repeat state machine, in milliseconds from any clock so it tests without one.

use std::collections::BTreeMap;

use super::{preset, Macro};

pub const REPEAT_DELAY_MS: u64 = 400;
pub const REPEAT_INTERVAL_MS: u64 = 100;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Binding {
    pub press: Option<Macro>,
    pub hold: Option<Macro>,
}

impl Binding {
    pub fn of(macros: &[Macro]) -> Binding {
        Binding { press: macros.iter().find(|m| m.trigger == "press").cloned(), hold: macros.iter().find(|m| m.trigger == "hold").cloned() }
    }
    pub fn is_empty(&self) -> bool {
        self.press.is_none() && self.hold.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fire {
    pub device: String,
    pub slot: String,
    pub trigger: &'static str,
    pub action: Macro,
}

fn fire(key: &(String, String), trigger: &'static str, action: &Macro) -> Fire {
    Fire { device: key.0.clone(), slot: key.1.clone(), trigger, action: action.clone() }
}

#[derive(Debug)]
struct Held {
    since: u64,
    hold_fired: bool,
    repeat_at: Option<u64>,
    binding: Binding,
}

#[derive(Debug, Default)]
pub struct Engine {
    pub hold_ms: u64,
    held: BTreeMap<(String, String), Held>,
}

impl Engine {
    pub fn new(hold_ms: u64) -> Engine {
        Engine { hold_ms, held: BTreeMap::new() }
    }

    /// A slot with only a press macro fires at once; with a hold macro too, press waits for the release.
    pub fn press(&mut self, device: &str, slot: &str, binding: Binding, now: u64) -> Vec<Fire> {
        let key = (device.to_string(), slot.to_string());
        if self.held.contains_key(&key) || binding.is_empty() {
            return vec![];
        }
        let mut out = vec![];
        let mut repeat_at = None;
        if binding.hold.is_none() {
            if let Some(m) = &binding.press {
                out.push(fire(&key, "press", m));
                if preset(&m.action).map(|p| p.repeats).unwrap_or(false) {
                    repeat_at = Some(now + REPEAT_DELAY_MS);
                }
            }
        }
        self.held.insert(key, Held { since: now, hold_fired: false, repeat_at, binding });
        out
    }

    pub fn release(&mut self, device: &str, slot: &str, now: u64) -> Vec<Fire> {
        let key = (device.to_string(), slot.to_string());
        let Some(h) = self.held.remove(&key) else { return vec![] };
        let mut out = vec![];
        if h.binding.hold.is_some() && !h.hold_fired && now.saturating_sub(h.since) < self.hold_ms {
            if let Some(m) = &h.binding.press {
                out.push(fire(&key, "press", m));
            }
        }
        out
    }

    pub fn tick(&mut self, now: u64) -> Vec<Fire> {
        let mut out = vec![];
        for (key, h) in self.held.iter_mut() {
            if let (Some(m), false) = (&h.binding.hold, h.hold_fired) {
                if now.saturating_sub(h.since) >= self.hold_ms {
                    h.hold_fired = true;
                    out.push(fire(key, "hold", m));
                }
            }
            if let (Some(at), Some(m)) = (h.repeat_at, &h.binding.press) {
                if now >= at {
                    h.repeat_at = Some(at.max(now.saturating_sub(REPEAT_INTERVAL_MS)) + REPEAT_INTERVAL_MS);
                    out.push(fire(key, "repeat", m));
                }
            }
        }
        out
    }

    /// When the next tick is due, if anything is held.
    pub fn deadline(&self) -> Option<u64> {
        self.held
            .values()
            .flat_map(|h| {
                let hold = if h.binding.hold.is_some() && !h.hold_fired { Some(h.since + self.hold_ms) } else { None };
                [hold, h.repeat_at]
            })
            .flatten()
            .min()
    }

    pub fn forget_device(&mut self, device: &str) {
        self.held.retain(|(d, _), _| d != device);
    }

    pub fn clear(&mut self) {
        self.held.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(trigger: &str, action: &str) -> Macro {
        Macro { family: "x".into(), button: "b".into(), trigger: trigger.into(), action: action.into(), ..Macro::default() }
    }

    fn actions(f: &[Fire]) -> Vec<(&'static str, String)> {
        f.iter().map(|x| (x.trigger, x.action.action.clone())).collect()
    }

    #[test]
    fn press_alone_fires_at_once_and_repeats_when_the_preset_does() {
        let mut e = Engine::new(600);
        let b = Binding::of(&[m("press", "volume_up")]);
        assert_eq!(actions(&e.press("d", "b", b, 1000)), vec![("press", "volume_up".to_string())]);
        assert_eq!(e.deadline(), Some(1400));
        assert!(e.tick(1300).is_empty());
        assert_eq!(actions(&e.tick(1400)), vec![("repeat", "volume_up".to_string())]);
        assert_eq!(actions(&e.tick(1450)), vec![]);
        assert_eq!(actions(&e.tick(1500)), vec![("repeat", "volume_up".to_string())]);
        assert!(e.release("d", "b", 1520).is_empty());
        assert_eq!(e.deadline(), None);
        assert!(e.tick(2000).is_empty(), "nothing repeats after the release");
    }

    #[test]
    fn a_non_repeating_press_fires_once() {
        let mut e = Engine::new(600);
        assert_eq!(actions(&e.press("d", "b", Binding::of(&[m("press", "screenshot")]), 0)), vec![("press", "screenshot".to_string())]);
        assert!(e.tick(5000).is_empty());
        assert!(e.press("d", "b", Binding::of(&[m("press", "screenshot")]), 10).is_empty(), "a repeated press event is ignored while held");
    }

    #[test]
    fn hold_fires_at_the_threshold_and_press_on_an_early_release() {
        let mut e = Engine::new(600);
        let b = Binding::of(&[m("press", "volume_up"), m("hold", "stop")]);
        assert!(e.press("d", "b", b.clone(), 0).is_empty(), "press waits for the release when a hold shares the slot");
        assert_eq!(e.deadline(), Some(600));
        assert_eq!(actions(&e.release("d", "b", 200)), vec![("press", "volume_up".to_string())]);

        assert!(e.press("d", "b", b, 1000).is_empty());
        assert!(e.tick(1500).is_empty());
        assert_eq!(actions(&e.tick(1600)), vec![("hold", "stop".to_string())]);
        assert!(e.tick(1700).is_empty(), "a hold fires once");
        assert!(e.release("d", "b", 1800).is_empty(), "no press after the hold fired");
        assert!(e.tick(3000).is_empty(), "no repeat on a slot that carries a hold");
    }

    #[test]
    fn hold_only_slot() {
        let mut e = Engine::new(600);
        assert!(e.press("d", "b", Binding::of(&[m("hold", "stop")]), 0).is_empty());
        assert!(e.release("d", "b", 100).is_empty());
        e.press("d", "b", Binding::of(&[m("hold", "stop")]), 200);
        assert_eq!(actions(&e.tick(800)), vec![("hold", "stop".to_string())]);
    }

    #[test]
    fn devices_are_independent_and_forgettable() {
        let mut e = Engine::new(600);
        e.press("a", "b", Binding::of(&[m("press", "volume_up")]), 0);
        e.press("c", "b", Binding::of(&[m("press", "volume_down")]), 0);
        e.forget_device("a");
        let fires = e.tick(400);
        assert_eq!(actions(&fires), vec![("repeat", "volume_down".to_string())]);
        assert_eq!((fires[0].device.as_str(), fires[0].slot.as_str()), ("c", "b"));
    }
}
