use base64::Engine;

/// Bộ chuyển đổi WMF (Windows Metafile) sang SVG vector nội tuyến cho công thức MathType / Office,
/// đồng thời cung cấp bộ giải mã ký hiệu dùng chung cho EMF và trích xuất đáp án dạng ảnh.

#[derive(Debug, Clone)]
pub(crate) struct GdiPen {
    pub style: u32,
    pub width: f32,
    pub color: (u8, u8, u8),
}

impl Default for GdiPen {
    fn default() -> Self {
        Self {
            style: 0, // PS_SOLID
            width: 1.0,
            color: (0, 0, 0),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GdiBrush {
    pub style: u32,
    pub color: (u8, u8, u8),
}

impl Default for GdiBrush {
    fn default() -> Self {
        Self {
            style: 0, // BS_SOLID
            color: (0, 0, 0),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GdiFont {
    pub height: i32,
    pub weight: i32,
    pub italic: bool,
    #[allow(dead_code)]
    pub underline: bool,
    pub charset: u8,
    pub face: String,
}

impl Default for GdiFont {
    fn default() -> Self {
        Self {
            height: 240,
            weight: 400,
            italic: false,
            underline: false,
            charset: 0,
            face: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
enum WmfGdiObject {
    Font(GdiFont),
    Pen(GdiPen),
    Brush(GdiBrush),
    Other,
}

fn add_gdi_object(table: &mut Vec<Option<WmfGdiObject>>, obj: WmfGdiObject) {
    if let Some(slot) = table.iter_mut().find(|s| s.is_none()) {
        *slot = Some(obj);
    } else {
        table.push(Some(obj));
    }
}

pub(crate) fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

pub(crate) fn color_to_css(rgb: (u8, u8, u8)) -> String {
    if rgb == (0, 0, 0) {
        "currentColor".to_string()
    } else {
        format!("#{:02x}{:02x}{:02x}", rgb.0, rgb.1, rgb.2)
    }
}

pub(crate) fn u32_colorref_to_rgb(c: u32) -> (u8, u8, u8) {
    (
        (c & 0xFF) as u8,
        ((c >> 8) & 0xFF) as u8,
        ((c >> 16) & 0xFF) as u8,
    )
}

/// Giải mã 1 byte trong font Adobe Symbol / Euclid Symbol / Symbol Tiger sang ký tự Unicode.
fn decode_symbol_byte(b: u8) -> Option<char> {
    match b {
        0x20 => Some(' '),
        0x21 => Some('!'),
        0x22 => Some('∀'),
        0x23 => Some('#'),
        0x24 => Some('∃'),
        0x25 => Some('%'),
        0x26 => Some('&'),
        0x27 => Some('∋'),
        0x28 => Some('('),
        0x29 => Some(')'),
        0x2A => Some('∗'),
        0x2B => Some('+'),
        0x2C => Some(','),
        0x2D => Some('−'),
        0x2E => Some('.'),
        0x2F => Some('/'),
        0x30..=0x39 => Some(b as char),
        0x3A => Some(':'),
        0x3B => Some(';'),
        0x3C => Some('<'),
        0x3D => Some('='),
        0x3E => Some('>'),
        0x3F => Some('?'),
        0x40 => Some('≅'),
        0x41 => Some('Α'),
        0x42 => Some('Β'),
        0x43 => Some('Χ'),
        0x44 => Some('Δ'),
        0x45 => Some('Ε'),
        0x46 => Some('Φ'),
        0x47 => Some('Γ'),
        0x48 => Some('Η'),
        0x49 => Some('Ι'),
        0x4A => Some('ϑ'),
        0x4B => Some('Κ'),
        0x4C => Some('Λ'),
        0x4D => Some('Μ'),
        0x4E => Some('Ν'),
        0x4F => Some('Ο'),
        0x50 => Some('Π'),
        0x51 => Some('Θ'),
        0x52 => Some('Ρ'),
        0x53 => Some('Σ'),
        0x54 => Some('Τ'),
        0x55 => Some('Υ'),
        0x56 => Some('ς'),
        0x57 => Some('Ω'),
        0x58 => Some('Ξ'),
        0x59 => Some('Ψ'),
        0x5A => Some('Ζ'),
        0x5B => Some('['),
        0x5C => Some('∴'),
        0x5D => Some(']'),
        0x5E => Some('⊥'),
        0x5F => Some('_'),
        0x61 => Some('α'),
        0x62 => Some('β'),
        0x63 => Some('χ'),
        0x64 => Some('δ'),
        0x65 => Some('ε'),
        0x66 => Some('φ'),
        0x67 => Some('γ'),
        0x68 => Some('η'),
        0x69 => Some('ι'),
        0x6A => Some('ϕ'),
        0x6B => Some('κ'),
        0x6C => Some('λ'),
        0x6D => Some('μ'),
        0x6E => Some('ν'),
        0x6F => Some('ο'),
        0x70 => Some('π'),
        0x71 => Some('θ'),
        0x72 => Some('ρ'),
        0x73 => Some('σ'),
        0x74 => Some('τ'),
        0x75 => Some('υ'),
        0x76 => Some('ϖ'),
        0x77 => Some('ω'),
        0x78 => Some('ξ'),
        0x79 => Some('ψ'),
        0x7A => Some('ζ'),
        0x7B => Some('{'),
        0x7C => Some('|'),
        0x7D => Some('}'),
        0x7E => Some('∼'),
        0xA0 => Some('\u{00A0}'),
        0xA1 => Some('ϒ'),
        0xA2 => Some('′'),
        0xA3 => Some('≤'),
        0xA4 => Some('⁄'),
        0xA5 => Some('∞'),
        0xA6 => Some('ƒ'),
        0xA7 => Some('♣'),
        0xA8 => Some('♦'),
        0xA9 => Some('♥'),
        0xAA => Some('♠'),
        0xAB => Some('↔'),
        0xAC => Some('←'),
        0xAD => Some('↑'),
        0xAE => Some('→'),
        0xAF => Some('↓'),
        0xB0 => Some('°'),
        0xB1 => Some('±'),
        0xB2 => Some('″'),
        0xB3 => Some('≥'),
        0xB4 => Some('×'),
        0xB5 => Some('∝'),
        0xB6 => Some('∂'),
        0xB7 => Some('•'),
        0xB8 => Some('÷'),
        0xB9 => Some('≠'),
        0xBA => Some('≡'),
        0xBB => Some('≈'),
        0xBC => Some('…'),
        0xBD => Some('∣'),
        0xBE => Some('⎯'),
        0xBF => Some('↵'),
        0xC0 => Some('ℵ'),
        0xC1 => Some('ℑ'),
        0xC2 => Some('ℜ'),
        0xC3 => Some('℘'),
        0xC4 => Some('⊗'),
        0xC5 => Some('⊕'),
        0xC6 => Some('∅'),
        0xC7 => Some('∩'),
        0xC8 => Some('∪'),
        0xC9 => Some('⊃'),
        0xCA => Some('⊇'),
        0xCB => Some('⊄'),
        0xCC => Some('⊂'),
        0xCD => Some('⊆'),
        0xCE => Some('∈'),
        0xCF => Some('∉'),
        0xD0 => Some('∠'),
        0xD1 => Some('∇'),
        0xD2 => Some('®'),
        0xD3 => Some('©'),
        0xD4 => Some('™'),
        0xD5 => Some('∏'),
        0xD6 => Some('√'),
        0xD7 => Some('·'),
        0xD8 => Some('¬'),
        0xD9 => Some('∧'),
        0xDA => Some('∨'),
        0xDB => Some('⇔'),
        0xDC => Some('⇐'),
        0xDD => Some('⇑'),
        0xDE => Some('⇒'),
        0xDF => Some('⇓'),
        0xE0 => Some('◊'),
        0xE1 => Some('⟨'),
        0xE5 => Some('∑'),
        0xE6 => Some('⎛'),
        0xE7 => Some('⎜'),
        0xE8 => Some('⎝'),
        0xE9 => Some('⎡'),
        0xEA => Some('⎢'),
        0xEB => Some('⎣'),
        0xEC => Some('⎧'),
        0xED => Some('⎨'),
        0xEE => Some('⎩'),
        0xEF => Some('⎪'),
        0xF1 => Some('⟩'),
        0xF2 => Some('∫'),
        0xF3 => Some('⌠'),
        0xF4 => Some('⎮'),
        0xF5 => Some('⌡'),
        0xF6 => Some('⎞'),
        0xF7 => Some('⎟'),
        0xF8 => Some('⎠'),
        0xF9 => Some('⎤'),
        0xFA => Some('⎥'),
        0xFB => Some('⎦'),
        0xFC => Some('⎫'),
        0xFD => Some('⎬'),
        0xFE => Some('⎭'),
        _ => None,
    }
}

/// Giải mã 1 byte trong font MT Extra / MT Extra Tiger / Euclid Extra.
fn decode_mt_extra_byte(b: u8) -> Option<char> {
    match b {
        0x20 => Some(' '),
        0x22 => Some('″'),
        0x23 => Some('‴'),
        0x24 => Some('⁗'),
        0x26 => Some('′'),
        0x3C => Some('⟨'),
        0x3E => Some('⟩'),
        0x40 => Some('∇'),
        0x44 => Some('Δ'),
        0x49 => Some('∩'),
        0x4B => Some('⋯'),
        0x4C => Some('⋮'),
        0x4D => Some('⋱'),
        0x4E => Some('⋰'),
        0x50 => Some('∥'),
        0x55 => Some('∪'),
        0x56 => Some('△'),
        0x57 => Some('□'),
        0x66 => Some('⌢'),
        0x67 => Some('ℏ'),
        0x68 => Some('ℏ'),
        0x6B => Some('ƛ'),
        0x6C => Some('ℓ'),
        0x6F => Some('°'),
        0x72 => Some('→'),
        0x75 => Some('⎯'),
        0x81 => Some('←'),
        0x82 => Some('↔'),
        0x83 => Some('⇌'),
        0x84 => Some('⇄'),
        0xA1 => Some('ℝ'),
        0xA2 => Some('ℤ'),
        0xA3 => Some('ℚ'),
        0xA4 => Some('ℂ'),
        0xA5 => Some('ℕ'),
        0xB0 => Some('°'),
        0xB7 => Some('∠'),
        _ if (0x21..=0x7E).contains(&b) => Some(b as char),
        _ => decode_mathtype_fallback_byte(b),
    }
}

fn decode_euclid_math_two_byte(b: u8) -> Option<char> {
    match b {
        0x43 => Some('ℂ'),
        0x4E => Some('ℕ'),
        0x51 => Some('ℚ'),
        0x52 => Some('ℝ'),
        0x5A => Some('ℤ'),
        _ if (0x20..=0x7E).contains(&b) => Some(b as char),
        _ => decode_cp1252_byte(b),
    }
}

fn decode_cp1252_byte(b: u8) -> Option<char> {
    match b {
        0x20..=0x7E => Some(b as char),
        0x80 => Some('€'),
        0x82 => Some('‚'),
        0x83 => Some('ƒ'),
        0x84 => Some('„'),
        0x85 => Some('…'),
        0x86 => Some('†'),
        0x87 => Some('‡'),
        0x88 => Some('ˆ'),
        0x89 => Some('‰'),
        0x8A => Some('Š'),
        0x8B => Some('‹'),
        0x8C => Some('Œ'),
        0x8E => Some('Ž'),
        0x91 => Some('‘'),
        0x92 => Some('’'),
        0x93 => Some('“'),
        0x94 => Some('”'),
        0x95 => Some('•'),
        0x96 => Some('–'),
        0x97 => Some('—'),
        0x98 => Some('˜'),
        0x99 => Some('™'),
        0x9A => Some('š'),
        0x9B => Some('›'),
        0x9C => Some('œ'),
        0x9E => Some('ž'),
        0x9F => Some('Ÿ'),
        0xA0..=0xFF => char::from_u32(b as u32),
        _ => None,
    }
}

fn decode_mathtype_fallback_byte(b: u8) -> Option<char> {
    match b {
        0x20..=0x7E => Some(b as char),
        0xA0 => Some('\u{00A0}'),
        0xA1 => Some('ℝ'),
        0xA2 => Some('′'),
        0xA3 => Some('″'),
        0xA4 => Some('‴'),
        0xA5 => Some('∞'),
        0xAE => Some('→'),
        0xB0 => Some('°'),
        0xB1 => Some('±'),
        0xB2 => Some('²'),
        0xB3 => Some('³'),
        0xB7 => Some('·'),
        0xBE => Some('⎯'),
        _ => decode_cp1252_byte(b),
    }
}

/// Giải mã mảng byte WMF theo font hiện hành (`face`, `charset`), trả về danh sách ký tự Unicode.
pub(crate) fn decode_gdi_bytes_with_font(raw: &[u8], face: &str, charset: u8) -> Vec<char> {
    let mut chars = Vec::with_capacity(raw.len());
    // GB2312_CHARSET (134) / GBK: xử lý cặp 2 byte MBCS (vd Malgun Gothic cs=134 0xA8 0x4F -> '∣')
    if charset == 134 {
        let mut i = 0;
        while i < raw.len() {
            let b0 = raw[i];
            if b0 >= 0x81 && i + 1 < raw.len() {
                let b1 = raw[i + 1];
                let mapped = match (b0, b1) {
                    (0xA8, 0x4F) => Some('∣'),
                    (0xA1, 0xE6) => Some('℃'),
                    (0xA1, 0xE3) => Some('°'),
                    (0xA3, 0xAC) => Some(','),
                    (0xA3, 0xBA) => Some(':'),
                    (0xA3, 0xBB) => Some(';'),
                    _ => None,
                };
                if let Some(ch) = mapped {
                    chars.push(ch);
                    i += 2;
                    continue;
                }
            }
            if let Some(ch) = decode_cp1252_byte(b0) {
                chars.push(ch);
            }
            i += 1;
        }
        return chars;
    }

    let fl = face.trim().to_ascii_lowercase();
    enum FontEncoding {
        Symbol,
        MtExtra,
        EuclidMathTwo,
        StandardText,
        UnknownSymbol,
    }
    let enc = if fl.contains("mt extra") || fl.contains("euclid extra") {
        FontEncoding::MtExtra
    } else if fl.contains("euclid math two") {
        FontEncoding::EuclidMathTwo
    } else if fl.contains("symbol") || fl == "euclid symbol" {
        FontEncoding::Symbol
    } else if fl.is_empty() {
        if charset == 2 {
            FontEncoding::Symbol
        } else {
            FontEncoding::UnknownSymbol
        }
    } else if charset == 2 && !fl.contains("times") && !fl.contains("arial") && !fl.contains("vni")
    {
        FontEncoding::Symbol
    } else {
        FontEncoding::StandardText
    };

    for &b in raw {
        let ch = match enc {
            FontEncoding::Symbol => decode_symbol_byte(b),
            FontEncoding::MtExtra => decode_mt_extra_byte(b),
            FontEncoding::EuclidMathTwo => decode_euclid_math_two_byte(b),
            FontEncoding::StandardText => decode_cp1252_byte(b),
            FontEncoding::UnknownSymbol => decode_mathtype_fallback_byte(b),
        };
        if let Some(c) = ch {
            chars.push(c);
        }
    }
    chars
}

/// Format một ký tự sang SVG text entity: ưu tiên &#xXXXX; cho ký hiệu toán phi-ASCII để kiểm thử & hiển thị chuẩn.
pub(crate) fn format_svg_char(ch: char) -> String {
    match ch {
        '&' => "&amp;".to_string(),
        '<' => "&lt;".to_string(),
        '>' => "&gt;".to_string(),
        '"' => "&quot;".to_string(),
        '\'' => "&#39;".to_string(),
        c if (c as u32) >= 0x20 && (c as u32) <= 0x7E => c.to_string(),
        c if (c as u32) == 0xA0 => "&#160;".to_string(),
        c if (c as u32) <= 0xFF => c.to_string(), // giữ nguyên chữ Việt á, í, ², ³ ở CP1252
        c => format!("&#x{:04X};", c as u32),
    }
}

/// Chuyển đổi WMF byte buffer thành chuỗi URL SVG Base64 (`data:image/svg+xml;base64,...`).
pub fn wmf_to_svg_data_url(bytes: &[u8]) -> Option<String> {
    let svg = wmf_to_svg(bytes)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(svg.as_bytes());
    Some(format!("data:image/svg+xml;base64,{}", b64))
}

#[derive(Debug, Clone)]
struct WmfParseOutput {
    svg: String,
    plain_text: String,
}

/// Trích xuất văn bản thuần túy từ WMF (ví dụ cell đáp án ngắn chỉ chứa công thức MathType WMF).
pub fn wmf_to_plain_text(bytes: &[u8]) -> Option<String> {
    let parsed = parse_wmf_internal(bytes)?;
    let trimmed = parsed.plain_text.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Phân tích WMF nhị phân thành SVG vector.
pub fn wmf_to_svg(bytes: &[u8]) -> Option<String> {
    Some(parse_wmf_internal(bytes)?.svg)
}

fn parse_wmf_internal(bytes: &[u8]) -> Option<WmfParseOutput> {
    if bytes.len() < 18 {
        return None;
    }

    let mut offset = 0usize;
    let mut min_x: i32 = 0;
    let mut min_y: i32 = 0;
    let mut width: i32 = 800;
    let mut height: i32 = 300;
    let mut inch: u16 = 1440;
    let mut has_apm = false;

    let key = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
    if key == 0x9AC6CDD7 && bytes.len() >= 40 {
        has_apm = true;
        let left = i16::from_le_bytes(bytes[6..8].try_into().ok()?) as i32;
        let top = i16::from_le_bytes(bytes[8..10].try_into().ok()?) as i32;
        let right = i16::from_le_bytes(bytes[10..12].try_into().ok()?) as i32;
        let bottom = i16::from_le_bytes(bytes[12..14].try_into().ok()?) as i32;
        inch = u16::from_le_bytes(bytes[14..16].try_into().ok()?);
        if right > left && bottom > top {
            min_x = left;
            min_y = top;
            width = right - left;
            height = bottom - top;
        }
        offset = 22;
    }

    if bytes.len() < offset + 18 {
        return None;
    }
    let header_size_words = u16::from_le_bytes(bytes[offset + 2..offset + 4].try_into().ok()?) as usize;
    offset += (header_size_words * 2).max(18);

    let mut has_win_ext = false;
    let mut svg_elements: Vec<String> = Vec::new();
    let mut text_runs: Vec<(i32, i32, String)> = Vec::new();

    let mut cur_x: i32 = 0;
    let mut cur_y: i32 = 0;
    let mut cur_font = GdiFont::default();
    let mut cur_pen = GdiPen::default();
    let mut cur_brush = GdiBrush::default();
    let mut cur_text_color: (u8, u8, u8) = (0, 0, 0);
    let mut cur_text_align: u16 = 0;
    let mut gdi_table: Vec<Option<WmfGdiObject>> = Vec::new();

    while offset + 6 <= bytes.len() {
        let record_words = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let func = u16::from_le_bytes(bytes[offset + 4..offset + 6].try_into().unwrap());

        if func == 0 || record_words < 3 {
            break;
        }
        let record_bytes = record_words.saturating_mul(2);
        if offset + record_bytes > bytes.len() {
            break;
        }
        let data = &bytes[offset + 6..offset + record_bytes];

        match func {
            // SetWindowOrg (0x020B)
            0x020B if data.len() >= 4 => {
                let win_org_y = i16::from_le_bytes(data[0..2].try_into().unwrap()) as i32;
                let win_org_x = i16::from_le_bytes(data[2..4].try_into().unwrap()) as i32;
                if !has_apm {
                    min_x = win_org_x;
                    min_y = win_org_y;
                }
            }
            // SetWindowExt (0x020C)
            0x020C if data.len() >= 4 => {
                let ey = i16::from_le_bytes(data[0..2].try_into().unwrap()) as i32;
                let ex = i16::from_le_bytes(data[2..4].try_into().unwrap()) as i32;
                if !has_apm && ex > 0 && ey > 0 {
                    width = ex;
                    height = ey;
                    has_win_ext = true;
                }
            }
            // SetTextAlign (0x012E)
            0x012E if data.len() >= 2 => {
                cur_text_align = u16::from_le_bytes(data[0..2].try_into().unwrap());
            }
            // SetTextColor (0x0209)
            0x0209 if data.len() >= 4 => {
                cur_text_color = (data[0], data[1], data[2]);
            }
            // MoveTo (0x0214)
            0x0214 if data.len() >= 4 => {
                cur_y = i16::from_le_bytes(data[0..2].try_into().unwrap()) as i32;
                cur_x = i16::from_le_bytes(data[2..4].try_into().unwrap()) as i32;
            }
            // LineTo (0x0213)
            0x0213 if data.len() >= 4 => {
                let ny = i16::from_le_bytes(data[0..2].try_into().unwrap()) as i32;
                let nx = i16::from_le_bytes(data[2..4].try_into().unwrap()) as i32;
                if cur_pen.style != 5 {
                    let sw = (cur_pen.width.max(1.0) * 1.5).max(1.5);
                    svg_elements.push(format!(
                        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{:.1}" stroke-linecap="round"/>"#,
                        cur_x,
                        cur_y,
                        nx,
                        ny,
                        color_to_css(cur_pen.color),
                        sw
                    ));
                }
                cur_x = nx;
                cur_y = ny;
            }
            // Polyline (0x0325)
            0x0325 if data.len() >= 2 => {
                let count = i16::from_le_bytes(data[0..2].try_into().unwrap()) as usize;
                if count >= 2 && data.len() >= 2 + count * 4 && cur_pen.style != 5 {
                    let mut pts = Vec::with_capacity(count);
                    for i in 0..count {
                        let x = i16::from_le_bytes(data[2 + i * 4..4 + i * 4].try_into().unwrap()) as i32;
                        let y = i16::from_le_bytes(data[4 + i * 4..6 + i * 4].try_into().unwrap()) as i32;
                        pts.push(format!("{},{}", x, y));
                    }
                    let sw = (cur_pen.width.max(1.0) * 1.5).max(1.5);
                    let dash = if cur_pen.style == 1 || cur_pen.style == 2 {
                        r#" stroke-dasharray="16,12""#
                    } else {
                        ""
                    };
                    svg_elements.push(format!(
                        r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="{:.1}" stroke-linecap="round"{}/>"#,
                        pts.join(" "),
                        color_to_css(cur_pen.color),
                        sw,
                        dash
                    ));
                }
            }
            // Polygon (0x0324)
            0x0324 if data.len() >= 2 => {
                let count = i16::from_le_bytes(data[0..2].try_into().unwrap()) as usize;
                if count >= 3 && data.len() >= 2 + count * 4 {
                    let mut pts = Vec::with_capacity(count);
                    for i in 0..count {
                        let x = i16::from_le_bytes(data[2 + i * 4..4 + i * 4].try_into().unwrap()) as i32;
                        let y = i16::from_le_bytes(data[4 + i * 4..6 + i * 4].try_into().unwrap()) as i32;
                        pts.push(format!("{},{}", x, y));
                    }
                    let fill = if cur_brush.style == 1 {
                        "none".to_string()
                    } else {
                        color_to_css(cur_brush.color)
                    };
                    let stroke = if cur_pen.style == 5 {
                        "none".to_string()
                    } else {
                        color_to_css(cur_pen.color)
                    };
                    svg_elements.push(format!(
                        r#"<polygon points="{}" fill="{}" stroke="{}" stroke-width="{:.1}"/>"#,
                        pts.join(" "),
                        fill,
                        stroke,
                        cur_pen.width.max(1.0)
                    ));
                }
            }
            // PolyPolygon (0x0538)
            0x0538 if data.len() >= 2 => {
                let n_polys = u16::from_le_bytes(data[0..2].try_into().unwrap()) as usize;
                if n_polys > 0 && data.len() >= 2 + n_polys * 2 {
                    let mut counts = Vec::with_capacity(n_polys);
                    let mut total_pts = 0usize;
                    for p_i in 0..n_polys {
                        let c = u16::from_le_bytes(data[2 + p_i * 2..4 + p_i * 2].try_into().unwrap()) as usize;
                        counts.push(c);
                        total_pts += c;
                    }
                    let mut pt_off = 2 + n_polys * 2;
                    if data.len() >= pt_off + total_pts * 4 {
                        let mut d_attr = String::new();
                        for c in counts {
                            for pi in 0..c {
                                let x = i16::from_le_bytes(data[pt_off..pt_off + 2].try_into().unwrap()) as i32;
                                let y = i16::from_le_bytes(data[pt_off + 2..pt_off + 4].try_into().unwrap()) as i32;
                                pt_off += 4;
                                if pi == 0 {
                                    d_attr.push_str(&format!("M{} {} ", x, y));
                                } else {
                                    d_attr.push_str(&format!("L{} {} ", x, y));
                                }
                            }
                            d_attr.push_str("Z ");
                        }
                        let fill = if cur_brush.style == 1 {
                            "none".to_string()
                        } else {
                            color_to_css(cur_brush.color)
                        };
                        svg_elements.push(format!(
                            r#"<path d="{}" fill="{}" fill-rule="evenodd"/>"#,
                            d_attr.trim(),
                            fill
                        ));
                    }
                }
            }
            // Rectangle (0x041B)
            0x041B if data.len() >= 8 => {
                let bottom = i16::from_le_bytes(data[0..2].try_into().unwrap()) as i32;
                let right = i16::from_le_bytes(data[2..4].try_into().unwrap()) as i32;
                let top = i16::from_le_bytes(data[4..6].try_into().unwrap()) as i32;
                let left = i16::from_le_bytes(data[6..8].try_into().unwrap()) as i32;
                let fill = if cur_brush.style == 1 {
                    "none".to_string()
                } else {
                    color_to_css(cur_brush.color)
                };
                let stroke = if cur_pen.style == 5 {
                    "none".to_string()
                } else {
                    color_to_css(cur_pen.color)
                };
                svg_elements.push(format!(
                    r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="{:.1}"/>"#,
                    left.min(right),
                    top.min(bottom),
                    (right - left).abs().max(1),
                    (bottom - top).abs().max(1),
                    fill,
                    stroke,
                    cur_pen.width.max(1.0)
                ));
            }
            // CreateFontIndirect (0x02FB): LOGFONT16 (face starts at byte offset 18!)
            0x02FB if data.len() >= 2 => {
                let fh = i16::from_le_bytes(data[0..2].try_into().unwrap()) as i32;
                let weight = if data.len() >= 10 {
                    i16::from_le_bytes(data[8..10].try_into().unwrap()) as i32
                } else {
                    400
                };
                let italic = data.get(10).copied().unwrap_or(0) != 0;
                let underline = data.get(11).copied().unwrap_or(0) != 0;
                let charset = data.get(13).copied().unwrap_or(0);
                let face = if data.len() > 18 {
                    let face_bytes = &data[18..];
                    let end = face_bytes.iter().position(|&b| b == 0).unwrap_or(face_bytes.len());
                    String::from_utf8_lossy(&face_bytes[..end]).trim().to_string()
                } else {
                    String::new()
                };
                add_gdi_object(
                    &mut gdi_table,
                    WmfGdiObject::Font(GdiFont {
                        height: fh.abs().max(1),
                        weight,
                        italic,
                        underline,
                        charset,
                        face,
                    }),
                );
            }
            // CreatePenIndirect (0x02FA)
            0x02FA => {
                let pen = if data.len() >= 10 {
                    let style = u16::from_le_bytes(data[0..2].try_into().unwrap()) as u32;
                    let w = i16::from_le_bytes(data[2..4].try_into().unwrap()).unsigned_abs() as f32;
                    GdiPen {
                        style,
                        width: w.max(1.0),
                        color: (data[6], data[7], data[8]),
                    }
                } else {
                    GdiPen::default()
                };
                add_gdi_object(&mut gdi_table, WmfGdiObject::Pen(pen));
            }
            // CreateBrushIndirect (0x02FC)
            0x02FC => {
                let brush = if data.len() >= 6 {
                    let style = u16::from_le_bytes(data[0..2].try_into().unwrap()) as u32;
                    GdiBrush {
                        style,
                        color: (data[2], data[3], data[4]),
                    }
                } else {
                    GdiBrush::default()
                };
                add_gdi_object(&mut gdi_table, WmfGdiObject::Brush(brush));
            }
            // Các opcode tạo GDI object khác (Palette, PatternBrush, Region) chiếm slot handle
            0x00F7 | 0x0142 | 0x01F9 | 0x06FF => {
                add_gdi_object(&mut gdi_table, WmfGdiObject::Other);
            }
            // DeleteObject (0x01F0)
            0x01F0 if data.len() >= 2 => {
                let idx = u16::from_le_bytes(data[0..2].try_into().unwrap()) as usize;
                if idx < gdi_table.len() {
                    gdi_table[idx] = None;
                }
            }
            // SelectObject (0x012D)
            0x012D if data.len() >= 2 => {
                let idx = u16::from_le_bytes(data[0..2].try_into().unwrap()) as usize;
                if let Some(Some(obj)) = gdi_table.get(idx) {
                    match obj {
                        WmfGdiObject::Font(f) => cur_font = f.clone(),
                        WmfGdiObject::Pen(p) => cur_pen = p.clone(),
                        WmfGdiObject::Brush(b) => cur_brush = b.clone(),
                        WmfGdiObject::Other => {}
                    }
                }
            }
            // TextOut (0x0521): len(i16), string(len [+pad]), y(i16), x(i16)
            0x0521 if data.len() >= 2 => {
                let count = i16::from_le_bytes(data[0..2].try_into().unwrap()).max(0) as usize;
                let coord_off = 2 + count + (count % 2);
                if data.len() >= coord_off + 4 {
                    let raw = &data[2..2 + count];
                    let y = i16::from_le_bytes(data[coord_off..coord_off + 2].try_into().unwrap()) as i32;
                    let x = i16::from_le_bytes(data[coord_off + 2..coord_off + 4].try_into().unwrap()) as i32;
                    let chars = decode_gdi_bytes_with_font(raw, &cur_font.face, cur_font.charset);
                    let plain: String = chars.iter().collect();
                    if !plain.is_empty() {
                        text_runs.push((y, x, plain));
                        let esc: String = chars.iter().map(|&c| format_svg_char(c)).collect();
                        svg_elements.push(render_svg_text_elem(
                            x,
                            y,
                            &cur_font,
                            cur_text_color,
                            cur_text_align,
                            &esc,
                        ));
                    }
                }
            }
            // ExtTextOut (0x0A32): Y(i16), X(i16), Count(i16), Opts(u16), [Rect 8B], Text, [DX Array]
            0x0A32 if data.len() >= 8 => {
                let y = i16::from_le_bytes(data[0..2].try_into().unwrap()) as i32;
                let x = i16::from_le_bytes(data[2..4].try_into().unwrap()) as i32;
                let count = i16::from_le_bytes(data[4..6].try_into().unwrap()).max(0) as usize;
                let options = u16::from_le_bytes(data[6..8].try_into().unwrap());

                let mut text_offset = 8;
                if options & 0x0006 != 0 {
                    text_offset += 8;
                }

                if data.len() >= text_offset + count {
                    let raw_chars = &data[text_offset..text_offset + count];
                    let dx_offset = text_offset + count + (count % 2);
                    let mut dxs = Vec::new();
                    if dx_offset < data.len() {
                        let num_dx = (data.len() - dx_offset) / 2;
                        for i in 0..num_dx {
                            let dx_val = i16::from_le_bytes(
                                data[dx_offset + i * 2..dx_offset + (i + 1) * 2]
                                    .try_into()
                                    .unwrap(),
                            ) as i32;
                            dxs.push(dx_val);
                        }
                    }

                    let use_cp = (cur_text_align & 1 != 0) && x == 0 && y == 0;
                    let pos_x = if use_cp { cur_x } else { x };
                    let pos_y = if use_cp { cur_y } else { y };

                    let chars = decode_gdi_bytes_with_font(raw_chars, &cur_font.face, cur_font.charset);
                    let plain: String = chars.iter().collect();
                    if !plain.is_empty() {
                        text_runs.push((pos_y, pos_x, plain));
                    }

                    // Với MBCS (GB2312) số char có thể khác số byte -> xuất nguyên chuỗi nếu lệch độ dài
                    if !dxs.is_empty() && chars.len() == raw_chars.len() {
                        let mut char_x = pos_x;
                        for (i, ch) in chars.iter().enumerate() {
                            let ch_str = format_svg_char(*ch);
                            svg_elements.push(render_svg_text_elem(
                                char_x,
                                pos_y,
                                &cur_font,
                                cur_text_color,
                                cur_text_align,
                                &ch_str,
                            ));
                            if i < dxs.len() {
                                char_x += dxs[i];
                            }
                        }
                        cur_x = char_x;
                    } else if !chars.is_empty() {
                        let esc: String = chars.iter().map(|&c| format_svg_char(c)).collect();
                        svg_elements.push(render_svg_text_elem(
                            pos_x,
                            pos_y,
                            &cur_font,
                            cur_text_color,
                            cur_text_align,
                            &esc,
                        ));
                    }
                }
            }
            _ => {}
        }

        offset += record_bytes;
    }

    if !has_apm && !has_win_ext {
        width = width.max(1);
        height = height.max(1);
    }

    if svg_elements.is_empty() {
        svg_elements.push(format!(
            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="none" stroke="currentColor" stroke-dasharray="4,4"/>"#,
            min_x, min_y, width, height
        ));
    }

    let (width_px, height_px) = if has_apm && inch > 0 {
        (
            ((width as f32 / inch as f32) * 96.0).max(4.0),
            ((height as f32 / inch as f32) * 96.0).max(4.0),
        )
    } else {
        let h = ((height as f32 / width.max(1) as f32) * 60.0).clamp(18.0, 120.0);
        let w = (h * (width as f32 / height.max(1) as f32)).clamp(20.0, 600.0);
        (w, h)
    };

    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" width="{:.1}" height="{:.1}" style="max-width: 100%; vertical-align: middle; display: inline-block;">{}</svg>"#,
        min_x,
        min_y,
        width.max(1),
        height.max(1),
        width_px,
        height_px,
        svg_elements.join("")
    );

    text_runs.sort_by_key(|(y, x, _)| (*y / 40, *x));
    let plain_text = text_runs
        .into_iter()
        .map(|(_, _, t)| t)
        .collect::<Vec<_>>()
        .join("");

    Some(WmfParseOutput { svg, plain_text })
}

fn render_svg_text_elem(
    x: i32,
    y: i32,
    font: &GdiFont,
    color: (u8, u8, u8),
    align: u16,
    escaped_text: &str,
) -> String {
    let italic_attr = if font.italic { r#" font-style="italic""# } else { "" };
    let weight_attr = if font.weight >= 700 { r#" font-weight="bold""# } else { "" };
    let baseline = match align & 0x18 {
        0x18 => r#" dominant-baseline="alphabetic""#,
        0x08 => r#" dominant-baseline="text-after-edge""#,
        _ => "",
    };
    let anchor = match align & 0x06 {
        0x06 => r#" text-anchor="middle""#,
        0x02 => r#" text-anchor="end""#,
        _ => "",
    };
    format!(
        r#"<text x="{}" y="{}" font-family="Cambria Math, Times New Roman, serif" font-size="{}" fill="{}"{}{}{}{}>{}</text>"#,
        x,
        y,
        font.height.max(1),
        color_to_css(color),
        italic_attr,
        weight_attr,
        baseline,
        anchor,
        escaped_text
    )
}
