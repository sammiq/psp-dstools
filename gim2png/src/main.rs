mod gim;

use anyhow::{Context, Result, bail};
use lexopt::{Arg, Parser, ValueExt};
use std::{
    borrow::Cow,
    io::{Read, Seek, SeekFrom},
};

struct Args {
    filenames: Vec<String>,
    offset: u64,
    tx: usize,
    ty: usize,
    linear: bool,
    verbose: bool,
}

fn parse_args() -> Result<Args, lexopt::Error> {
    let mut filenames = Vec::new();
    let mut offset = 0;
    let mut tx = 0;
    let mut ty = 0;
    let mut linear = false;
    let mut verbose = false;

    let mut parser = Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Arg::Short('x') | Arg::Long("tx") => {
                tx = parser.value()?.parse()?;
            }
            Arg::Short('y') | Arg::Long("ty") => {
                ty = parser.value()?.parse()?;
            }
            Arg::Short('o') | Arg::Long("offset") => {
                offset = parser.value()?.parse()?;
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                verbose = true;
            }
            Arg::Short('l') | Arg::Long("linear") => {
                linear = true;
            }
            Arg::Value(val) => {
                filenames.push(val.string()?);
            }
            Arg::Long("help") => {
                println!("Usage: gim2png [options] <files>...");
                println!("Options:");
                println!("  -l, --linear         treat PSP tiled images as linear");
                println!("  -o, --offset <n>     Skip the first <n> bytes of the input file");
                println!("  -v, --verbose        Enable verbose output");
                println!("  -x, --tx <n>         Tile width (default 0 for auto)");
                println!("  -y, --ty <n>         Tile height (default 0 for auto)");
                println!("  --help               Show this help message");
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
        tx,
        ty,
        offset,
        linear,
        verbose,
    });
}

//macro to println based on verbose flag, that takes the verbose flag as first arg and the rest as normal println args
macro_rules! vprintln {
    ($verbose:expr, $($arg:tt)*) => {
        if $verbose {
            println!($($arg)*);
        }
    };
}

fn main() -> Result<()> {
    let args = parse_args().expect("Failed to parse command line");
    for filename in &args.filenames {
        match process_image(filename, &args) {
            Ok(_) => {}
            Err(e) => eprintln!("Error processing file {}: {}", filename, e),
        }
    }
    Ok(())
}

fn process_image(filename: &str, args: &Args) -> Result<()> {
    let mut file = std::fs::File::open(filename).with_context(|| format!("Failed to open file: {}", filename))?;
    vprintln!(args.verbose, "Opened file: {}", filename);
    let input_name = std::path::Path::new(filename).file_stem().unwrap().to_string_lossy();

    //work out file size
    let file_size = file.metadata()?.len();
    vprintln!(args.verbose, "File size: {} bytes", file_size);

    if args.offset > 0 {
        vprintln!(args.verbose, "Seeking to offset: {}", args.offset);
        Seek::seek(&mut file, SeekFrom::Start(args.offset)).with_context(|| format!("Failed to seek to offset {}", args.offset))?;
    }

    vprintln!(args.verbose, "Reading file data...");
    let mut file_data = vec![0u8; file_size as usize];
    file.read_exact(&mut file_data).context("Failed to read file data")?;

    let picture = gim::load_gim_image(&file_data).context("Failed to load image")?;
    let format: gim::ImageFormat = picture.image_header.image_format().context("Failed to get image format")?;
    let order: gim::ImageOrder = picture.image_header.image_order().context("Failed to get image order")?;

    vprintln!(args.verbose, "GIM Image Format: {:?}", format);
    vprintln!(args.verbose, "GIM Image Order: {:?}", order);

    if picture.image_header.frame_count > 1 || picture.image_header.level_count > 1 {
        bail!("WARNING: GIM Image has multiple frames or levels, which is not supported for conversion.");
    }

    if format == gim::ImageFormat::RGBA8888 {
        let output_file_name = format!("{}.png", input_name);
        vprintln!(args.verbose, "Writing output file: {}", output_file_name);
        let mut ow = std::io::BufWriter::new(std::fs::File::create(&output_file_name).context("Failed to create output file")?);

        //the data is aligned by these parameters from the header
        let iw = (picture.image_header.width as usize).div_ceil(picture.image_header.pitch_align as usize)
            * picture.image_header.pitch_align as usize;
        let ih = (picture.image_header.height as usize).div_ceil(picture.image_header.height_align as usize)
            * picture.image_header.height_align as usize;

        let mut encoder = png::Encoder::new(&mut ow, iw as u32, ih as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().context("Failed to write PNG header")?;

        let mut out = vec![0u8; iw * ih * 4];
        if order == gim::ImageOrder::PSPImage && !args.linear {
            // read as 4 x 8 tiles and convert to linear output
            let tw = if args.tx > 0 { args.tx } else { 4 };
            let th = if args.ty > 0 { args.ty } else { 8 };
            let tiles_x = iw / tw;
            let tiles_y = ih / th;

            for ty in 0..tiles_y {
                for tx in 0..tiles_x {
                    let tile_index = ty * tiles_x + tx;
                    let tile_offset = tile_index * tw * th;

                    for y in 0..th {
                        for x in 0..tw {
                            let src = (tile_offset + y * tw + x) * 4;

                            // Convert tile coords -> image coords
                            let px = tx * tw + x;
                            let py = ty * th + y;
                            let dst = (py * iw + px) * 4;

                            out[dst] = picture.image_data[src];
                            out[dst + 1] = picture.image_data[src + 1];
                            out[dst + 2] = picture.image_data[src + 2];
                            out[dst + 3] = picture.image_data[src + 3];
                        }
                    }
                }
            }
        } else {
            //linear image data
            let mut out = vec![0u8; iw * ih * 4];
            for y in 0..ih {
                for x in 0..iw {
                    let src = (y * iw + x) * 4;
                    let dst = (y * iw + x) * 4;

                    out[dst] = picture.image_data[src];
                    out[dst + 1] = picture.image_data[src + 1];
                    out[dst + 2] = picture.image_data[src + 2];
                    out[dst + 3] = picture.image_data[src + 3];
                }
            }
        }

        writer.write_image_data(&out).context("Failed to write PNG data")?;
        println!("Extracted texture file: {}", output_file_name);
    } else if format == gim::ImageFormat::INDEX8 {
        if let Some(palette) = picture.palette_header
            && let Some(raw_pal_data) = picture.palette_data
        {
            let pal_data = convert_palette_for_png(&palette, raw_pal_data)?;

            let output_file_name = format!("{}.png", input_name);
            println!("Writing output file: {}", output_file_name);
            let mut ow = std::io::BufWriter::new(std::fs::File::create(&output_file_name).context("Failed to create output file")?);

            //the data is aligned by these parameters from the header
            let iw = (picture.image_header.width as usize).div_ceil(picture.image_header.pitch_align as usize)
                * picture.image_header.pitch_align as usize;
            let ih = (picture.image_header.height as usize).div_ceil(picture.image_header.height_align as usize)
                * picture.image_header.height_align as usize;

            let mut encoder = png::Encoder::new(&mut ow, iw as u32, ih as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("Failed to write PNG header");

            let mut out = vec![0u8; iw * ih * 4];
            if order == gim::ImageOrder::PSPImage && !args.linear {
                // read as 16 x 8 tiles and convert to linear output
                let tw = if args.tx > 0 { args.tx } else { 16 };
                let th = if args.ty > 0 { args.ty } else { 8 };
                let tiles_x = iw / tw;
                let tiles_y = ih / th;

                for ty in 0..tiles_y {
                    for tx in 0..tiles_x {
                        let tile_index = ty * tiles_x + tx;
                        let tile_offset = tile_index * tw * th;

                        for y in 0..th {
                            for x in 0..tw {
                                let src = tile_offset + y * tw + x; // palette index

                                // Convert tile coords -> image coords
                                let px = tx * tw + x;
                                let py = ty * th + y;
                                let dst = (py * iw + px) * 4;

                                let pal_offset = (picture.image_data[src] as usize) * 4;

                                out[dst] = pal_data[pal_offset + 0];
                                out[dst + 1] = pal_data[pal_offset + 1];
                                out[dst + 2] = pal_data[pal_offset + 2];
                                out[dst + 3] = pal_data[pal_offset + 3];
                            }
                        }
                    }
                }
            } else {
                //linear image data
                for y in 0..ih {
                    for x in 0..iw {
                        let src = y * iw + x; // palette index
                        let dst = (y * iw + x) * 4;

                        let pal_offset = (picture.image_data[src] as usize) * 4;

                        out[dst] = pal_data[pal_offset + 0];
                        out[dst + 1] = pal_data[pal_offset + 1];
                        out[dst + 2] = pal_data[pal_offset + 2];
                        out[dst + 3] = pal_data[pal_offset + 3];
                    }
                }
            }

            writer.write_image_data(&out).context("Failed to write PNG data")?;
            println!("Extracted texture file: {}", output_file_name);
        } else {
            bail!("Error: GIM Image Format has no understood palette.");
        }
    } else {
        bail!("Error: GIM Image Format '{}' not supported for conversion.", format);
    }
    Ok(())
}

fn convert_palette_for_png<'a>(palette_header: &gim::GimImageHeader, palette_data: &'a [u8]) -> Result<Cow<'a, [u8]>> {
    let format = palette_header.image_format().context("Failed to get palette image format")?;

    match format {
        gim::ImageFormat::RGBA8888 => {
            return Ok(Cow::Borrowed(palette_data));
        }
        gim::ImageFormat::RGBA5551 => {
            let mut out = vec![0u8; 256 * 4];

            for i in 0..256 {
                let src_offset = i * 2;
                let dst_offset = i * 4;
                let pix_low = palette_data[src_offset];
                let pix_high = palette_data[src_offset + 1];
                let pix = ((pix_high as u16) << 8) | (pix_low as u16);

                let b = (((pix >> 10) & 0x1F) << 3) as u8;
                let g = (((pix >> 5) & 0x1F) << 3) as u8;
                let r = ((pix & 0x1F) << 3) as u8;
                let a = if (pix & 0x8000) != 0 { 255 } else { 0 };

                out[dst_offset] = r;
                out[dst_offset + 1] = g;
                out[dst_offset + 2] = b;
                out[dst_offset + 3] = a;
            }
            return Ok(Cow::Owned(out));
        }
        _ => {
            bail!("Error: GIM Palette format '{}' not supported for conversion.", format);
        }
    }
}
