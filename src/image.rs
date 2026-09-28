#[derive(Clone, Debug, PartialEq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub channels: usize,
    pub data: Vec<u8>,
}

const PNG_SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

pub fn is_png(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && bytes[..8] == PNG_SIG
}

pub fn is_jpeg(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF
}

pub fn decode(bytes: &[u8]) -> Result<Raster, String> {
    if is_jpeg(bytes) {
        return Err("JPEG data is passed through to PDF verbatim as a DCTDecode stream and is never rasterised".into());
    }
    if is_png(bytes) {
        return decode_png(bytes);
    }
    if bytes.is_empty() {
        return Err("empty image file".into());
    }
    Err("unsupported image format: the data is neither PNG nor JPEG".into())
}

pub struct JpegInfo {
    pub width: u32,
    pub height: u32,
    pub components: u8,
    pub progressive: bool,
}

pub fn jpeg_size(bytes: &[u8]) -> Option<(u32, u32)> {
    jpeg_info(bytes).map(|i| (i.width, i.height))
}

pub fn jpeg_info(bytes: &[u8]) -> Option<JpegInfo> {
    if !is_jpeg(bytes) {
        return None;
    }
    let mut i = 2usize;
    while i + 1 < bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < bytes.len() && bytes[j] == 0xFF {
            j += 1;
        }
        if j >= bytes.len() {
            break;
        }
        let marker = bytes[j];
        i = j + 1;
        if marker == 0x01 || marker == 0xD8 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        if marker == 0xD9 {
            break;
        }
        if i + 1 >= bytes.len() {
            break;
        }
        let len = (((bytes[i] as usize) << 8) | bytes[i + 1] as usize).max(2);
        if is_sof(marker) {
            if i + 7 >= bytes.len() {
                return None;
            }
            return Some(JpegInfo {
                height: be16(&bytes[i + 3..i + 5]),
                width: be16(&bytes[i + 5..i + 7]),
                components: bytes[i + 7],
                progressive: marker == 0xC2,
            });
        }
        i += len;
    }
    None
}

fn is_sof(marker: u8) -> bool {
    matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC)
}

fn be16(b: &[u8]) -> u32 {
    ((b[0] as u32) << 8) | b[1] as u32
}

fn be32(b: &[u8]) -> u32 {
    ((b[0] as u32) << 24) | ((b[1] as u32) << 16) | ((b[2] as u32) << 8) | b[3] as u32
}

fn decode_png(bytes: &[u8]) -> Result<Raster, String> {
    if bytes.len() < 8 {
        return Err("png file is truncated".into());
    }
    let mut pos = 8usize;
    let (mut width, mut height) = (0u32, 0u32);
    let (mut depth, mut ctype, mut interlace) = (0u8, 0u8, 0u8);
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut trns: Vec<u8> = Vec::new();
    let mut idat: Vec<u8> = Vec::new();
    let mut seen_ihdr = false;
    while pos + 8 <= bytes.len() {
        let len = be32(&bytes[pos..pos + 4]) as usize;
        let ty = &bytes[pos + 4..pos + 8];
        let start = pos + 8;
        if start + len + 4 > bytes.len() {
            return Err(format!("png chunk {} is truncated", String::from_utf8_lossy(ty)));
        }
        let data = &bytes[start..start + len];
        let crc = be32(&bytes[start + len..start + len + 4]);
        if crc32(ty, data) != crc {
            return Err(format!("png chunk {} failed its CRC check", String::from_utf8_lossy(ty)));
        }
        match ty {
            b"IHDR" => {
                if len < 13 {
                    return Err("png IHDR chunk is too short".into());
                }
                width = be32(&data[0..4]);
                height = be32(&data[4..8]);
                depth = data[8];
                ctype = data[9];
                if data[10] != 0 {
                    return Err("unsupported png compression method".into());
                }
                if data[11] != 0 {
                    return Err("unsupported png filter method".into());
                }
                interlace = data[12];
                seen_ihdr = true;
            }
            b"PLTE" => {
                palette = data.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
            }
            b"tRNS" => trns = data.to_vec(),
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        pos = start + len + 4;
    }
    if !seen_ihdr {
        return Err("png has no IHDR chunk".into());
    }
    if width == 0 || height == 0 {
        return Err("png has a zero dimension".into());
    }
    if interlace != 0 {
        return Err("interlaced (Adam7) PNGs are not supported".into());
    }
    if depth != 8 && depth != 16 {
        return Err(format!("unsupported png bit depth {}", depth));
    }
    let src_channels: usize = match ctype {
        0 => 1,
        2 => 3,
        3 => 1,
        4 => 2,
        6 => 4,
        other => return Err(format!("unsupported png colour type {}", other)),
    };
    if ctype == 3 && palette.is_empty() {
        return Err("indexed png without a PLTE chunk".into());
    }
    if idat.is_empty() {
        return Err("png has no image data".into());
    }
    let raw = inflate(&idat)?;
    let bits_per_pixel = src_channels * depth as usize;
    let bpp = ((bits_per_pixel + 7) / 8).max(1);
    let stride = (width as usize * bits_per_pixel + 7) / 8;
    let need = height as usize * (stride + 1);
    if raw.len() < need {
        return Err(format!("png image data is short: {} of {} bytes", raw.len(), need));
    }
    let mut lines: Vec<u8> = vec![0; height as usize * stride];
    let mut prev = vec![0u8; stride];
    let mut off = 0usize;
    for row in 0..height as usize {
        let filter = raw[off];
        off += 1;
        let src = &raw[off..off + stride];
        off += stride;
        let at = row * stride;
        let dst = &mut lines[at..at + stride];
        match filter {
            0 => dst.copy_from_slice(src),
            1 => {
                for i in 0..stride {
                    let a = if i >= bpp { dst[i - bpp] } else { 0 };
                    dst[i] = src[i].wrapping_add(a);
                }
            }
            2 => {
                for i in 0..stride {
                    dst[i] = src[i].wrapping_add(prev[i]);
                }
            }
            3 => {
                for i in 0..stride {
                    let a = if i >= bpp { dst[i - bpp] as u32 } else { 0 };
                    dst[i] = src[i].wrapping_add(((a + prev[i] as u32) / 2) as u8);
                }
            }
            4 => {
                for i in 0..stride {
                    let a = if i >= bpp { dst[i - bpp] as i32 } else { 0 };
                    let b = prev[i] as i32;
                    let c = if i >= bpp { prev[i - bpp] as i32 } else { 0 };
                    let p = a + b - c;
                    let pa = (p - a).abs();
                    let pb = (p - b).abs();
                    let pc = (p - c).abs();
                    let pred = if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    };
                    dst[i] = src[i].wrapping_add(pred as u8);
                }
            }
            other => return Err(format!("unknown png scanline filter {}", other)),
        }
        prev.copy_from_slice(dst);
    }
    let pixels = width as usize * height as usize;
    let step = if depth == 16 { 2usize } else { 1 };
    let mut samples: Vec<u8> = Vec::with_capacity(pixels * src_channels);
    for i in 0..pixels * src_channels {
        samples.push(lines[i * step]);
    }
    let mut out: Vec<u8> = Vec::with_capacity(pixels * 4);
    let mut alpha = false;
    match ctype {
        0 => {
            if trns.len() >= 2 {
                alpha = true;
                let key = trns[0];
                for s in &samples {
                    out.push(*s);
                    out.push(if *s == key { 0 } else { 255 });
                }
            } else {
                out.extend_from_slice(&samples);
            }
        }
        2 => {
            if trns.len() >= 6 {
                alpha = true;
                let key = &trns[0..3];
                for px in samples.chunks_exact(3) {
                    out.extend_from_slice(px);
                    out.push(if px == key { 0 } else { 255 });
                }
            } else {
                out.extend_from_slice(&samples);
            }
        }
        3 => {
            let mut has = false;
            for idx in &samples {
                let entry = palette.get(*idx as usize).copied().unwrap_or([0, 0, 0]);
                out.extend_from_slice(&entry);
                if !trns.is_empty() {
                    let a = trns.get(*idx as usize).copied().unwrap_or(255);
                    has = has || a != 255;
                    out.push(a);
                }
            }
            alpha = has;
        }
        4 => {
            let mut opaque = true;
            for px in samples.chunks_exact(2) {
                out.push(px[0]);
                if px[1] != 255 {
                    opaque = false;
                }
                out.push(px[1]);
            }
            if opaque {
                let mut g = Vec::with_capacity(out.len() / 2);
                let mut i = 0;
                while i < out.len() {
                    g.push(out[i]);
                    i += 2;
                }
                out = g;
            } else {
                alpha = true;
            }
        }
        _ => {
            let mut opaque = true;
            for px in samples.chunks_exact(4) {
                if px[3] != 255 {
                    opaque = false;
                    break;
                }
            }
            if opaque {
                for px in samples.chunks_exact(4) {
                    out.extend_from_slice(&px[0..3]);
                }
            } else {
                out.extend_from_slice(&samples);
                alpha = true;
            }
        }
    }
    let channels = if alpha {
        4
    } else if ctype == 0 || ctype == 4 {
        1
    } else {
        3
    };
    if pixels > 0 && out.len() != pixels * channels {
        return Err("internal png channel normalisation error".into());
    }
    Ok(Raster { width, height, channels, data: out })
}

pub fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 2 {
        return Err("zlib stream is too short".into());
    }
    let cmf = data[0];
    let flg = data[1];
    if cmf & 0x0F != 8 {
        return Err(format!("unsupported zlib compression method {}", cmf & 0x0F));
    }
    if ((cmf as u16) << 8 | flg as u16) % 31 != 0 {
        return Err("zlib header check failed".into());
    }
    if flg & 0x20 != 0 {
        return Err("zlib preset dictionaries are not supported".into());
    }
    if flg & 0x80 != 0 {
        return Err("zlib streams carrying a gzip wrapper are not supported".into());
    }
    let mut br = Bits::new(&data[2..]);
    let out = inflate_blocks(&mut br)?;
    br.align();
    if br.remaining() >= 4 {
        let tail = br.take(4);
        let want = ((tail[0] as u32) << 24) | ((tail[1] as u32) << 16) | ((tail[2] as u32) << 8) | tail[3] as u32;
        let got = adler32(&out);
        if want != got {
            return Err(format!("zlib adler32 mismatch: header {:08x}, computed {:08x}", want, got));
        }
    }
    Ok(out)
}

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
    buf: u32,
    cnt: u32,
}

impl<'a> Bits<'a> {
    fn new(d: &'a [u8]) -> Bits<'a> {
        Bits { d, pos: 0, buf: 0, cnt: 0 }
    }

    fn remaining(&self) -> usize {
        self.d.len() - self.pos
    }

    fn take(&mut self, n: usize) -> Vec<u8> {
        let n = n.min(self.remaining());
        let v = self.d[self.pos..self.pos + n].to_vec();
        self.pos += n;
        v
    }

    fn bits(&mut self, n: u32) -> Result<u32, String> {
        if n == 0 {
            return Ok(0);
        }
        while self.cnt < n {
            let b = match self.d.get(self.pos) {
                Some(b) => *b,
                None => return Err("deflate stream ended unexpectedly".into()),
            };
            self.pos += 1;
            self.buf |= (b as u32) << self.cnt;
            self.cnt += 8;
        }
        let v = self.buf & ((1u32 << n) - 1);
        self.buf >>= n;
        self.cnt -= n;
        Ok(v)
    }

    fn align(&mut self) {
        let drop = self.cnt % 8;
        self.buf >>= drop;
        self.cnt -= drop;
    }
}

struct Huff {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huff {
    fn build(lengths: &[u8]) -> Huff {
        let mut counts = [0u16; 16];
        for l in lengths {
            let idx = *l as usize;
            if idx < 16 {
                counts[idx] += 1;
            }
        }
        counts[0] = 0;
        let mut offs = [0u16; 16];
        for i in 1..16 {
            offs[i] = offs[i - 1] + counts[i - 1];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (s, l) in lengths.iter().enumerate() {
            let idx = *l as usize;
            if idx > 0 && idx < 16 {
                let slot = offs[idx] as usize;
                if slot < symbols.len() {
                    symbols[slot] = s as u16;
                    offs[idx] += 1;
                }
            }
        }
        Huff { counts, symbols }
    }

    fn decode(&self, br: &mut Bits) -> Result<u16, String> {
        let mut code = 0i32;
        let mut first = 0i32;
        let mut index = 0i32;
        for len in 1..16 {
            code |= br.bits(1)? as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                let slot = index + (code - first);
                return match self.symbols.get(slot as usize) {
                    Some(v) => Ok(*v),
                    None => Err("invalid huffman symbol in deflate stream".into()),
                };
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err("invalid huffman code in deflate stream".into())
    }
}

const LBASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258,
];
const LEXT: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DBASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
    8193, 12289, 16385, 24577,
];
const DEXT: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13,
];

fn fixed_tables() -> (Huff, Huff) {
    let mut lit = [0u8; 288];
    for (i, l) in lit.iter_mut().enumerate() {
        *l = if i < 144 {
            8
        } else if i < 256 {
            9
        } else if i < 280 {
            7
        } else {
            8
        };
    }
    (Huff::build(&lit), Huff::build(&[5u8; 30]))
}

fn dynamic_tables(br: &mut Bits) -> Result<(Huff, Huff), String> {
    const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
    let hlit = br.bits(5)? as usize + 257;
    let hdist = br.bits(5)? as usize + 1;
    let hclen = br.bits(4)? as usize + 4;
    let mut cl = [0u8; 19];
    for i in 0..hclen.min(19) {
        cl[ORDER[i]] = br.bits(3)? as u8;
    }
    let clh = Huff::build(&cl);
    let mut lengths = vec![0u8; hlit + hdist];
    let mut i = 0usize;
    while i < lengths.len() {
        let sym = clh.decode(br)?;
        match sym {
            0..=15 => {
                lengths[i] = sym as u8;
                i += 1;
            }
            16 => {
                if i == 0 {
                    return Err("deflate repeat with no previous code length".into());
                }
                let prev = lengths[i - 1];
                let n = 3 + br.bits(2)? as usize;
                if i + n > lengths.len() {
                    return Err("deflate length repeat overflows the code table".into());
                }
                for _ in 0..n {
                    lengths[i] = prev;
                    i += 1;
                }
            }
            17 | 18 => {
                let n = if sym == 17 { 3 + br.bits(3)? as usize } else { 11 + br.bits(7)? as usize };
                if i + n > lengths.len() {
                    return Err("deflate zero repeat overflows the code table".into());
                }
                i += n;
            }
            other => return Err(format!("invalid code length symbol {}", other)),
        }
    }
    Ok((Huff::build(&lengths[..hlit]), Huff::build(&lengths[hlit..])))
}

fn inflate_blocks(br: &mut Bits) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::new();
    loop {
        let last = br.bits(1)?;
        let btype = br.bits(2)?;
        match btype {
            0 => {
                br.align();
                let len = br.bits(16)? as usize;
                let nlen = br.bits(16)? as usize;
                if len != (!nlen & 0xFFFF) {
                    return Err("stored deflate block has a bad length pair".into());
                }
                for _ in 0..len {
                    out.push(br.bits(8)? as u8);
                }
            }
            1 | 2 => {
                let (lit, dist) = if btype == 1 { fixed_tables() } else { dynamic_tables(br)? };
                loop {
                    let sym = lit.decode(br)? as usize;
                    if sym < 256 {
                        out.push(sym as u8);
                    } else if sym == 256 {
                        break;
                    } else {
                        let idx = sym - 257;
                        if idx >= 29 {
                            return Err(format!("invalid length symbol {}", sym));
                        }
                        let len = LBASE[idx] as usize + br.bits(LEXT[idx] as u32)? as usize;
                        let ds = dist.decode(br)? as usize;
                        if ds >= 30 {
                            return Err(format!("invalid distance symbol {}", ds));
                        }
                        let d = DBASE[ds] as usize + br.bits(DEXT[ds] as u32)? as usize;
                        if d > out.len() || d == 0 {
                            return Err("deflate back-reference points before the start of the output".into());
                        }
                        let mut src = out.len() - d;
                        for _ in 0..len {
                            let b = out[src];
                            out.push(b);
                            src += 1;
                        }
                    }
                }
            }
            other => return Err(format!("invalid deflate block type {}", other)),
        }
        if last == 1 {
            break;
        }
    }
    Ok(out)
}

fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for chunk in data.chunks(5552) {
        for byte in chunk {
            a += *byte as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

fn crc32(kind: &[u8], data: &[u8]) -> u32 {
    let mut c: u32 = 0xFFFF_FFFF;
    for byte in kind.iter().chain(data.iter()) {
        c ^= *byte as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    !c
}

#[cfg(test)]
pub fn test_crc32(kind: &[u8], data: &[u8]) -> u32 {
    crc32(kind, data)
}

#[cfg(test)]
pub fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78u8, 0x01];
    let mut i = 0usize;
    if data.is_empty() {
        out.push(0x01);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(!0u16).to_le_bytes());
    }
    while i < data.len() {
        let n = (data.len() - i).min(65535);
        let last = if i + n >= data.len() { 1u8 } else { 0u8 };
        out.push(last);
        out.extend_from_slice(&(n as u16).to_le_bytes());
        out.extend_from_slice(&(!(n as u16)).to_le_bytes());
        out.extend_from_slice(&data[i..i + n]);
        i += n;
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

#[cfg(test)]
pub fn zlib_wrap(deflate: &[u8], plain: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78u8, 0x01];
    out.extend_from_slice(deflate);
    out.extend_from_slice(&adler32(plain).to_be_bytes());
    out
}

#[cfg(test)]
pub fn make_png(width: u32, height: u32, channels: usize, pixels: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&PNG_SIG);
    let color: u8 = match channels {
        1 => 0,
        3 => 2,
        4 => 6,
        _ => 0,
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, color, 0, 0, 0]);
    png_chunk(&mut out, b"IHDR", &ihdr);
    let stride = width as usize * channels;
    let mut raw = Vec::new();
    for row in 0..height as usize {
        raw.push(0u8);
        let start = row * stride;
        if start < pixels.len() {
            let end = (start + stride).min(pixels.len());
            raw.extend_from_slice(&pixels[start..end]);
            raw.resize(raw.len() + (stride - (end - start)), 0);
        } else {
            raw.resize(raw.len() + stride, 0);
        }
    }
    png_chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    png_chunk(&mut out, b"IEND", &[]);
    out
}

#[cfg(test)]
fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(kind, data).to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    struct BitWriter {
        buf: Vec<u8>,
        acc: u32,
        cnt: u32,
    }

    impl BitWriter {
        fn new() -> BitWriter {
            BitWriter { buf: Vec::new(), acc: 0, cnt: 0 }
        }

        fn bits(&mut self, v: u32, n: u32) {
            for k in 0..n {
                let b = (v >> k) & 1;
                self.acc |= b << self.cnt;
                self.cnt += 1;
                if self.cnt == 8 {
                    self.buf.push(self.acc as u8);
                    self.acc = 0;
                    self.cnt = 0;
                }
            }
        }

        fn code(&mut self, v: u32, n: u32) {
            for k in (0..n).rev() {
                self.bits((v >> k) & 1, 1);
            }
        }

        fn finish(mut self) -> Vec<u8> {
            if self.cnt > 0 {
                self.buf.push(self.acc as u8);
            }
            self.buf
        }
    }

    fn canonical(lengths: &[u8]) -> Vec<(u32, u32)> {
        let mut counts = [0u32; 16];
        for l in lengths {
            counts[*l as usize] += 1;
        }
        counts[0] = 0;
        let mut next = [0u32; 16];
        let mut code = 0u32;
        for len in 1..16 {
            code = (code + counts[len - 1]) << 1;
            next[len] = code;
        }
        let mut out = vec![(0u32, 0u32); lengths.len()];
        for (sym, l) in lengths.iter().enumerate() {
            if *l > 0 {
                out[sym] = (next[*l as usize], *l as u32);
                next[*l as usize] += 1;
            }
        }
        out
    }

    #[test]
    fn checksums_match_known_vectors() {
        assert_eq!(adler32(b"abc"), 0x024d_0127);
        assert_eq!(adler32(b""), 1);
        assert_eq!(crc32(b"", b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b"IEND", b""), 0xAE42_6082);
    }

    #[test]
    fn inflate_stored_block() {
        let plain = b"the quick brown fox jumps over the lazy dog".to_vec();
        let mut deflate = vec![0x01u8];
        deflate.extend_from_slice(&(plain.len() as u16).to_le_bytes());
        deflate.extend_from_slice(&(!(plain.len() as u16)).to_le_bytes());
        deflate.extend_from_slice(&plain);
        let z = zlib_wrap(&deflate, &plain);
        assert_eq!(inflate(&z).unwrap(), plain);
    }

    #[test]
    fn inflate_stored_writer_round_trip() {
        for len in [0usize, 1, 7, 256, 5000] {
            let plain: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
            assert_eq!(inflate(&zlib_stored(&plain)).unwrap(), plain, "len {}", len);
        }
    }

    #[test]
    fn inflate_stored_rejects_bad_length_pair() {
        let plain = b"hello world".to_vec();
        let mut deflate = vec![0x01u8];
        deflate.extend_from_slice(&11u16.to_le_bytes());
        deflate.extend_from_slice(&11u16.to_le_bytes());
        deflate.extend_from_slice(&plain);
        let err = inflate(&zlib_wrap(&deflate, &plain)).unwrap_err();
        assert!(err.contains("length pair"), "{}", err);
    }

    #[test]
    fn inflate_fixed_huffman_block() {
        let plain = b"hello".to_vec();
        let mut w = BitWriter::new();
        w.bits(1, 1);
        w.bits(1, 2);
        for c in plain.iter() {
            let sym = *c as u32;
            if sym < 144 {
                w.code(0x30 + sym, 8);
            } else {
                w.code(0x190 + (sym - 144), 9);
            }
        }
        w.code(0, 7);
        assert_eq!(inflate(&zlib_wrap(&w.finish(), &plain)).unwrap(), plain);
    }

    #[test]
    fn inflate_fixed_huffman_with_back_reference() {
        let plain = b"ababababab".to_vec();
        let mut w = BitWriter::new();
        w.bits(1, 1);
        w.bits(1, 2);
        for c in b"ab".iter() {
            w.code(0x30 + *c as u32, 8);
        }
        w.code(6, 7);
        w.code(1, 5);
        w.code(0, 7);
        assert_eq!(inflate(&zlib_wrap(&w.finish(), &plain)).unwrap(), plain);
    }

    #[test]
    fn inflate_dynamic_huffman_block() {
        let plain = b"hello".to_vec();
        let mut lit_lengths = vec![8u8; 257];
        lit_lengths[255] = 9;
        lit_lengths[256] = 9;
        let lit_codes = canonical(&lit_lengths);
        let mut cl_lengths = [0u8; 19];
        cl_lengths[0] = 1;
        cl_lengths[8] = 2;
        cl_lengths[9] = 2;
        let cl_codes = canonical(&cl_lengths);
        let mut w = BitWriter::new();
        w.bits(1, 1);
        w.bits(2, 2);
        w.bits(0, 5);
        w.bits(0, 5);
        w.bits(5, 4);
        for l in [0u8, 0, 0, 1, 2, 0, 2, 0, 0] {
            w.bits(l as u32, 3);
        }
        for l in lit_lengths.iter().chain(std::iter::once(&0u8)) {
            let (c, n) = cl_codes[*l as usize];
            w.code(c, n);
        }
        for c in plain.iter() {
            let (c, l) = lit_codes[*c as usize];
            w.code(c, l);
        }
        let (c, l) = lit_codes[256];
        w.code(c, l);
        assert_eq!(inflate(&zlib_wrap(&w.finish(), &plain)).unwrap(), plain);
    }

    #[test]
    fn inflate_rejects_corrupt_adler() {
        let plain = b"payload".to_vec();
        let mut z = zlib_stored(&plain);
        let last = z.len() - 1;
        z[last] = z[last].wrapping_add(1);
        let err = inflate(&z).unwrap_err();
        assert!(err.contains("adler32"), "{}", err);
    }

    #[test]
    fn inflate_rejects_bad_header() {
        assert!(inflate(&[0x00, 0x00, 0x00]).is_err());
        assert!(inflate(&[0x78]).is_err());
    }

    #[test]
    fn decode_rgb_png() {
        let pixels = vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0];
        let png = make_png(2, 2, 3, &pixels);
        assert!(is_png(&png));
        assert!(!is_jpeg(&png));
        let r = decode(&png).unwrap();
        assert_eq!((r.width, r.height, r.channels), (2, 2, 3));
        assert_eq!(r.data, pixels);
    }

    #[test]
    fn decode_grayscale_png() {
        let png = make_png(2, 1, 1, &[10, 200]);
        let r = decode(&png).unwrap();
        assert_eq!(r.channels, 1);
        assert_eq!(r.data, vec![10, 200]);
    }

    #[test]
    fn decode_png_applies_scanline_filters() {
        let mut raw = Vec::new();
        raw.push(1);
        raw.extend_from_slice(&[1, 2, 3, 1, 1, 1]);
        raw.push(2);
        raw.extend_from_slice(&[1, 1, 1, 1, 1, 1]);
        raw.push(3);
        raw.extend_from_slice(&[1, 1, 1, 1, 1, 1]);
        raw.push(4);
        raw.extend_from_slice(&[1, 1, 1, 1, 1, 1]);
        let mut out = Vec::new();
        out.extend_from_slice(&PNG_SIG);
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&2u32.to_be_bytes());
        ihdr.extend_from_slice(&4u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        png_chunk(&mut out, b"IHDR", &ihdr);
        png_chunk(&mut out, b"IDAT", &zlib_stored(&raw));
        png_chunk(&mut out, b"IEND", &[]);
        let r = decode(&out).unwrap();
        assert_eq!((r.width, r.height, r.channels), (2, 4, 3));
        let want: Vec<u8> = vec![
            1, 2, 3, 2, 3, 4, 2, 3, 4, 3, 4, 5, 2, 2, 3, 3, 4, 5, 3, 3, 4, 4, 5, 6,
        ];
        assert_eq!(r.data, want);
    }

    #[test]
    fn decode_rgba_png_keeps_alpha() {
        let pixels = vec![10, 20, 30, 255, 40, 50, 60, 0];
        let png = make_png(2, 1, 4, &pixels);
        let r = decode(&png).unwrap();
        assert_eq!(r.channels, 4);
        assert_eq!(r.data, pixels);
    }

    #[test]
    fn decode_png_16bit_samples_take_high_byte() {
        let mut raw = Vec::new();
        raw.push(0);
        raw.extend_from_slice(&[0x12, 0x34, 0xAB, 0xCD]);
        let mut out = Vec::new();
        out.extend_from_slice(&PNG_SIG);
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&2u32.to_be_bytes());
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&[16, 0, 0, 0, 0]);
        png_chunk(&mut out, b"IHDR", &ihdr);
        png_chunk(&mut out, b"IDAT", &zlib_stored(&raw));
        png_chunk(&mut out, b"IEND", &[]);
        let r = decode(&out).unwrap();
        assert_eq!(r.data, vec![0x12, 0xAB]);
    }

    #[test]
    fn decode_png_palette_with_trns() {
        let mut out = Vec::new();
        out.extend_from_slice(&PNG_SIG);
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&2u32.to_be_bytes());
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 3, 0, 0, 0]);
        png_chunk(&mut out, b"IHDR", &ihdr);
        png_chunk(&mut out, b"PLTE", &[255, 0, 0, 0, 0, 255]);
        png_chunk(&mut out, b"tRNS", &[0, 255]);
        let raw = vec![0u8, 0, 1];
        png_chunk(&mut out, b"IDAT", &zlib_stored(&raw));
        png_chunk(&mut out, b"IEND", &[]);
        let r = decode(&out).unwrap();
        assert_eq!(r.channels, 4);
        assert_eq!(r.data, vec![255, 0, 0, 0, 0, 0, 255, 255]);
    }

    #[test]
    fn decode_rejects_interlaced_and_corrupt() {
        let mut out = Vec::new();
        out.extend_from_slice(&PNG_SIG);
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 1]);
        png_chunk(&mut out, b"IHDR", &ihdr);
        png_chunk(&mut out, b"IDAT", &zlib_stored(&[0, 0, 0, 0]));
        png_chunk(&mut out, b"IEND", &[]);
        let err = decode(&out).unwrap_err();
        assert!(err.contains("Adam7"), "{}", err);

        let mut broken = make_png(1, 1, 3, &[1, 2, 3]);
        let n = broken.len();
        broken[n - 1] ^= 0xFF;
        assert!(decode(&broken).is_err());
    }

    #[test]
    fn decode_rejects_jpeg_with_a_message() {
        let jpeg = [0xFFu8, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0];
        assert!(is_jpeg(&jpeg));
        let err = decode(&jpeg).unwrap_err();
        assert!(err.contains("DCTDecode"), "{}", err);
    }

    #[test]
    fn jpeg_size_reads_sof_markers() {
        let mut jpeg = vec![0xFF, 0xD8];
        jpeg.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
        jpeg.extend_from_slice(b"JFIF\0");
        jpeg.extend_from_slice(&[0x01, 0x02, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);
        jpeg.extend_from_slice(&[0xFF, 0xC2, 0x00, 0x0B, 0x08, 0x01, 0x2C, 0x01, 0x90, 0x03]);
        jpeg.extend_from_slice(&[0xFF, 0xD9]);
        let info = jpeg_info(&jpeg).unwrap();
        assert_eq!((info.width, info.height), (400, 300));
        assert_eq!(info.components, 3);
        assert!(info.progressive);
        assert_eq!(jpeg_size(&jpeg), Some((400, 300)));
        assert_eq!(jpeg_size(b"not a jpeg"), None);
    }
}


