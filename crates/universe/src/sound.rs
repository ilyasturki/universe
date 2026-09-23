//! Playback outputs — every output route a card can play, through a profile switch if need be, and the sinks no route
//! stands for — read from one `pw-dump`; picking one goes through `wpctl` and leaves it WirePlumber's default.

use std::process::Command;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    pub id: String,
    pub label: String,
    pub device: String,
    pub current: bool,
}

#[derive(Debug, Default)]
pub struct Graph {
    sinks: Vec<Sink>,
    cards: Vec<Card>,
    default: String,
}

#[derive(Debug)]
struct Sink {
    id: u64,
    name: String,
    description: String,
    card: Option<u64>,
    profile_device: Option<u64>,
}

#[derive(Debug)]
struct Card {
    id: u64,
    name: String,
    description: String,
    profile: Option<u64>,
    profiles: Vec<Profile>,
    routes: Vec<Route>,
    /// `(route index, profile device)` of each route in use.
    active: Vec<(u64, u64)>,
}

#[derive(Debug)]
struct Profile {
    index: u64,
    name: String,
    priority: i64,
    available: bool,
}

#[derive(Debug)]
struct Route {
    index: u64,
    name: String,
    description: String,
    available: bool,
    devices: Vec<u64>,
    profiles: Vec<u64>,
}

pub(crate) fn wpctl(args: &[&str]) -> Result<String, String> {
    let out = Command::new("wpctl").args(args).output().map_err(|e| format!("wpctl: {e}"))?;
    if !out.status.success() {
        return Err(format!("wpctl {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn number(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str()?.parse().ok())
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_owned()
}

fn numbers(v: &Value) -> Vec<u64> {
    v.as_array().map(|a| a.iter().filter_map(number).collect()).unwrap_or_default()
}

// "unknown" is most jacks without detection: only "no" says nothing is plugged in.
fn available(v: &Value) -> bool {
    v != "no"
}

fn params<'a>(o: &'a Value, name: &str) -> impl Iterator<Item = &'a Value> {
    o.pointer(&format!("/info/params/{name}")).and_then(Value::as_array).into_iter().flatten()
}

fn input_part(profile: &str) -> Option<&str> {
    profile.split('+').find(|p| p.starts_with("input:"))
}

impl Graph {
    pub fn read() -> Result<Graph, String> {
        let out = Command::new("pw-dump").output().map_err(|e| format!("pw-dump: {e}"))?;
        if !out.status.success() {
            return Err(format!("pw-dump: {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        Graph::parse(&String::from_utf8_lossy(&out.stdout))
    }

    pub fn parse(dump: &str) -> Result<Graph, String> {
        let objects: Vec<Value> = serde_json::from_str(dump).map_err(|e| format!("pw-dump: {e}"))?;
        let mut graph = Graph::default();
        for o in &objects {
            let props = &o["info"]["props"];
            match o["type"].as_str().unwrap_or_default() {
                "PipeWire:Interface:Node" if props["media.class"] == "Audio/Sink" => graph.sinks.push(Sink {
                    id: number(&o["id"]).unwrap_or_default(),
                    name: text(&props["node.name"]),
                    description: text(&props["node.description"]),
                    card: number(&props["device.id"]),
                    profile_device: number(&props["card.profile.device"]),
                }),
                "PipeWire:Interface:Device" if props["media.class"] == "Audio/Device" => graph.cards.push(Card {
                    id: number(&o["id"]).unwrap_or_default(),
                    name: text(&props["device.name"]),
                    description: text(&props["device.description"]),
                    profile: params(o, "Profile").find_map(|p| number(&p["index"])),
                    profiles: params(o, "EnumProfile")
                        .filter_map(|p| {
                            Some(Profile {
                                index: number(&p["index"])?,
                                name: text(&p["name"]),
                                priority: p["priority"].as_i64().unwrap_or_default(),
                                available: available(&p["available"]),
                            })
                        })
                        .collect(),
                    routes: params(o, "EnumRoute")
                        .filter(|r| r["direction"] == "Output")
                        .filter_map(|r| {
                            Some(Route {
                                index: number(&r["index"])?,
                                name: text(&r["name"]),
                                description: text(&r["description"]),
                                available: available(&r["available"]),
                                devices: numbers(&r["devices"]),
                                profiles: numbers(&r["profiles"]),
                            })
                        })
                        .collect(),
                    active: params(o, "Route")
                        .filter(|r| r["direction"] == "Output")
                        .filter_map(|r| Some((number(&r["index"])?, number(&r["device"])?)))
                        .collect(),
                }),
                "PipeWire:Interface:Metadata" if o["props"]["metadata.name"] == "default" => {
                    // The effective default; `default.configured.audio.sink` can name a sink of a profile no longer in use.
                    let value = o["metadata"].as_array().into_iter().flatten().find(|m| m["key"] == "default.audio.sink").map(|m| &m["value"]);
                    graph.default = match value {
                        Some(Value::String(json)) => serde_json::from_str::<Value>(json).map(|v| text(&v["name"])).unwrap_or_default(),
                        Some(v) => text(&v["name"]),
                        None => String::new(),
                    };
                }
                _ => {}
            }
        }
        Ok(graph)
    }

    fn card(&self, id: Option<u64>) -> Option<&Card> {
        self.cards.iter().find(|c| Some(c.id) == id)
    }

    fn default_sink(&self) -> Option<&Sink> {
        self.sinks.iter().find(|s| !self.default.is_empty() && s.name == self.default)
    }

    /// The route the sink plays through; none on a profile without routes (pro-audio) or a sink of no card.
    fn route_of(&self, sink: &Sink) -> Option<(&Card, &Route)> {
        let card = self.card(sink.card)?;
        let pd = sink.profile_device?;
        let (index, _) = card.active.iter().find(|(_, d)| *d == pd)?;
        card.routes.iter().find(|r| r.index == *index && r.devices.contains(&pd)).map(|r| (card, r))
    }

    fn is_current(&self, card: &Card, route: &Route) -> bool {
        self.default_sink().and_then(|s| self.route_of(s)).is_some_and(|(c, r)| c.id == card.id && r.index == route.index)
    }

    fn playable(&self, card: &Card, route: &Route) -> bool {
        self.is_current(card, route) || (route.available && (card.profile.is_some_and(|p| route.profiles.contains(&p)) || card.profile_for(route).is_some()))
    }

    pub fn outputs(&self) -> Vec<Output> {
        let routes = self.cards.iter().flat_map(|card| {
            card.routes.iter().filter(move |r| self.playable(card, r)).map(move |r| Output {
                id: format!("{}/{}", card.name, r.name),
                label: r.description.clone(),
                device: card.description.clone(),
                current: self.is_current(card, r),
            })
        });
        let bare = self.sinks.iter().filter(|s| self.route_of(s).is_none()).map(|s| Output {
            id: s.name.clone(),
            label: s.description.clone(),
            device: String::new(),
            current: s.name == self.default,
        });
        routes.chain(bare).collect()
    }

    /// What GNOME's own OSD labels the default sink with: its route, else the sink.
    pub fn current_label(&self) -> String {
        self.default_sink().map(|s| self.route_of(s).map_or(&s.description, |(_, r)| &r.description).clone()).unwrap_or_default()
    }

    fn find_route(&self, id: &str) -> Option<(&Card, &Route)> {
        let (card, route) = id.split_once('/')?;
        let card = self.cards.iter().find(|c| c.name == card)?;
        card.routes.iter().find(|r| r.name == route).map(|r| (card, r))
    }

    /// The card's sink on one of `devices`, and the route it plays through now.
    fn sink_for(&self, card: u64, devices: &[u64]) -> Option<(u64, Option<u64>)> {
        let sink = self.sinks.iter().find(|s| s.card == Some(card) && s.profile_device.is_some_and(|d| devices.contains(&d)))?;
        Some((sink.id, self.route_of(sink).map(|(_, r)| r.index)))
    }
}

impl Card {
    /// The profile to switch to for a route the current one does not play: the input kept when one can be, then the card's
    /// own preference.
    fn profile_for(&self, route: &Route) -> Option<u64> {
        let input = self.profiles.iter().find(|p| Some(p.index) == self.profile).and_then(|p| input_part(&p.name));
        self.profiles
            .iter()
            .filter(|p| p.available && route.profiles.contains(&p.index))
            .max_by_key(|p| (input.is_some_and(|i| p.name.split('+').any(|part| part == i)), p.priority))
            .map(|p| p.index)
    }
}

pub fn outputs() -> Result<Vec<Output>, String> {
    Ok(Graph::read()?.outputs())
}

pub fn select(id: &str) -> Result<(), String> {
    let graph = Graph::read()?;
    if let Some(sink) = graph.sinks.iter().find(|s| s.name == id) {
        return wpctl(&["set-default", &sink.id.to_string()]).map(drop);
    }
    let (card, route) = graph.find_route(id).ok_or_else(|| format!("no output '{id}'"))?;
    if !card.profile.is_some_and(|p| route.profiles.contains(&p)) {
        let profile = card.profile_for(route).ok_or_else(|| format!("{}: no profile plays {}", card.description, route.description))?;
        wpctl(&["set-profile", &card.id.to_string(), &profile.to_string()])?;
    }
    let (card_id, route_index, devices) = (card.id, route.index, route.devices.clone());
    // A profile switch brings the sink up a moment later, under a new id.
    let mut tries = 0;
    let (sink, playing) = loop {
        if let Some(found) = Graph::read()?.sink_for(card_id, &devices) {
            break found;
        }
        tries += 1;
        if tries == 30 {
            return Err(format!("{id}: no sink came up"));
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let sink = sink.to_string();
    if playing != Some(route_index) {
        wpctl(&["set-route", &sink, &route_index.to_string()])?;
    }
    wpctl(&["set-default", &sink]).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed from a real desktop: two GPU HDMI cards, a USB receiver on an input-only profile, the board's audio on S/PDIF.
    const DUMP: &str = r#"[
      {"id":53,"type":"PipeWire:Interface:Device","info":{"props":{"media.class":"Audio/Device","device.name":"alsa_card.pci-0000_03_00.1","device.description":"Navi 31 HDMI/DP Audio"},"params":{
        "EnumProfile":[{"index":0,"name":"off","priority":0,"available":"yes"},{"index":1,"name":"output:hdmi-stereo","priority":5900,"available":"yes"},{"index":2,"name":"output:hdmi-stereo-extra1","priority":5700,"available":"no"}],
        "Profile":[{"index":1,"name":"output:hdmi-stereo"}],
        "EnumRoute":[{"index":0,"direction":"Output","name":"hdmi-output-0","description":"HDMI / DisplayPort","available":"yes","devices":[4],"profiles":[1]},
                     {"index":1,"direction":"Output","name":"hdmi-output-1","description":"HDMI / DisplayPort 2","available":"no","devices":[5],"profiles":[2]}],
        "Route":[{"index":0,"direction":"Output","device":4}]}}},
      {"id":54,"type":"PipeWire:Interface:Device","info":{"props":{"media.class":"Audio/Device","device.name":"alsa_card.pci-0000_17_00.1","device.description":"Radeon High Definition Audio Controller"},"params":{
        "EnumProfile":[{"index":0,"name":"off","priority":0,"available":"yes"},{"index":1,"name":"output:hdmi-stereo","priority":5900,"available":"no"}],
        "Profile":[{"index":0,"name":"off"}],
        "EnumRoute":[{"index":0,"direction":"Output","name":"hdmi-output-0","description":"HDMI / DisplayPort","available":"no","devices":[4],"profiles":[1]}],
        "Route":[]}}},
      {"id":55,"type":"PipeWire:Interface:Device","info":{"props":{"media.class":"Audio/Device","device.name":"alsa_card.usb-Maono_DM40_RX-00","device.description":"Maono DM40 RX"},"params":{
        "EnumProfile":[{"index":0,"name":"off","priority":0,"available":"yes"},{"index":1,"name":"output:analog-stereo+input:mono-fallback","priority":6501,"available":"unknown"},
                       {"index":2,"name":"output:analog-stereo","priority":6500,"available":"unknown"},{"index":6,"name":"input:mono-fallback","priority":1,"available":"unknown"}],
        "Profile":[{"index":6,"name":"input:mono-fallback"}],
        "EnumRoute":[{"index":0,"direction":"Input","name":"analog-input-mic","description":"Microphone","available":"unknown","devices":[2],"profiles":[6,1]},
                     {"index":1,"direction":"Output","name":"analog-output","description":"Analog Output","available":"unknown","devices":[3],"profiles":[2,1]}],
        "Route":[{"index":0,"direction":"Input","device":2}]}}},
      {"id":56,"type":"PipeWire:Interface:Device","info":{"props":{"media.class":"Audio/Device","device.name":"alsa_card.pci-0000_17_00.6","device.description":"Ryzen HD Audio Controller"},"params":{
        "EnumProfile":[{"index":1,"name":"output:analog-stereo+input:analog-stereo","priority":6565,"available":"no"},{"index":2,"name":"output:analog-stereo","priority":6500,"available":"no"},
                       {"index":3,"name":"output:iec958-stereo+input:analog-stereo","priority":5565,"available":"yes"},{"index":4,"name":"output:iec958-stereo","priority":5500,"available":"yes"}],
        "Profile":[{"index":3,"name":"output:iec958-stereo+input:analog-stereo"}],
        "EnumRoute":[{"index":3,"direction":"Output","name":"analog-output-headphones","description":"Headphones","available":"no","devices":[4],"profiles":[2,1]},
                     {"index":4,"direction":"Output","name":"iec958-stereo-output","description":"Digital Output (S/PDIF)","available":"unknown","devices":[5],"profiles":[4,3]}],
        "Route":[{"index":4,"direction":"Output","device":5}]}}},
      {"id":51,"type":"PipeWire:Interface:Node","info":{"props":{"media.class":"Audio/Sink","node.name":"alsa_output.pci-0000_17_00.6.iec958-stereo","node.description":"Ryzen HD Audio Controller Digital Stereo (IEC958)","device.id":56,"card.profile.device":5}}},
      {"id":90,"type":"PipeWire:Interface:Node","info":{"props":{"media.class":"Audio/Sink","node.name":"alsa_output.pci-0000_03_00.1.hdmi-stereo","node.description":"Navi 31 HDMI/DP Audio Digital Stereo (HDMI)","device.id":53,"card.profile.device":4}}},
      {"id":120,"type":"PipeWire:Interface:Node","info":{"props":{"media.class":"Audio/Sink","node.name":"effect_input.eq","node.description":"Equalizer"}}},
      {"id":41,"type":"PipeWire:Interface:Metadata","props":{"metadata.name":"default"},"metadata":[
        {"subject":0,"key":"default.configured.audio.sink","type":"Spa:String:JSON","value":{"name":"alsa_output.pci-0000_03_00.1.hdmi-stereo-extra1"}},
        {"subject":0,"key":"default.audio.sink","type":"Spa:String:JSON","value":{"name":"alsa_output.pci-0000_03_00.1.hdmi-stereo"}}]}
    ]"#;

    fn ids(outputs: &[Output]) -> Vec<&str> {
        outputs.iter().map(|o| o.id.as_str()).collect()
    }

    #[test]
    fn lists_the_routes_a_card_can_play_and_the_sinks_no_route_stands_for() {
        let outputs = Graph::parse(DUMP).unwrap().outputs();
        assert_eq!(
            ids(&outputs),
            [
                "alsa_card.pci-0000_03_00.1/hdmi-output-0",
                "alsa_card.usb-Maono_DM40_RX-00/analog-output",
                "alsa_card.pci-0000_17_00.6/iec958-stereo-output",
                "effect_input.eq",
            ],
            "unplugged routes are left out, jacks of unknown state kept, the equalizer has no route"
        );
        assert_eq!((outputs[0].label.as_str(), outputs[0].device.as_str()), ("HDMI / DisplayPort", "Navi 31 HDMI/DP Audio"));
        assert_eq!(outputs.iter().filter(|o| o.current).map(|o| o.id.as_str()).collect::<Vec<_>>(), ["alsa_card.pci-0000_03_00.1/hdmi-output-0"]);
    }

    #[test]
    fn the_default_is_the_effective_one_not_the_configured() {
        let graph = Graph::parse(DUMP).unwrap();
        assert_eq!(graph.current_label(), "HDMI / DisplayPort");
        let without = DUMP.replace(r#""default.audio.sink""#, r#""other""#);
        let graph = Graph::parse(&without).unwrap();
        assert!(graph.outputs().iter().all(|o| !o.current));
        assert_eq!(graph.current_label(), "");
    }

    #[test]
    fn a_sink_of_no_route_is_labelled_by_itself() {
        let dump = DUMP.replace(r#""name":"alsa_output.pci-0000_03_00.1.hdmi-stereo"}}]"#, r#""name":"effect_input.eq"}}]"#);
        let graph = Graph::parse(&dump).unwrap();
        assert_eq!(graph.current_label(), "Equalizer");
        assert_eq!(graph.outputs().iter().filter(|o| o.current).map(|o| o.id.as_str()).collect::<Vec<_>>(), ["effect_input.eq"]);
    }

    #[test]
    fn a_route_off_the_profile_switches_to_one_keeping_the_input() {
        let graph = Graph::parse(DUMP).unwrap();
        let (card, route) = graph.find_route("alsa_card.usb-Maono_DM40_RX-00/analog-output").unwrap();
        assert_eq!(card.profile_for(route), Some(1), "output+input over output alone: the microphone stays");
        let (card, route) = graph.find_route("alsa_card.pci-0000_17_00.6/analog-output-headphones").unwrap();
        assert_eq!(card.profile_for(route), None, "every analog profile is unavailable");
        assert!(graph.find_route("alsa_card.pci-0000_17_00.6/nope").is_none());
    }

    #[test]
    fn metadata_as_a_json_string() {
        let dump = DUMP.replace(r#"{"name":"alsa_output.pci-0000_03_00.1.hdmi-stereo"}"#, r#""{\"name\":\"alsa_output.pci-0000_17_00.6.iec958-stereo\"}""#);
        assert_eq!(Graph::parse(&dump).unwrap().current_label(), "Digital Output (S/PDIF)");
    }

    #[test]
    fn not_json_is_an_error() {
        assert!(Graph::parse("not json").is_err());
        assert!(Graph::parse("[]").unwrap().outputs().is_empty());
    }
}
