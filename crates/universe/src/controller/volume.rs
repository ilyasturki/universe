//! Volume macros as PulseAudio calls on the default sink: nothing is typed, so no key reaches the game.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use libpulse_binding::callbacks::ListResult;
use libpulse_binding::context::{Context, FlagSet, State};
use libpulse_binding::mainloop::standard::Mainloop;
use libpulse_binding::operation::{Operation, State as OpState};
use libpulse_binding::time::MicroSeconds;
use libpulse_binding::volume::{ChannelVolumes, Volume};

pub const NORMAL: u32 = Volume::NORMAL.0;
const DEADLINE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Up,
    Down,
    ToggleMute,
}

/// One channel moved by `percent` of the normal volume and kept within [0, normal]: a sink boosted above
/// it comes back down to normal on either step.
pub fn step(volume: u32, percent: u8, up: bool) -> u32 {
    let delta = (u64::from(NORMAL) * u64::from(percent) / 100) as u32;
    if up { volume.saturating_add(delta) } else { volume.saturating_sub(delta) }.min(NORMAL)
}

fn pump(ml: &mut Mainloop, since: Instant) -> Result<(), String> {
    if since.elapsed() > DEADLINE {
        return Err("pulseaudio did not answer in time".into());
    }
    ml.prepare(Some(MicroSeconds(200_000))).map_err(|e| format!("mainloop: {e}"))?;
    ml.poll().map_err(|e| format!("mainloop: {e}"))?;
    ml.dispatch().map_err(|e| format!("mainloop: {e}"))?;
    Ok(())
}

fn wait<T: ?Sized>(ml: &mut Mainloop, op: &Operation<T>, since: Instant) -> Result<(), String> {
    while op.get_state() == OpState::Running {
        pump(ml, since)?;
    }
    if op.get_state() == OpState::Cancelled {
        return Err("operation cancelled".into());
    }
    Ok(())
}

/// Blocking: connects to the default server, applies the change to the default sink, disconnects.
pub fn apply(change: Change, percent: u8) -> Result<(), String> {
    let since = Instant::now();
    let mut ml = Mainloop::new().ok_or("mainloop")?;
    let mut ctx = Context::new(&ml, "universe").ok_or("context")?;
    ctx.connect(None, FlagSet::NOAUTOSPAWN, None).map_err(|e| format!("connect: {e}"))?;
    loop {
        match ctx.get_state() {
            State::Ready => break,
            State::Failed | State::Terminated => return Err(format!("no pulseaudio server: {}", ctx.errno())),
            _ => pump(&mut ml, since)?,
        }
    }
    let sink: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let op = {
        let sink = sink.clone();
        ctx.introspect().get_server_info(move |info| *sink.borrow_mut() = info.default_sink_name.as_ref().map(|s| s.to_string()))
    };
    wait(&mut ml, &op, since)?;
    let name = sink.borrow().clone().ok_or("no default sink")?;
    let found: Rc<RefCell<Option<(ChannelVolumes, bool)>>> = Rc::new(RefCell::new(None));
    let op = {
        let found = found.clone();
        ctx.introspect().get_sink_info_by_name(&name, move |r| {
            if let ListResult::Item(i) = r {
                *found.borrow_mut() = Some((i.volume, i.mute));
            }
        })
    };
    wait(&mut ml, &op, since)?;
    let (mut volume, mute) = found.borrow().ok_or_else(|| format!("sink {name} not found"))?;
    let done: Rc<RefCell<Option<bool>>> = Rc::new(RefCell::new(None));
    let cb: Box<dyn FnMut(bool)> = {
        let done = done.clone();
        Box::new(move |ok| *done.borrow_mut() = Some(ok))
    };
    let op = match change {
        Change::ToggleMute => ctx.introspect().set_sink_mute_by_name(&name, !mute, Some(cb)),
        Change::Up | Change::Down => {
            for v in volume.get_mut() {
                v.0 = step(v.0, percent, change == Change::Up);
            }
            ctx.introspect().set_sink_volume_by_name(&name, &volume, Some(cb))
        }
    };
    wait(&mut ml, &op, since)?;
    let ok = *done.borrow();
    ctx.disconnect();
    match ok {
        Some(true) => Ok(()),
        _ => Err(format!("{name}: {change:?} refused")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_by_percent_within_the_normal_range() {
        let two = NORMAL / 50;
        assert_eq!(step(NORMAL / 2, 2, true), NORMAL / 2 + two);
        assert_eq!(step(NORMAL / 2, 2, false), NORMAL / 2 - two);
        assert_eq!(step(NORMAL - 10, 2, true), NORMAL, "clamped at normal");
        assert_eq!(step(NORMAL + 20_000, 2, true), NORMAL, "a boosted sink drops back to normal");
        assert_eq!(step(NORMAL + 20_000, 2, false), NORMAL);
        assert_eq!(step(100, 6, false), 0, "clamped at silence");
        assert_eq!(step(0, 100, true), NORMAL);
        assert_eq!(step(NORMAL / 2, 0, true), NORMAL / 2);
    }

    // Needs a PulseAudio server: `cargo test -- --ignored mute_round_trip`. Mutes, then unmutes: the sink ends as it began.
    #[test]
    #[ignore]
    fn mute_round_trip() {
        apply(Change::ToggleMute, 2).unwrap();
        apply(Change::ToggleMute, 2).unwrap();
    }
}
