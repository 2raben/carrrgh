//! Tiny versioned preferences, independent of profiles and market realms.
use super::{BindingMap, Bindings};
use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task, block_on, poll_once};

#[derive(Resource, Default)]
struct Pending(Option<Task<Result<Option<BindingMap>, String>>>);
pub(super) struct PersistencePlugin;
impl Plugin for PersistencePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pending>()
            .add_systems(Startup, load)
            .add_systems(Update, save)
            .add_systems(Last, flush);
    }
}
fn load(mut bindings: ResMut<Bindings>, mut pending: ResMut<Pending>) {
    bindings.loading = true;
    pending.0 = Some(IoTaskPool::get().spawn(async { read() }));
}
fn finish(bindings: &mut Bindings, result: Result<Option<BindingMap>, String>) {
    let loading = std::mem::take(&mut bindings.loading);
    match result {
        Ok(map) => {
            if let Some(map) = map {
                bindings.map = map;
            }
            bindings.save_error = false;
            bindings.notice = if loading {
                String::new()
            } else {
                "Key bindings saved on this device.".into()
            };
        }
        Err(error) => {
            bindings.save_error = true;
            bindings.notice = format!(
                "Bindings not saved: {error}. Restore defaults or retry saving to replace the stored settings."
            );
        }
    }
    bindings.revision += 1;
}
fn save(mut bindings: ResMut<Bindings>, mut pending: ResMut<Pending>) {
    if let Some(result) = pending
        .0
        .as_mut()
        .and_then(|task| block_on(poll_once(task)))
    {
        pending.0 = None;
        finish(&mut bindings, result);
    }
    if pending.0.is_none() && bindings.dirty {
        bindings.dirty = false;
        let map = bindings.map.clone();
        pending.0 = Some(IoTaskPool::get().spawn(async move { write(&map).map(|()| None) }));
    }
}
fn flush(
    mut exits: MessageReader<AppExit>,
    mut bindings: ResMut<Bindings>,
    mut pending: ResMut<Pending>,
) {
    if exits.read().next().is_none() {
        return;
    }
    if let Some(task) = pending.0.take() {
        finish(&mut bindings, block_on(task));
    }
    if bindings.dirty {
        let result = write(&bindings.map).map(|()| None);
        finish(&mut bindings, result);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn path() -> Result<std::path::PathBuf, String> {
    crate::storage::sqlite::SqliteStore::default_path()
        .map(|p| p.with_file_name("key-bindings.json"))
        .ok_or("No local data directory".into())
}
#[cfg(not(target_arch = "wasm32"))]
fn read() -> Result<Option<BindingMap>, String> {
    use std::io::Read;
    let file = match std::fs::File::open(path()?) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut text = String::new();
    file.take(16_385)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    BindingMap::decode(&text).map(Some)
}
#[cfg(not(target_arch = "wasm32"))]
fn write(map: &BindingMap) -> Result<(), String> {
    write_at(&path()?, map).map_err(|e| e.to_string())
}
#[cfg(not(target_arch = "wasm32"))]
fn write_at(path: &std::path::Path, map: &BindingMap) -> std::io::Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("No parent directory"))?;
    std::fs::create_dir_all(parent)?;
    let temp = path.with_extension(format!("{}.tmp", crate::storage::RunId::random().0));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(serde_json::to_string_pretty(map)?.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}
#[cfg(target_arch = "wasm32")]
fn storage() -> Result<web_sys::Storage, String> {
    web_sys::window()
        .ok_or("No browser window")?
        .local_storage()
        .map_err(|e| format!("{e:?}"))?
        .ok_or("Local storage is blocked".into())
}
#[cfg(target_arch = "wasm32")]
fn read() -> Result<Option<BindingMap>, String> {
    storage()?
        .get_item("car-drive/key-bindings")
        .map_err(|e| format!("{e:?}"))?
        .map(|text| BindingMap::decode(&text))
        .transpose()
}
#[cfg(target_arch = "wasm32")]
fn write(map: &BindingMap) -> Result<(), String> {
    storage()?
        .set_item(
            "car-drive/key-bindings",
            &serde_json::to_string(map).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("{e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_preferences_replace_reopens_without_changing_profile_files() {
        let dir = std::env::temp_dir().join(format!(
            "car-drive-bindings-{}",
            crate::storage::RunId::random().0
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let profile = dir.join("profiles.sqlite3");
        std::fs::write(&profile, b"unrelated profile").unwrap();
        let path = dir.join("key-bindings.json");
        let mut map = BindingMap::default();
        write_at(&path, &map).unwrap();
        map.assign(super::super::Action::Boost, Some(0), KeyCode::KeyB)
            .unwrap();
        write_at(&path, &map).unwrap();
        assert_eq!(
            BindingMap::decode(&std::fs::read_to_string(&path).unwrap()).unwrap(),
            map
        );
        assert_eq!(std::fs::read(&profile).unwrap(), b"unrelated profile");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
