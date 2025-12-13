use byteorder::{LittleEndian, ReadBytesExt};
use lexopt::{Arg, Parser, ValueExt};
use std::io::{Read, Seek};

struct Args {
    filenames: Vec<String>,
    skipcheck: bool,
    output_dir: String,
}

fn parse_args() -> Result<Args, lexopt::Error> {
    let mut filenames = Vec::new();
    let mut skipcheck = false;
    let mut output_dir = None;
    let mut parser = Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Arg::Short('s') | Arg::Long("skipcheck") => {
                skipcheck = true;
            }
            Arg::Short('o') | Arg::Long("output") => {
                output_dir = Some(parser.value()?.string()?);
            }
            Arg::Value(val) => {
                filenames.push(val.string()?);
            }
            Arg::Long("help") => {
                println!("Usage: binextract [-s|--skipcheck] <binfile>");
                std::process::exit(0);
            }
            _ => return Err(arg.unexpected()),
        }
    }

    if filenames.is_empty() {
        eprint!("Error: No input file specified.\n");
        std::process::exit(1);
    }

    return Ok(Args {
        filenames,
        skipcheck,
        output_dir :output_dir.unwrap_or_default(),
    });
}

fn main() {
    //pull all command args and treat the first like the input
    let args = parse_args().expect("Failed to parse command line");
    for filename in args.filenames {
        let input_file = &filename;
        //open the input file as binary and read the first 4 bytes as a little endian u32 to get the number of entries
        let mut file = std::fs::File::open(input_file).expect("Failed to open input file");

        let num_entries = file.read_u32::<LittleEndian>().expect("Failed to read number of entries");
        println!("Number of entries: {}", num_entries);

        //sanity check the number of entries
        if num_entries == 0 || num_entries > 10000 {
            eprintln!("Error: Suspicious number of entries in {}: {}", input_file, num_entries);
            continue;
        }

        //read the next num_entries * little endian u32s as file lengths
        let mut lengths = Vec::new();
        for _ in 0..num_entries {
            let length = file.read_u32::<LittleEndian>().expect("Failed to read file length");
            lengths.push(length);
        }

        println!("Finished header data at: 0x{:X}", file.stream_position().expect("Failed to read position"));

        let mut num_files = num_entries;
        if !args.skipcheck {
            //first check the last entry and see if it contains the string 'PSP CHECK'
            let (last_entry_offset, last_entry_length) = calc_offset_to_entry((num_entries - 1) as usize, &lengths);
            file.seek(std::io::SeekFrom::Start(last_entry_offset))
                .expect("Failed to seek to last entry");
            let mut last_entry_data = vec![0u8; last_entry_length as usize];
            if let Err(e) = file.read_exact(&mut last_entry_data) {
                eprintln!("Error: Failed to read last entry data, invalid file. Error reported was: {}", e);
                std::process::exit(1);
            }
            if last_entry_data.starts_with(b"PSPCHECK") == false {
                eprintln!("Error: Last entry is not a 'PSPCHECK' signature, invalid file.");
                std::process::exit(1);
            }
            num_files -= 1;
        }

        // make a directory for the extracted files with the name of the input file without extension
        let input_name = std::path::Path::new(input_file).file_stem().expect("Failed to get file stem");
        let mut output_dir = std::path::PathBuf::from(&args.output_dir);
        output_dir.push(&input_name);
        std::fs::create_dir_all(&output_dir).expect("Failed to create output directory");

        for i in 0..num_files {
            let (entry_offset, entry_length) = calc_offset_to_entry(i as usize, &lengths);
            println!("Processing file {} - offset: 0x{:X} size: 0x{:X}", i, entry_offset, entry_length);

            file.seek(std::io::SeekFrom::Start(entry_offset))
                .expect("Failed to seek to file data");

            let mut file_data = vec![0u8; entry_length as usize];
            file.read_exact(&mut file_data).expect("Failed to read file data");

            println!("Finished reading file data at: 0x{:X}", file.stream_position().expect("Failed to read position"));

            let suffix = detect_file_suffix(&file_data);
            let mut output_path = std::path::PathBuf::from(&output_dir); // use specified output directory
            output_path.push(input_name); //add input file stem as base name
            output_path.add_extension(format!("{}.{}", i, suffix)); //add index and suffix as extension
            std::fs::write(&output_path, &file_data).expect("Failed to write output file");
            println!("Extracted file {}: {} bytes", output_path.display(), entry_length);
        }
    }
}

fn detect_file_suffix(file_data: &[u8]) -> &'static str {
    match file_data.get(0..4) {
        Some(b"MIG.") => "gim", //PSP Image
        Some(b"MThd") => "mid", //MIDI Audio
        Some(b"PPHD") => "phd", //PSP Audio
        Some(b"PSMF") => "psmf", //PSP Movie
        Some(b"VAGp") => "vag", //Playstation Audio
        _ => "bin",
    }
}

fn calc_offset_to_entry(index: usize, lengths: &[u32]) -> (u64, u64) {
    let mut offset = 4 + (lengths.len() as u64 * 4);
    if offset & 15 != 0 {
        offset = (offset & !15) + 16;
    }

    if index > 0 {
        for i in 0..index {
            offset += lengths[i] as u64;
            if offset & 15 != 0 {
                offset = (offset & !15) + 16;
            }
        }
    }

    return (offset, lengths[index] as u64);
}
