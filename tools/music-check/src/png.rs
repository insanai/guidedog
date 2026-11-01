//! Decode the RGBA8 corpus PNGs and compare composited pixels with a small tolerance.

pub(crate) fn compare(expected: &[u8], actual: &[u8]) -> Result<(), String> {
    check_dpi(expected)?;
    check_dpi(actual)?;
    let (width, height, expected) = decode(expected)?;
    let (other_width, other_height, actual) = decode(actual)?;
    if (width, height) != (other_width, other_height) {
        return Err("PNG dimensions changed.".into());
    }
    let mut changed = 0;
    let mut error = 0u64;
    for (before, after) in expected.as_chunks::<4>().0.iter().zip(actual.as_chunks::<4>().0) {
        let mut different = false;
        for channel in 0..3 {
            let composite =
                |pixel: &[u8]| 255 - (255 - u16::from(pixel[channel])) * u16::from(pixel[3]) / 255;
            let difference = composite(before).abs_diff(composite(after));
            different |= difference > 8;
            error += u64::from(difference);
        }
        changed += usize::from(different);
    }
    let pixels = width * height;
    if changed * 1000 > pixels * 2 || error * 100 > pixels as u64 * 3 * 255 {
        return Err(format!("PNG pixels changed: {changed}/{pixels} exceed tolerance."));
    }
    Ok(())
}

pub(crate) fn decode(png: &[u8]) -> Result<(usize, usize, Vec<u8>), String> {
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("Expected PNG.".into());
    }
    let (mut width, mut height) = (0, 0);
    let mut packed = Vec::new();
    let mut at = 8usize;
    while let Some(header) = png.get(at..at.saturating_add(8)) {
        let length = number(&header[..4])?;
        let data = png.get(at + 8..at.saturating_add(8 + length)).ok_or("Truncated PNG.")?;
        match &header[4..8] {
            b"IHDR" if data.len() == 13 && data[8..] == [8, 6, 0, 0, 0] => {
                width = number(&data[..4])?;
                height = number(&data[4..8])?;
            }
            b"IDAT" => packed.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        at = at.checked_add(12 + length).ok_or("PNG size overflow.")?;
    }
    let size = width.checked_mul(height).filter(|n| *n <= 16_000_000).ok_or("PNG pixel limit.")?;
    let stride = width * 4;
    let filtered =
        gd_flate::zlib_decompress(&packed, size * 4 + height).map_err(|e| e.to_string())?;
    if width == 0 || filtered.len() != (stride + 1) * height {
        return Err("Invalid RGBA8 PNG.".into());
    }
    let mut pixels = vec![0u8; size * 4];
    for row in 0..height {
        let filter = filtered[row * (stride + 1)];
        for column in 0..stride {
            let at = row * stride + column;
            let left = if column >= 4 { pixels[at - 4] } else { 0 };
            let up = if row > 0 { pixels[at - stride] } else { 0 };
            let corner = if row > 0 && column >= 4 { pixels[at - stride - 4] } else { 0 };
            let predictor = match filter {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                4 => paeth(left, up, corner),
                _ => return Err("Unknown PNG filter.".into()),
            };
            pixels[at] = filtered[row * (stride + 1) + column + 1].wrapping_add(predictor);
        }
    }
    Ok((width, height, pixels))
}

pub(crate) fn check_dpi(png: &[u8]) -> Result<(), String> {
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("Expected PNG.".into());
    }
    let mut at = 8usize;
    while let Some(header) = png.get(at..at.saturating_add(8)) {
        let length = number(&header[..4])?;
        let data = png.get(at + 8..at.saturating_add(8 + length)).ok_or("Truncated PNG.")?;
        if &header[4..8] == b"pHYs" {
            if data.len() == 9
                && number(&data[..4])? == 5669
                && number(&data[4..8])? == 5669
                && data[8] == 1
            {
                return Ok(());
            }
            return Err("PNG pHYs chunk does not record 144 DPI (5669 px/m).".into());
        }
        if &header[4..8] == b"IEND" {
            break;
        }
        at = at.checked_add(12 + length).ok_or("PNG size overflow.")?;
    }
    Err("PNG is missing its 144 DPI pHYs chunk.".into())
}

fn number(bytes: &[u8]) -> Result<usize, String> {
    Ok(u32::from_be_bytes(bytes.try_into().map_err(|_| "Missing PNG integer.")?) as usize)
}

fn paeth(left: u8, up: u8, corner: u8) -> u8 {
    let (a, b, c) = (i32::from(left), i32::from(up), i32::from(corner));
    let p = a + b - c;
    let (da, db, dc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
    if da <= db && da <= dc {
        left
    } else if db <= dc {
        up
    } else {
        corner
    }
}
