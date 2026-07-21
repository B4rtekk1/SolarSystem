use crate::camera::Camera;
use crate::ecs::{Entity, World};
use crate::nbody::NBodySimulation;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAGIC: &[u8; 4] = b"ORBS";
const VERSION: u8 = 1;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Deserialize)]
pub struct SaveData {
    pub world: World,
    pub physics: NBodySimulation,
    pub camera: Camera,
    pub simulation_speed: f64,
    pub simulation_paused: bool,
    pub orbits_visible: bool,
    pub planet_orbits_visible: bool,
    pub moon_orbits_visible: bool,
    pub orbit_thickness_scale: f32,
    pub selected_body: Option<Entity>,
    #[serde(default)]
    pub camera_follow_enabled: bool,
    pub initial_total_energy_by_entity: Vec<Option<f64>>,
    pub rotation_time: f32,
    pub window_width: u32,
    pub window_height: u32,
}

pub fn save_to_file(path: impl AsRef<Path>, data: &SaveData) -> std::io::Result<()> {
    validate_save_data(data).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Refusing to save invalid scene: {error}"),
        )
    })?;

    let path = path.as_ref();
    let temp_path = temp_path_for(path)?;
    let result = write_temp_and_replace(path, &temp_path, data);
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn write_temp_and_replace(path: &Path, temp_path: &Path, data: &SaveData) -> std::io::Result<()> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp_path)?;
    let mut writer = BufWriter::new(file);
    write_header(&mut writer)?;
    serde_json::to_writer(&mut writer, data).map_err(invalid_data)?;
    writer.flush()?;
    let file = writer
        .into_inner()
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    file.sync_all()?;
    replace_file(temp_path, path)?;
    sync_parent(path)?;
    Ok(())
}

pub fn load_from_file(path: impl AsRef<Path>) -> std::io::Result<SaveData> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    check_header(&mut reader)?;
    let data = serde_json::from_reader(reader).map_err(invalid_data)?;
    validate_save_data(&data).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Invalid save data: {error}"),
        )
    })?;
    Ok(data)
}

fn temp_path_for(path: &Path) -> std::io::Result<PathBuf> {
    let file_name = path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Save path has no file name",
        )
    })?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_name = format!(
        ".{}.{}.{}.tmp",
        file_name.to_string_lossy(),
        std::process::id(),
        counter
    );
    Ok(parent.join(temp_name))
}

fn replace_file(temp_path: &Path, path: &Path) -> std::io::Result<()> {
    match fs::rename(temp_path, path) {
        Ok(()) => Ok(()),
        Err(error) if path.exists() => {
            fs::remove_file(path)?;
            fs::rename(temp_path, path).map_err(|rename_error| {
                std::io::Error::new(
                    rename_error.kind(),
                    format!("{rename_error}; original replace error: {error}"),
                )
            })
        }
        Err(error) => Err(error),
    }
}

fn sync_parent(path: &Path) -> std::io::Result<()> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };

    match File::open(parent).and_then(|directory| directory.sync_all()) {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Unsupported
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn write_header(w: &mut impl Write) -> std::io::Result<()> {
    w.write_all(MAGIC)?;
    w.write_all(&[VERSION])?;
    Ok(())
}

fn check_header(r: &mut impl std::io::Read) -> std::io::Result<()> {
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid file format",
        ));
    }
    let mut version = [0u8; 1];
    r.read_exact(&mut version)?;
    if version[0] != VERSION {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Unsupported file version",
        ));
    }
    Ok(())
}

fn invalid_data(error: serde_json::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, error)
}

fn validate_save_data(data: &SaveData) -> Result<(), String> {
    data.world.validate()?;
    data.physics.validate_for_world(&data.world)?;
    data.camera.validate()?;

    if !data.simulation_speed.is_finite()
        || !data.orbit_thickness_scale.is_finite()
        || !data.rotation_time.is_finite()
    {
        return Err("Scene controls contain invalid values".to_string());
    }
    if let Some(entity) = data.selected_body {
        if entity.index() >= data.world.entity_capacity() {
            return Err("Selected entity is outside the world".to_string());
        }
    }
    if data.initial_total_energy_by_entity.len() != data.world.entity_capacity() {
        return Err("Initial energy array length does not match world".to_string());
    }
    for energy in data.initial_total_energy_by_entity.iter().flatten() {
        if !energy.is_finite() {
            return Err("Initial energy array contains invalid values".to_string());
        }
    }
    if data.window_width == 0 || data.window_height == 0 {
        return Err("Saved window size must be non-zero".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nbody::NBodyConfig;
    use crate::scene::create_world;
    use std::io::Read;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn save_round_trips_scene() {
        let path = test_path("round-trip.orbs");
        let data = test_save_data();

        save_to_file(&path, &data).unwrap();
        let loaded = load_from_file(&path).unwrap();

        assert_eq!(loaded.world.entity_capacity(), data.world.entity_capacity());
        assert_eq!(
            loaded.physics.planet_entities(),
            data.physics.planet_entities()
        );
        assert_eq!(loaded.window_width, data.window_width);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn load_rejects_corrupt_header() {
        let path = test_path("bad-header.orbs");
        fs::write(&path, b"NOPE\x01{}").unwrap();

        let error = load_error(&path);

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn load_rejects_unsupported_version() {
        let path = test_path("bad-version.orbs");
        fs::write(&path, b"ORBS\xff{}").unwrap();

        let error = load_error(&path);

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn load_rejects_incomplete_file() {
        let path = test_path("incomplete.orbs");
        fs::write(&path, b"ORBS\x01{\"world\"").unwrap();

        let error = load_error(&path);

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn save_does_not_replace_existing_file_when_data_is_invalid() {
        let path = test_path("invalid-save.orbs");
        fs::write(&path, b"existing").unwrap();
        let mut data = test_save_data();
        data.initial_total_energy_by_entity.pop();

        let error = save_to_file(&path, &data).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        let mut contents = Vec::new();
        File::open(&path)
            .unwrap()
            .read_to_end(&mut contents)
            .unwrap();
        assert_eq!(contents, b"existing");
        let _ = fs::remove_file(path);
    }

    fn test_save_data() -> SaveData {
        let world = create_world();
        let physics = NBodySimulation::from_world(&world, NBodyConfig::default());
        SaveData {
            initial_total_energy_by_entity: vec![None; world.entity_capacity()],
            world,
            physics,
            camera: Camera::default(),
            simulation_speed: 1.0,
            simulation_paused: false,
            orbits_visible: true,
            planet_orbits_visible: true,
            moon_orbits_visible: true,
            orbit_thickness_scale: 1.0,
            selected_body: None,
            camera_follow_enabled: false,
            rotation_time: 0.0,
            window_width: 1280,
            window_height: 800,
        }
    }

    fn test_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("SolarSystem-{nanos}-{name}"))
    }

    fn load_error(path: &Path) -> std::io::Error {
        match load_from_file(path) {
            Ok(_) => panic!("load unexpectedly succeeded"),
            Err(error) => error,
        }
    }
}
