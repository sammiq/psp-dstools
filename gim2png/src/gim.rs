use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use core::mem;

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod, Debug)]
struct GimHeader {
    signature: u32,
    version: u32,
    style: u32,
    option: u32,
}

const GIM_FORMAT_SIGNATURE: u32 = 0x2e47494d; /* '.GIM' */
const GIM_FORMAT_VERSION: u32 = 0x312e3030; /* '1.00' */
const GIM_FORMAT_STYLE_PSP: u32 = 0x00505350; /* 'PSP'  */

#[repr(C)]
#[derive(Clone, Copy, Debug, Zeroable, Pod)]
struct GimChunk {
    chunk_type: u16,
    unused: u16,
    next_offs: u32,  //relative
    child_offs: u32, //relative
    data_offs: u32,  //relative
}

#[allow(dead_code)]
const SCEGIM_BLOCK: u16 = 0x0001;
#[allow(dead_code)]
const SCEGIM_FILE: u16 = 0x0002;
const SCEGIM_PICTURE: u16 = 0x0003;
const SCEGIM_IMAGE: u16 = 0x0004;
const SCEGIM_PALETTE: u16 = 0x0005;
#[allow(dead_code)]
const SCEGIM_SEQUENCE: u16 = 0x0006;
#[allow(dead_code)]
const SCEGIM_FILE_INFO: u16 = 0x00ff;

#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
pub struct GimImageHeader {
    pub header_size: u16,
    pub reference: u16,
    pub format: u16,
    pub order: u16,
    pub width: u16,
    pub height: u16,
    pub bpp: u16,
    pub pitch_align: u16,
    pub height_align: u16,
    pub dim_count: u16,
    pub reserved: u16,
    pub reserved2: u16,
    pub offsets: u32,
    pub images: u32,
    pub total: u32,
    pub plane_mask: u32,
    pub level_type: u16,
    pub level_count: u16,
    pub frame_type: u16,
    pub frame_count: u16,
}

impl GimImageHeader {
    pub fn image_format(&self) -> Option<ImageFormat> {
        self.format.try_into().ok()
    }

    pub fn image_order(&self) -> Option<ImageOrder> {
        self.order.try_into().ok()
    }
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    RGBA5650 = 0,
    RGBA5551 = 1,
    RGBA4444 = 2,
    RGBA8888 = 3,
    INDEX4 = 4,
    INDEX8 = 5,
    INDEX16 = 6,
    INDEX32 = 7,
    DXT1 = 8,
    DXT3 = 9,
    DXT5 = 10,
    DXT1EXT = 264,
    DXT3EXT = 265,
    DXT5EXT = 266,
}

impl TryFrom<u16> for ImageFormat {
    type Error = &'static str;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ImageFormat::RGBA5650),
            1 => Ok(ImageFormat::RGBA5551),
            2 => Ok(ImageFormat::RGBA4444),
            3 => Ok(ImageFormat::RGBA8888),
            4 => Ok(ImageFormat::INDEX4),
            5 => Ok(ImageFormat::INDEX8),
            6 => Ok(ImageFormat::INDEX16),
            7 => Ok(ImageFormat::INDEX32),
            8 => Ok(ImageFormat::DXT1),
            9 => Ok(ImageFormat::DXT3),
            10 => Ok(ImageFormat::DXT5),
            264 => Ok(ImageFormat::DXT1EXT),
            265 => Ok(ImageFormat::DXT3EXT),
            266 => Ok(ImageFormat::DXT5EXT),
            _ => Err("Invalid enum value"),
        }
    }
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            ImageFormat::RGBA5650 => "RGBA5650",
            ImageFormat::RGBA5551 => "RGBA5551",
            ImageFormat::RGBA4444 => "RGBA4444",
            ImageFormat::RGBA8888 => "RGBA8888",
            ImageFormat::INDEX4 => "INDEX4",
            ImageFormat::INDEX8 => "INDEX8",
            ImageFormat::INDEX16 => "INDEX16",
            ImageFormat::INDEX32 => "INDEX32",
            ImageFormat::DXT1 => "DXT1",
            ImageFormat::DXT3 => "DXT3",
            ImageFormat::DXT5 => "DXT5",
            ImageFormat::DXT1EXT => "DXT1EXT",
            ImageFormat::DXT3EXT => "DXT3EXT",
            ImageFormat::DXT5EXT => "DXT5EXT",
        };
        write!(f, "{}", name)
    }
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageOrder {
    Normal = 0,
    PSPImage = 1,
}

impl TryFrom<u16> for ImageOrder {
    type Error = &'static str;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ImageOrder::Normal),
            1 => Ok(ImageOrder::PSPImage),
            _ => Err("Invalid enum value"),
        }
    }
}

impl std::fmt::Display for ImageOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            ImageOrder::Normal => "Normal",
            ImageOrder::PSPImage => "PSPImage",
        };
        write!(f, "{}", name)
    }
}

fn gim_picture_check_file_header(buffer: &[u8]) -> Result<()> {
    let header = bytemuck::try_from_bytes::<GimHeader>(&buffer[0..core::mem::size_of::<GimHeader>()])
        .map_err(|e| anyhow::anyhow!(e))
        .context("Failed to read GIM header")?;

    if header.signature != GIM_FORMAT_SIGNATURE {
        anyhow::bail!("Invalid GIM signature");
    }
    if header.version != GIM_FORMAT_VERSION {
        anyhow::bail!("Unsupported GIM version");
    }
    if header.style != GIM_FORMAT_STYLE_PSP {
        anyhow::bail!("Unsupported GIM style");
    }

    Ok(())
}

fn gim_picture_get_chunk_header(bytes: &[u8], start: usize) -> Result<&GimChunk> {
    let end = start + mem::size_of::<GimChunk>();
    let root_chunk = bytemuck::try_from_bytes::<GimChunk>(&bytes[start..end])
        .map_err(|e| anyhow::anyhow!(e))
        .context("Failed to read GIM chunk header")?;
    Ok(root_chunk)
}

fn gim_get_child_chunk<'a>(
    buffer: &'a [u8],
    start_offset: usize,
    parent_chunk: &GimChunk,
    chunk_type: u16,
) -> Result<Option<(&'a GimChunk, usize)>> {
    let chunk_end = start_offset + parent_chunk.next_offs as usize;
    let mut child_offs = start_offset + parent_chunk.child_offs as usize;
    let mut found_chunk = None;
    while child_offs < chunk_end {
        //this needs to be relative
        let child_chunk = gim_picture_get_chunk_header(&buffer, child_offs).context("child chunk should be valid")?;
        //println!("{:?}", child_chunk);
        if child_chunk.chunk_type == chunk_type {
            found_chunk = Some((child_chunk, child_offs));
        }
        child_offs += child_chunk.next_offs as usize;
    }
    Ok(found_chunk)
}

/// Iterates over all child chunks of a parent, calling the callback for each child.
/// The callback receives (&GimChunk, offset) and can return a Result.
/// If the callback returns an error, iteration stops and the error is returned.
fn gim_process_child_chunks<'a, F>(buffer: &'a [u8], start_offset: usize, parent_chunk: &GimChunk, mut callback: F) -> Result<()>
where
    F: FnMut(&'a GimChunk, usize) -> Result<()>,
{
    let chunk_end = start_offset + parent_chunk.next_offs as usize;
    let mut child_offs = start_offset + parent_chunk.child_offs as usize;
    while child_offs < chunk_end {
        let child_chunk = gim_picture_get_chunk_header(&buffer, child_offs).context("child chunk should be valid")?;
        callback(child_chunk, child_offs)?;
        child_offs += child_chunk.next_offs as usize;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct GimPicture<'a> {
    pub image_header: &'a GimImageHeader,
    pub image_offsets: &'a [u32],
    pub image_data: &'a [u8],
    pub palette_header: Option<&'a GimImageHeader>,
    pub palette_offsets: Option<&'a [u32]>,
    pub palette_data: Option<&'a [u8]>,
}

pub fn load_gim_image<'a>(buffer: &'a[u8]) -> Result<GimPicture<'a>> {
    gim_picture_check_file_header(buffer)?;

    let start_offset = mem::size_of::<GimHeader>();
    let root_chunk = gim_picture_get_chunk_header(buffer, start_offset)?;
    //println!("{:?}", root_chunk);

    //look for a child chunk that is a picture
    let mut image_header = None;
    let mut image_offsets = None;
    let mut image_data = None;
    let mut palette_header = None;
    let mut palette_offsets = None;
    let mut palette_data = None;
    match gim_get_child_chunk(buffer, start_offset, root_chunk, SCEGIM_PICTURE)? {
        Some((chunk, offset)) => {
            gim_process_child_chunks(buffer, offset, chunk, |child_chunk, child_offset| {
                //println!("Found child chunk: {:?} at offset {}", child_chunk, child_offset);
                match child_chunk.chunk_type {
                    SCEGIM_IMAGE => {
                        let header_offset = child_offset + child_chunk.data_offs as usize;
                        let header = bytemuck::try_from_bytes::<GimImageHeader>(
                            &buffer[header_offset..header_offset + mem::size_of::<GimImageHeader>()],
                        )
                        .map_err(|e| anyhow::anyhow!(e))
                        .context("Failed to read GIM image header")?;

                        //println!("Found image header: {:?}", header);
                        //let format_string = header.format.try_into().map_or("unknown".to_string(), |f: ImageFormat| f.to_string());
                        //println!("Found image format: {:?}", format_string);
                        //let order_string = header.order.try_into().map_or("unknown".to_string(), |o: ImageOrder| o.to_string());
                        //println!("Found image order: {:?}", order_string);

                        let offsets_size = (header.level_count as usize * header.frame_count as usize) * mem::size_of::<u32>();
                        let offsets_offset = header_offset + header.offsets as usize;
                        let slice: &[u32] = bytemuck::try_cast_slice(&buffer[offsets_offset..offsets_offset + offsets_size])
                            .map_err(|e| anyhow::anyhow!(e))
                            .context("Failed to read GIM image offsets")?;
                        //println!("{:?}", slice);
                        image_header = Some(header);
                        image_offsets = Some(slice);
                        let images_start = header_offset + header.images as usize;
                        let images_end = header_offset + header.total as usize;
                        image_data = Some(&buffer[images_start..images_end]);
                    }
                    SCEGIM_PALETTE => {
                        let header_offset = child_offset + child_chunk.data_offs as usize;
                        let header = bytemuck::try_from_bytes::<GimImageHeader>(
                            &buffer[header_offset..header_offset + mem::size_of::<GimImageHeader>()],
                        )
                        .map_err(|e| anyhow::anyhow!(e))
                        .context("Failed to read GIM image header")?;

                        //println!("Found image header: {:?}", header);
                        //let format_string = header.format.try_into().map_or("unknown".to_string(), |f: ImageFormat| f.to_string());
                        //println!("Found image format: {:?}", format_string);
                        //let order_string = header.order.try_into().map_or("unknown".to_string(), |o: ImageOrder| o.to_string());
                        //println!("Found image order: {:?}", order_string);

                        let offsets_size = (header.level_count as usize * header.frame_count as usize) * mem::size_of::<u32>();
                        let offsets_offset = header_offset + header.offsets as usize;
                        let slice: &[u32] = bytemuck::try_cast_slice(&buffer[offsets_offset..offsets_offset + offsets_size])
                            .map_err(|e| anyhow::anyhow!(e))
                            .context("Failed to read GIM image offsets")?;
                        //println!("{:?}", slice);
                        palette_header = Some(header);
                        palette_offsets = Some(slice);
                        let palette_start = header_offset + header.images as usize;
                        let palette_end = header_offset + header.total as usize;
                        palette_data = Some(&buffer[palette_start..palette_end]);
                    }
                    _ => {
                        anyhow::bail!("Unsupported child chunk type: {}", child_chunk.chunk_type);
                    }
                }
                Ok(())
            })?;
        }
        None => {
            anyhow::bail!("Picture chunk not found");
        }
    }

    Ok(GimPicture {
        image_header: image_header.ok_or_else(|| anyhow::anyhow!("Image header not found"))?,
        image_offsets: image_offsets.ok_or_else(|| anyhow::anyhow!("Image offsets not found"))?,
        image_data: image_data.ok_or_else(|| anyhow::anyhow!("Image data not found"))?,
        palette_header,
        palette_offsets,
        palette_data,
    })
}
