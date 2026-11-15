//! Local keyboard preferences and the garage's key-capture workflow.
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod persistence;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Left,
    Right,
    Boost,
    Brake,
    Restart,
}
pub const ACTIONS: [Action; 5] = [
    Action::Left,
    Action::Right,
    Action::Boost,
    Action::Brake,
    Action::Restart,
];
impl Action {
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "Steer left",
            Self::Right => "Steer right",
            Self::Boost => "Boost",
            Self::Brake => "Brake",
            Self::Restart => "Restart / end drive",
        }
    }
}

// Explicit stable storage names; physical positions, independent of typed text.
macro_rules! keys {
    ($($key:ident),* $(,)?) => { &[$((stringify!($key), KeyCode::$key)),*] };
}
const KEYS: &[(&str, KeyCode)] = keys![
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Space,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Backspace,
    Enter,
    Tab,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    Minus,
    Equal,
    BracketLeft,
    BracketRight,
    Backslash,
    Semicolon,
    Quote,
    Backquote,
    Comma,
    Period,
    Slash,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadSubtract,
    NumpadMultiply,
    NumpadDivide,
    NumpadDecimal,
    NumpadEnter,
];
fn key_name(key: KeyCode) -> Option<&'static str> {
    KEYS.iter()
        .find(|(_, code)| *code == key)
        .map(|(name, _)| *name)
}
fn key_code(name: &str) -> Option<KeyCode> {
    KEYS.iter()
        .find(|(id, _)| *id == name)
        .map(|(_, code)| *code)
}
pub fn label(name: &str) -> String {
    if let Some(letter) = name
        .strip_prefix("Key")
        .or_else(|| name.strip_prefix("Digit"))
    {
        letter.into()
    } else if let Some(direction) = name.strip_prefix("Arrow") {
        format!("Arrow {direction}")
    } else {
        name.into()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingMap {
    format: String,
    version: u32,
    keys: BTreeMap<Action, Vec<String>>,
}
impl Default for BindingMap {
    fn default() -> Self {
        Self {
            format: "car-drive-key-bindings".into(),
            version: 1,
            keys: [
                (Action::Left, vec!["KeyA", "ArrowLeft"]),
                (Action::Right, vec!["KeyD", "ArrowRight"]),
                (Action::Boost, vec!["Space", "ArrowUp", "KeyW"]),
                (Action::Brake, vec!["ArrowDown", "KeyS"]),
                (Action::Restart, vec!["KeyR", "Backspace"]),
            ]
            .into_iter()
            .map(|(a, keys)| (a, keys.into_iter().map(str::to_owned).collect()))
            .collect(),
        }
    }
}
impl BindingMap {
    fn decode(text: &str) -> Result<Self, String> {
        if text.len() > 16_384 {
            return Err("Settings file is too large".into());
        }
        let value: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if value.format != "car-drive-key-bindings" || value.version != 1 {
            return Err("Unsupported key-bindings format or version".into());
        }
        let mut seen = std::collections::HashSet::new();
        for action in ACTIONS {
            let keys = value.keys.get(&action).ok_or("Missing action")?;
            if keys.is_empty() || keys.len() > 4 {
                return Err("Each action needs 1–4 keys".into());
            }
            for key in keys {
                if key_code(key).is_none() || !seen.insert(key) {
                    return Err("Unknown or conflicting key".into());
                }
            }
        }
        Ok(value)
    }
    pub fn keys(&self, action: Action) -> &[String] {
        &self.keys[&action]
    }
    pub fn labels(&self, action: Action) -> String {
        self.keys(action)
            .iter()
            .map(|s| label(s))
            .collect::<Vec<_>>()
            .join(" / ")
    }
    pub fn pressed(&self, action: Action, input: &ButtonInput<KeyCode>, edge: bool) -> bool {
        self.keys(action)
            .iter()
            .filter_map(|s| key_code(s))
            .any(|key| {
                if edge {
                    input.just_pressed(key)
                } else {
                    input.pressed(key)
                }
            })
    }
    fn assign(&mut self, action: Action, slot: Option<usize>, key: KeyCode) -> Result<(), String> {
        let name = key_name(key).ok_or(
            "Unsupported key. G and Esc are reserved; modifiers and chords are not supported.",
        )?;
        for other in ACTIONS {
            if self
                .keys(other)
                .iter()
                .enumerate()
                .any(|(i, k)| k == name && !(other == action && slot == Some(i)))
            {
                return Err(format!(
                    "{} is already assigned to {}. Choose another key or cancel.",
                    label(name),
                    other.name()
                ));
            }
        }
        let keys = self.keys.get_mut(&action).ok_or("Unknown action")?;
        if let Some(slot) = slot {
            *keys.get_mut(slot).ok_or("Binding no longer exists")? = name.into();
        } else if keys.len() < 4 {
            keys.push(name.into());
        } else {
            return Err("At most four keys per action".into());
        }
        Ok(())
    }
    fn remove(&mut self, action: Action, slot: usize) -> Result<(), String> {
        let keys = self.keys.get_mut(&action).ok_or("Unknown action")?;
        if keys.len() <= 1 {
            return Err("Keep at least one key for each action".into());
        }
        if slot >= keys.len() {
            return Err("Binding no longer exists".into());
        }
        keys.remove(slot);
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum BindingIntent {
    Open,
    Close,
    Select(Action),
    Capture(Option<usize>),
    Remove(usize),
    Reset,
    Cancel,
    Retry,
}
#[derive(Resource)]
pub struct Bindings {
    pub map: BindingMap,
    pub open: bool,
    pub selected: Action,
    pub capture: Option<Option<usize>>,
    pub capture_ready: bool,
    pub notice: String,
    pub revision: u64,
    pub loading: bool,
    pub dirty: bool,
    pub save_error: bool,
    // Prevent a captured/cancelled key from leaking into the menu until released.
    pub waiting_release: bool,
}
impl Default for Bindings {
    fn default() -> Self {
        Self {
            map: BindingMap::default(),
            open: false,
            selected: Action::Boost,
            capture: None,
            capture_ready: false,
            notice: String::new(),
            revision: 0,
            loading: false,
            dirty: false,
            save_error: false,
            waiting_release: false,
        }
    }
}
impl Bindings {
    pub fn apply(&mut self, intent: BindingIntent) {
        if self.loading {
            return;
        }
        if self.capture.is_some() && !matches!(intent, BindingIntent::Cancel) {
            return;
        }
        let result = match intent {
            BindingIntent::Open => {
                self.open = true;
                Ok(())
            }
            BindingIntent::Close => {
                self.open = false;
                Ok(())
            }
            BindingIntent::Select(action) => {
                self.selected = action;
                Ok(())
            }
            BindingIntent::Capture(slot) => {
                self.capture = Some(slot);
                self.capture_ready = false;
                self.notice = "Release keys, then press a new key. Esc cancels.".into();
                Ok(())
            }
            BindingIntent::Cancel => {
                self.capture = None;
                self.waiting_release = true;
                if !self.save_error {
                    self.notice = "Binding unchanged.".into();
                }
                Ok(())
            }
            BindingIntent::Remove(slot) => {
                let result = self.map.remove(self.selected, slot);
                if result.is_ok() {
                    self.changed();
                }
                result
            }
            BindingIntent::Reset => {
                self.map = BindingMap::default();
                self.changed();
                Ok(())
            }
            BindingIntent::Retry => {
                self.dirty = true;
                self.save_error = false;
                Ok(())
            }
        };
        if let Err(error) = result {
            self.notice = error;
        }
        self.revision += 1;
    }
    fn changed(&mut self) {
        self.dirty = true;
        self.save_error = false;
        self.notice = "Saving key bindings...".into();
    }
    pub fn capture_key(&mut self, key: KeyCode) {
        let Some(slot) = self.capture else {
            return;
        };
        match self.map.assign(self.selected, slot, key) {
            Ok(()) => {
                self.capture = None;
                self.waiting_release = true;
                self.changed();
            }
            Err(error) => self.notice = error,
        }
        self.revision += 1;
    }
}

pub struct BindingsPlugin;
impl Plugin for BindingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Bindings>()
            .add_plugins(persistence::PersistencePlugin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_round_trip_and_conflicts_preserve_the_previous_mapping() {
        let mut map = BindingMap::default();
        map.assign(Action::Boost, Some(0), KeyCode::KeyB).unwrap();
        map.assign(Action::Brake, None, KeyCode::KeyV).unwrap();
        map.remove(Action::Boost, 1).unwrap();
        let saved = serde_json::to_string(&map).unwrap();
        assert_eq!(BindingMap::decode(&saved).unwrap(), map);
        let before = map.clone();
        assert!(map.assign(Action::Boost, Some(0), KeyCode::KeyV).is_err());
        assert!(map.assign(Action::Boost, None, KeyCode::Escape).is_err());
        assert!(map.assign(Action::Boost, None, KeyCode::KeyG).is_err());
        assert_eq!(map, before);
        map.remove(Action::Left, 0).unwrap();
        assert!(map.remove(Action::Left, 0).is_err());
    }
    #[test]
    fn invalid_and_future_settings_are_rejected() {
        let map = BindingMap::default();
        let text = serde_json::to_string(&map).unwrap();
        assert!(BindingMap::decode(&text.replace("\"version\":1", "\"version\":2")).is_err());
        assert!(BindingMap::decode(&text.replace("ArrowDown", "Space")).is_err());
        assert!(BindingMap::decode(&text.replace("ArrowDown", "Unknown")).is_err());
        assert!(BindingMap::decode("{}").is_err());
        assert!(BindingMap::decode(&" ".repeat(16_385)).is_err());
    }
    #[test]
    fn cancel_and_reset_keep_every_action_reachable() {
        let mut bindings = Bindings::default();
        bindings.apply(BindingIntent::Capture(Some(0)));
        bindings.capture_key(KeyCode::KeyB);
        assert_eq!(bindings.map.keys(Action::Boost)[0], "KeyB");
        let map = bindings.map.clone();
        bindings.apply(BindingIntent::Capture(None));
        bindings.apply(BindingIntent::Cancel);
        assert_eq!(bindings.map, map);
        assert!(bindings.capture.is_none());
        bindings.apply(BindingIntent::Reset);
        assert_eq!(bindings.map, BindingMap::default());
    }
}
