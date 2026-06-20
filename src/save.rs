use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

const MAGIC: &[u8; 4] = b"ORBS";
const VERSION: u8 = 1;

#[derive(Serialize, Deserialize)]
struct SaveData {}

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
