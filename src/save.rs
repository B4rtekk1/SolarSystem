use crate::camera::Camera;
use crate::ecs::{Entity, World};
use crate::nbody::NBodySimulation;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

const MAGIC: &[u8; 4] = b"ORBS";
const VERSION: u8 = 1;

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
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    write_header(&mut writer)?;
    serde_json::to_writer(&mut writer, data).map_err(invalid_data)?;
    writer.flush()
}

pub fn load_from_file(path: impl AsRef<Path>) -> std::io::Result<SaveData> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    check_header(&mut reader)?;
    serde_json::from_reader(reader).map_err(invalid_data)
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
