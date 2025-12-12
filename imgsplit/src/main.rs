use std::{io::{Read, Seek, SeekFrom, Write}, path::Path};

use anyhow::{Context, Result, anyhow};
use bytemuck::{Pod, Zeroable};
use lexopt::{Arg, Parser, ValueExt};

struct Args {
    input_path: String,
}

fn parse_args() -> Result<Args, lexopt::Error> {
    let mut parser = Parser::from_env();
    let mut input_path = None;

    while let Some(arg) = parser.next()? {
        match arg {
            Arg::Value(val) => {
                if input_path.is_none() {
                    input_path = Some(val.string()?);
                }
            }
            _ => return Err(arg.unexpected()),
        }
    }

    if input_path.is_none() {
        eprint!("Error: No input path specified.\n");
        std::process::exit(1);
    }

    Ok(Args {
        input_path: input_path.unwrap(),
    })
}

fn main() -> Result<()> {
    let args = parse_args().map_err(|e| anyhow!("Failed to parse command line: {}", e))?;
    let cache = load_cd_cache(&args.input_path)?;

    let file_name = Path::new(&args.input_path).join("PSXCD.IMG");
    let mut file = std::fs::File::open(&file_name).with_context(|| format!("Failed to open file: {}", file_name.display()))?;

    for (i, name) in cache.names().iter().enumerate() {
        if name.name[0] == 0 {
            break;
        }
        let loc = &cache.locs()[i];
        let filename = String::from_utf8_lossy(&name.name);
        println!(
            "File {}: {} (start block: {}, num blocks: {}, size: {})",
            i,
            filename.trim_end_matches('\0'),
            loc.start_block,
            loc.num_blocks,
            loc.file_size
        );

        file.seek(SeekFrom::Start((loc.start_block as u64) * 0x800))?;
        let mut buffer = vec![0u8; (loc.num_blocks as usize) * 0x800];
        file.read_exact(&mut buffer)?;

        let mut outfile = std::fs::File::create(filename.trim_end_matches('\0'))?;
        outfile.write_all(&buffer[..(loc.file_size as usize)])?;
    }
    Ok(())
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
struct CdLoc {
    start_block: u32,
    num_blocks: u32,
    file_size: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
struct CdName {
    name: [u8; 32],
}

struct CDCache {
    name_file_data: Vec<u8>,
    loc_file_data: Vec<u8>,
}

impl CDCache {
    fn names(&self) -> &[CdName] {
        let size = std::mem::size_of::<CdName>();
        let len = self.name_file_data.len() / size;
        bytemuck::try_cast_slice(&self.name_file_data[..len * size]).expect("Buffer not aligned for CdName")
    }
    fn locs(&self) -> &[CdLoc] {
        let size = std::mem::size_of::<CdLoc>();
        let len = self.loc_file_data.len() / size;
        bytemuck::try_cast_slice(&self.loc_file_data[..len * size]).expect("Buffer not aligned for CdLoc")
    }
}

fn load_cd_cache(path: &str) -> Result<CDCache> {
    let mut file_name = Path::new(path).join("PSXCDNAM.BIN");
    let mut file = std::fs::File::open(&file_name).with_context(|| format!("Failed to open file: {}", file_name.display()))?;
    let file_size = file.metadata()?.len() as usize;
    let mut name_file_data = vec![0u8; file_size];
    file.read_exact(&mut name_file_data).context("Failed to read file data")?;

    file_name = Path::new(path).join("PSXCDLOC.BIN");
    file = std::fs::File::open(&file_name).with_context(|| format!("Failed to open file: {}", file_name.display()))?;
    let file_size = file.metadata()?.len() as usize;
    let mut loc_file_data = vec![0u8; file_size];
    file.read_exact(&mut loc_file_data).context("Failed to read file data")?;

    Ok(CDCache {
        name_file_data,
        loc_file_data,
    })
}
