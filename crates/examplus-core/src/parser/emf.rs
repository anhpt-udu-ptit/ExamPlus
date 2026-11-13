use base64::Engine;
use std::collections::HashMap;

use super::wmf::{
    color_to_css, decode_gdi_bytes_with_font, escape_xml, format_svg_char, u32_colorref_to_rgb,
    GdiBrush, GdiFont, GdiPen,
};

/// Bộ chuyển đổi EMF (Enhanced Metafile) thuần Rust sang SVG vector cho hình vẽ hình học,
/// công thức ChemDraw và biểu đồ khoa học không phụ thuộc GDI+/LibreOffice.

#[derive(Debug, Clone)]
enum EmfGdiObject {
    Pen(GdiPen),
    Brush(GdiBrush),
    Font(GdiFont),
}

#[derive(Debug, Clone)]
struct DcState {
    xform: [f32; 6], // [m11, m12, m21, m22, dx, dy] row-major
    pen: GdiPen,
    brush: GdiBrush,
    font: GdiFont,
    text_color: (u8, u8, u8),
    text_align: u32,
    poly_fill_even_odd: bool,
    cur_pos: (f32, f32),
}

impl Default for DcState {
    fn default() -> Self {
        Self {
            xform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            pen: GdiPen::default(),
            brush: GdiBrush::default(),
            font: GdiFont::default(),
            text_color: (0, 0, 0),
            text_align: 0,
            poly_fill_even_odd: true,
            cur_pos: (0.0, 0.0),
        }
    }
}

fn mul_xform(a: [f32; 6], b: [f32; 6]) -> [f32; 6] {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

fn transform_pt(m: &[f32; 6], x: f32, y: f32) -> (f32, f32) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

fn xform_scale(m: &[f32; 6]) -> f32 {
    let sx = (m[0] * m[0] + m[1] * m[1]).sqrt();
    let sy = (m[2] * m[2] + m[3] * m[3]).sqrt();
    ((sx + sy) * 0.5).max(0.01)
}

fn read_u32(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off + 4)
        .and_then(|s| s.try_into().ok())
        .map(u32::from_le_bytes)
}

fn read_i32(b: &[u8], off: usize) -> Option<i32> {
    b.get(off..off + 4)
        .and_then(|s| s.try_into().ok())
        .map(i32::from_le_bytes)
}

fn read_i16(b: &[u8], off: usize) -> Option<i16> {
    b.get(off..off + 2)
        .and_then(|s| s.try_into().ok())
        .map(i16::from_le_bytes)
}

fn read_f32(b: &[u8], off: usize) -> Option<f32> {
    b.get(off..off + 4)
        .and_then(|s| s.try_into().ok())
        .map(f32::from_le_bytes)
}

pub fn emf_to_svg_data_url(bytes: &[u8]) -> Option<String> {
    let svg = emf_to_svg(bytes)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(svg.as_bytes());
    Some(format!("data:image/svg+xml;base64,{}", b64))
}

pub fn emf_to_svg(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 88 {
        return None;
    }
    let rec_type = read_u32(bytes, 0)?;
    let rec_size = read_u32(bytes, 4)? as usize;
    let sig = read_u32(bytes, 40)?;
    // EMR_HEADER == 1 và signature == 0x464D4520 (" EMF")
    if rec_type != 1 || rec_size < 88 || sig != 0x464D4520 {
        return None;
    }

    let b_left = read_i32(bytes, 8).unwrap_or(0) as f32;
    let b_top = read_i32(bytes, 12).unwrap_or(0) as f32;
    let b_right = read_i32(bytes, 16).unwrap_or(0) as f32;
    let b_bottom = read_i32(bytes, 20).unwrap_or(0) as f32;

    let f_left = read_i32(bytes, 24).unwrap_or(0) as f32;
    let f_top = read_i32(bytes, 28).unwrap_or(0) as f32;
    let f_right = read_i32(bytes, 32).unwrap_or(0) as f32;
    let f_bottom = read_i32(bytes, 36).unwrap_or(0) as f32;

    let mut dc = DcState::default();
    let mut dc_stack: Vec<DcState> = Vec::new();
    let mut handles: HashMap<u32, EmfGdiObject> = HashMap::new();
    let mut svg_elements: Vec<String> = Vec::new();

    let mut in_path = false;
    let mut path_d = String::new();
    let mut path_last_pt = (0.0f32, 0.0f32);

    let mut obs_min_x = f32::INFINITY;
    let mut obs_min_y = f32::INFINITY;
    let mut obs_max_x = f32::NEG_INFINITY;
    let mut obs_max_y = f32::NEG_INFINITY;
    let mut track_pt = |px: f32, py: f32| {
        if px.is_finite() && py.is_finite() {
            obs_min_x = obs_min_x.min(px);
            obs_min_y = obs_min_y.min(py);
            obs_max_x = obs_max_x.max(px);
            obs_max_y = obs_max_y.max(py);
        }
    };

    let stroke_attr = |dc: &DcState| -> (String, f32, &'static str) {
        if (dc.pen.style & 0xF) == 5 {
            ("none".to_string(), 0.0, "")
        } else {
            let sw = (dc.pen.width.max(1.0) * xform_scale(&dc.xform)).max(1.0);
            let dash = match dc.pen.style & 0xF {
                1 => r#" stroke-dasharray="6,4""#,
                2 => r#" stroke-dasharray="2,3""#,
                3 => r#" stroke-dasharray="6,3,2,3""#,
                _ => "",
            };
            (color_to_css(dc.pen.color), sw, dash)
        }
    };

    let fill_attr = |dc: &DcState| -> String {
        if dc.brush.style == 1 {
            "none".to_string()
        } else {
            color_to_css(dc.brush.color)
        }
    };

    let mut offset = rec_size;
    while offset + 8 <= bytes.len() {
        let rt = read_u32(bytes, offset).unwrap_or(0);
        let rs = read_u32(bytes, offset + 4).unwrap_or(0) as usize;
        if rs < 8 || offset + rs > bytes.len() {
            break;
        }
        let rec = &bytes[offset..offset + rs];

        match rt {
            // EMR_EOF
            14 => break,
            // EMR_SETPOLYFILLMODE
            19 if rs >= 12 => {
                dc.poly_fill_even_odd = read_u32(rec, 8).unwrap_or(1) == 1;
            }
            // EMR_SETTEXTALIGN
            22 if rs >= 12 => {
                dc.text_align = read_u32(rec, 8).unwrap_or(0);
            }
            // EMR_SETTEXTCOLOR
            24 if rs >= 12 => {
                dc.text_color = u32_colorref_to_rgb(read_u32(rec, 8).unwrap_or(0));
            }
            // EMR_MOVETOEX
            27 if rs >= 16 => {
                let x = read_i32(rec, 8).unwrap_or(0) as f32;
                let y = read_i32(rec, 12).unwrap_or(0) as f32;
                let (px, py) = transform_pt(&dc.xform, x, y);
                dc.cur_pos = (px, py);
                if in_path {
                    path_d.push_str(&format!("M{:.2} {:.2} ", px, py));
                    path_last_pt = (px, py);
                    track_pt(px, py);
                }
            }
            // EMR_SAVEDC
            33 => dc_stack.push(dc.clone()),
            // EMR_RESTOREDC
            34 if rs >= 12 => {
                let n = read_i32(rec, 8).unwrap_or(-1);
                if n < 0 {
                    let count = (-n) as usize;
                    for _ in 0..count {
                        if let Some(prev) = dc_stack.pop() {
                            dc = prev;
                        }
                    }
                } else if (n as usize) < dc_stack.len() {
                    dc = dc_stack[n as usize].clone();
                    dc_stack.truncate(n as usize);
                }
            }
            // EMR_SETWORLDTRANSFORM
            35 if rs >= 32 => {
                dc.xform = [
                    read_f32(rec, 8).unwrap_or(1.0),
                    read_f32(rec, 12).unwrap_or(0.0),
                    read_f32(rec, 16).unwrap_or(0.0),
                    read_f32(rec, 20).unwrap_or(1.0),
                    read_f32(rec, 24).unwrap_or(0.0),
                    read_f32(rec, 28).unwrap_or(0.0),
                ];
            }
            // EMR_MODIFYWORLDTRANSFORM
            36 if rs >= 36 => {
                let xf = [
                    read_f32(rec, 8).unwrap_or(1.0),
                    read_f32(rec, 12).unwrap_or(0.0),
                    read_f32(rec, 16).unwrap_or(0.0),
                    read_f32(rec, 20).unwrap_or(1.0),
                    read_f32(rec, 24).unwrap_or(0.0),
                    read_f32(rec, 28).unwrap_or(0.0),
                ];
                let mode = read_u32(rec, 32).unwrap_or(1);
                dc.xform = match mode {
                    1 => [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                    2 => mul_xform(xf, dc.xform),
                    3 => mul_xform(dc.xform, xf),
                    4 => xf,
                    _ => dc.xform,
                };
            }
            // EMR_SELECTOBJECT
            37 if rs >= 12 => {
                let ih = read_u32(rec, 8).unwrap_or(0);
                if ih >= 0x8000_0000 {
                    match ih {
                        0x8000_0000 => dc.brush = GdiBrush { style: 0, color: (255, 255, 255) },
                        0x8000_0001 => dc.brush = GdiBrush { style: 0, color: (192, 192, 192) },
                        0x8000_0002 => dc.brush = GdiBrush { style: 0, color: (128, 128, 128) },
                        0x8000_0003 => dc.brush = GdiBrush { style: 0, color: (64, 64, 64) },
                        0x8000_0004 => dc.brush = GdiBrush { style: 0, color: (0, 0, 0) },
                        0x8000_0005 => dc.brush = GdiBrush { style: 1, color: (0, 0, 0) }, // NULL_BRUSH
                        0x8000_0006 => dc.pen = GdiPen { style: 0, width: 1.0, color: (255, 255, 255) },
                        0x8000_0007 => dc.pen = GdiPen { style: 0, width: 1.0, color: (0, 0, 0) },
                        0x8000_0008 => dc.pen = GdiPen { style: 5, width: 0.0, color: (0, 0, 0) }, // NULL_PEN
                        _ => {}
                    }
                } else if let Some(obj) = handles.get(&ih) {
                    match obj {
                        EmfGdiObject::Pen(p) => dc.pen = p.clone(),
                        EmfGdiObject::Brush(b) => dc.brush = b.clone(),
                        EmfGdiObject::Font(f) => dc.font = f.clone(),
                    }
                }
            }
            // EMR_CREATEPEN
            38 if rs >= 28 => {
                let ih = read_u32(rec, 8).unwrap_or(0);
                let style = read_u32(rec, 12).unwrap_or(0);
                let width = read_i32(rec, 16).unwrap_or(1).unsigned_abs() as f32;
                let color = u32_colorref_to_rgb(read_u32(rec, 24).unwrap_or(0));
                handles.insert(
                    ih,
                    EmfGdiObject::Pen(GdiPen {
                        style,
                        width: width.max(1.0),
                        color,
                    }),
                );
            }
            // EMR_CREATEBRUSHINDIRECT
            39 if rs >= 24 => {
                let ih = read_u32(rec, 8).unwrap_or(0);
                let style = read_u32(rec, 12).unwrap_or(0);
                let color = u32_colorref_to_rgb(read_u32(rec, 16).unwrap_or(0));
                handles.insert(ih, EmfGdiObject::Brush(GdiBrush { style, color }));
            }
            // EMR_DELETEOBJECT
            40 if rs >= 12 => {
                let ih = read_u32(rec, 8).unwrap_or(0);
                handles.remove(&ih);
            }
            // EMR_ELLIPSE (42) / EMR_RECTANGLE (43)
            42 | 43 if rs >= 24 => {
                let l = read_i32(rec, 8).unwrap_or(0) as f32;
                let t = read_i32(rec, 12).unwrap_or(0) as f32;
                let r = read_i32(rec, 16).unwrap_or(0) as f32;
                let b = read_i32(rec, 20).unwrap_or(0) as f32;
                let (x0, y0) = transform_pt(&dc.xform, l, t);
                let (x1, y1) = transform_pt(&dc.xform, r, b);
                track_pt(x0, y0);
                track_pt(x1, y1);
                let (st, sw, dash) = stroke_attr(&dc);
                let fill = fill_attr(&dc);
                if rt == 42 {
                    let cx = (x0 + x1) * 0.5;
                    let cy = (y0 + y1) * 0.5;
                    let rx = ((x1 - x0) * 0.5).abs().max(0.5);
                    let ry = ((y1 - y0) * 0.5).abs().max(0.5);
                    svg_elements.push(format!(
                        r#"<ellipse cx="{:.2}" cy="{:.2}" rx="{:.2}" ry="{:.2}" fill="{}" stroke="{}" stroke-width="{:.2}"{}/>"#,
                        cx, cy, rx, ry, fill, st, sw, dash
                    ));
                } else {
                    svg_elements.push(format!(
                        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" stroke="{}" stroke-width="{:.2}"{}/>"#,
                        x0.min(x1),
                        y0.min(y1),
                        (x1 - x0).abs().max(0.5),
                        (y1 - y0).abs().max(0.5),
                        fill,
                        st,
                        sw,
                        dash
                    ));
                }
            }
            // EMR_LINETO
            54 if rs >= 16 => {
                let x = read_i32(rec, 8).unwrap_or(0) as f32;
                let y = read_i32(rec, 12).unwrap_or(0) as f32;
                let (px, py) = transform_pt(&dc.xform, x, y);
                track_pt(dc.cur_pos.0, dc.cur_pos.1);
                track_pt(px, py);
                if in_path {
                    path_d.push_str(&format!("L{:.2} {:.2} ", px, py));
                    path_last_pt = (px, py);
                } else {
                    let (st, sw, dash) = stroke_attr(&dc);
                    if st != "none" {
                        svg_elements.push(format!(
                            r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="{:.2}" stroke-linecap="round"{}/>"#,
                            dc.cur_pos.0, dc.cur_pos.1, px, py, st, sw, dash
                        ));
                    }
                }
                dc.cur_pos = (px, py);
            }
            // EMR_BEGINPATH
            59 => {
                in_path = true;
                path_d.clear();
                path_last_pt = dc.cur_pos;
            }
            // EMR_ENDPATH
            60 => {
                in_path = false;
            }
            // EMR_CLOSEFIGURE
            61 => {
                if !path_d.is_empty() {
                    path_d.push_str("Z ");
                }
            }
            // EMR_FILLPATH (62) | EMR_STROKEANDFILLPATH (63) | EMR_STROKEPATH (64)
            62 | 63 | 64 => {
                if !path_d.is_empty() {
                    let fill = if rt == 64 {
                        "none".to_string()
                    } else {
                        fill_attr(&dc)
                    };
                    let (st, sw, dash) = if rt == 62 {
                        ("none".to_string(), 0.0, "")
                    } else {
                        stroke_attr(&dc)
                    };
                    let rule = if dc.poly_fill_even_odd {
                        r#" fill-rule="evenodd""#
                    } else {
                        ""
                    };
                    svg_elements.push(format!(
                        r#"<path d="{}" fill="{}" stroke="{}" stroke-width="{:.2}"{}{}/>"#,
                        path_d.trim(),
                        fill,
                        st,
                        sw,
                        dash,
                        rule
                    ));
                    path_d.clear();
                }
            }
            // EMR_ABORTPATH
            68 => {
                in_path = false;
                path_d.clear();
            }
            // EMR_EXTCREATEFONTINDIRECTW
            82 if rs >= 104 => {
                let ih = read_u32(rec, 8).unwrap_or(0);
                let lf_height = read_i32(rec, 12).unwrap_or(-200);
                let lf_weight = read_i32(rec, 28).unwrap_or(400);
                let lf_italic = rec.get(32).copied().unwrap_or(0) != 0;
                let lf_underline = rec.get(33).copied().unwrap_or(0) != 0;
                let lf_charset = rec.get(35).copied().unwrap_or(0);
                let face_raw = &rec[40..104.min(rs)];
                let mut u16s = Vec::with_capacity(32);
                for chunk in face_raw.chunks_exact(2) {
                    let v = u16::from_le_bytes([chunk[0], chunk[1]]);
                    if v == 0 {
                        break;
                    }
                    u16s.push(v);
                }
                let face = String::from_utf16_lossy(&u16s).trim().to_string();
                handles.insert(
                    ih,
                    EmfGdiObject::Font(GdiFont {
                        height: lf_height.abs().max(1),
                        weight: lf_weight,
                        italic: lf_italic,
                        underline: lf_underline,
                        charset: lf_charset,
                        face,
                    }),
                );
            }
            // EMR_EXTTEXTOUTA (83) / EMR_EXTTEXTOUTW (84)
            83 | 84 if rs >= 76 => {
                let rx = read_i32(rec, 36).unwrap_or(0) as f32;
                let ry = read_i32(rec, 40).unwrap_or(0) as f32;
                let nch = read_u32(rec, 44).unwrap_or(0) as usize;
                let off_str = read_u32(rec, 48).unwrap_or(0) as usize;
                let is_wide = rt == 84;
                let byte_len = if is_wide { nch.saturating_mul(2) } else { nch };
                if nch > 0 && off_str + byte_len <= rs {
                    let slice = &rec[off_str..off_str + byte_len];
                    let text_str = if is_wide {
                        let u16s: Vec<u16> = slice
                            .chunks_exact(2)
                            .map(|c| u16::from_le_bytes([c[0], c[1]]))
                            .collect();
                        let s = String::from_utf16_lossy(&u16s);
                        // Nếu font là Symbol/MT Extra với codepoint < 256 -> giải mã qua bảng ký hiệu GDI
                        let fl = dc.font.face.to_ascii_lowercase();
                        if fl.contains("symbol") || fl.contains("mt extra") || fl.contains("euclid extra") {
                            let bytes_u8: Vec<u8> = u16s.iter().map(|&v| (v & 0xFF) as u8).collect();
                            decode_gdi_bytes_with_font(&bytes_u8, &dc.font.face, dc.font.charset)
                                .into_iter()
                                .map(format_svg_char)
                                .collect::<String>()
                        } else {
                            escape_xml(&s)
                        }
                    } else {
                        decode_gdi_bytes_with_font(slice, &dc.font.face, dc.font.charset)
                            .into_iter()
                            .map(format_svg_char)
                            .collect::<String>()
                    };

                    if !text_str.is_empty() {
                        let (px, py) = transform_pt(&dc.xform, rx, ry);
                        track_pt(px, py);
                        let fsize = (dc.font.height as f32 * xform_scale(&dc.xform)).max(6.0);
                        let baseline = match dc.text_align & 0x18 {
                            0x18 => r#" dominant-baseline="alphabetic""#,
                            0x08 => r#" dominant-baseline="text-after-edge""#,
                            _ => r#" dominant-baseline="hanging""#,
                        };
                        let anchor = match dc.text_align & 0x06 {
                            0x06 => r#" text-anchor="middle""#,
                            0x02 => r#" text-anchor="end""#,
                            _ => "",
                        };
                        let italic = if dc.font.italic { r#" font-style="italic""# } else { "" };
                        let weight = if dc.font.weight >= 700 { r#" font-weight="bold""# } else { "" };
                        let family = if dc.font.face.is_empty() {
                            "Cambria Math, Times New Roman, serif".to_string()
                        } else {
                            format!("{}, Cambria Math, serif", escape_xml(&dc.font.face))
                        };
                        svg_elements.push(format!(
                            r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}"{}{}{}{}>{}</text>"#,
                            px,
                            py,
                            family,
                            fsize,
                            color_to_css(dc.text_color),
                            italic,
                            weight,
                            baseline,
                            anchor,
                            text_str
                        ));
                    }
                }
            }
            // EMR_POLYLINE16 (87) / EMR_POLYLINETO16 (89) / EMR_POLYGON16 (86) / EMR_POLYBEZIER16 (85) / EMR_POLYBEZIERTO16 (88)
            85 | 86 | 87 | 88 | 89 if rs >= 28 => {
                let c_pts = read_u32(rec, 24).unwrap_or(0) as usize;
                if c_pts > 0 && rs >= 28 + c_pts * 4 {
                    let mut pts = Vec::with_capacity(c_pts);
                    for i in 0..c_pts {
                        let x = read_i16(rec, 28 + i * 4).unwrap_or(0) as f32;
                        let y = read_i16(rec, 30 + i * 4).unwrap_or(0) as f32;
                        let (px, py) = transform_pt(&dc.xform, x, y);
                        track_pt(px, py);
                        pts.push((px, py));
                    }
                    if in_path {
                        match rt {
                            87 | 86 => {
                                if let Some(&(x0, y0)) = pts.first() {
                                    path_d.push_str(&format!("M{:.2} {:.2} ", x0, y0));
                                    for &(px, py) in &pts[1..] {
                                        path_d.push_str(&format!("L{:.2} {:.2} ", px, py));
                                    }
                                    if rt == 86 {
                                        path_d.push_str("Z ");
                                    }
                                    path_last_pt = *pts.last().unwrap();
                                }
                            }
                            89 => {
                                for &(px, py) in &pts {
                                    path_d.push_str(&format!("L{:.2} {:.2} ", px, py));
                                }
                                if let Some(&last) = pts.last() {
                                    path_last_pt = last;
                                }
                            }
                            85 => {
                                if pts.len() >= 4 {
                                    let (x0, y0) = pts[0];
                                    if path_d.is_empty() || path_last_pt != (x0, y0) {
                                        path_d.push_str(&format!("M{:.2} {:.2} ", x0, y0));
                                    }
                                    for chunk in pts[1..].chunks_exact(3) {
                                        path_d.push_str(&format!(
                                            "C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} ",
                                            chunk[0].0, chunk[0].1, chunk[1].0, chunk[1].1, chunk[2].0, chunk[2].1
                                        ));
                                        path_last_pt = chunk[2];
                                    }
                                }
                            }
                            88 => {
                                for chunk in pts.chunks_exact(3) {
                                    path_d.push_str(&format!(
                                        "C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} ",
                                        chunk[0].0, chunk[0].1, chunk[1].0, chunk[1].1, chunk[2].0, chunk[2].1
                                    ));
                                    path_last_pt = chunk[2];
                                }
                            }
                            _ => {}
                        }
                    } else if rt == 87 || rt == 89 {
                        let (st, sw, dash) = stroke_attr(&dc);
                        if st != "none" {
                            let pt_str = pts
                                .iter()
                                .map(|(x, y)| format!("{:.2},{:.2}", x, y))
                                .collect::<Vec<_>>()
                                .join(" ");
                            svg_elements.push(format!(
                                r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="{:.2}" stroke-linecap="round"{}/>"#,
                                pt_str, st, sw, dash
                            ));
                        }
                    } else if rt == 86 {
                        let (st, sw, dash) = stroke_attr(&dc);
                        let fill = fill_attr(&dc);
                        let pt_str = pts
                            .iter()
                            .map(|(x, y)| format!("{:.2},{:.2}", x, y))
                            .collect::<Vec<_>>()
                            .join(" ");
                        svg_elements.push(format!(
                            r#"<polygon points="{}" fill="{}" stroke="{}" stroke-width="{:.2}"{}/>"#,
                            pt_str, fill, st, sw, dash
                        ));
                    } else if (rt == 85 || rt == 88) && pts.len() >= 4 {
                        let mut d = format!("M{:.2} {:.2} ", pts[0].0, pts[0].1);
                        for chunk in pts[1..].chunks_exact(3) {
                            d.push_str(&format!(
                                "C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} ",
                                chunk[0].0, chunk[0].1, chunk[1].0, chunk[1].1, chunk[2].0, chunk[2].1
                            ));
                        }
                        let (st, sw, dash) = stroke_attr(&dc);
                        if st != "none" {
                            svg_elements.push(format!(
                                r#"<path d="{}" fill="none" stroke="{}" stroke-width="{:.2}"{}/>"#,
                                d.trim(),
                                st,
                                sw,
                                dash
                            ));
                        }
                    }
                }
            }
            // EMR_POLYPOLYGON16 (91) / EMR_POLYLINE (4) / EMR_POLYGON (3)
            91 if rs >= 32 => {
                let n_polys = read_u32(rec, 24).unwrap_or(0) as usize;
                let c_pts = read_u32(rec, 28).unwrap_or(0) as usize;
                let pts_off = 32 + n_polys * 4;
                if n_polys > 0 && rs >= pts_off + c_pts * 4 {
                    let mut d = String::new();
                    let mut p_idx = 0usize;
                    for poly_i in 0..n_polys {
                        let cnt = read_u32(rec, 32 + poly_i * 4).unwrap_or(0) as usize;
                        for local_i in 0..cnt {
                            if p_idx + local_i >= c_pts {
                                break;
                            }
                            let byte_i = pts_off + (p_idx + local_i) * 4;
                            let x = read_i16(rec, byte_i).unwrap_or(0) as f32;
                            let y = read_i16(rec, byte_i + 2).unwrap_or(0) as f32;
                            let (px, py) = transform_pt(&dc.xform, x, y);
                            track_pt(px, py);
                            if local_i == 0 {
                                d.push_str(&format!("M{:.2} {:.2} ", px, py));
                            } else {
                                d.push_str(&format!("L{:.2} {:.2} ", px, py));
                            }
                        }
                        d.push_str("Z ");
                        p_idx += cnt;
                    }
                    let fill = fill_attr(&dc);
                    let (st, sw, dash) = stroke_attr(&dc);
                    let rule = if dc.poly_fill_even_odd {
                        r#" fill-rule="evenodd""#
                    } else {
                        ""
                    };
                    svg_elements.push(format!(
                        r#"<path d="{}" fill="{}" stroke="{}" stroke-width="{:.2}"{}{}/>"#,
                        d.trim(),
                        fill,
                        st,
                        sw,
                        dash,
                        rule
                    ));
                }
            }
            // EMR_POLYLINE (4) / EMR_POLYGON (3) 32-bit coords
            3 | 4 if rs >= 28 => {
                let c_pts = read_u32(rec, 24).unwrap_or(0) as usize;
                if c_pts >= 2 && rs >= 28 + c_pts * 8 {
                    let mut pt_str = Vec::with_capacity(c_pts);
                    for i in 0..c_pts {
                        let x = read_i32(rec, 28 + i * 8).unwrap_or(0) as f32;
                        let y = read_i32(rec, 32 + i * 8).unwrap_or(0) as f32;
                        let (px, py) = transform_pt(&dc.xform, x, y);
                        track_pt(px, py);
                        pt_str.push(format!("{:.2},{:.2}", px, py));
                    }
                    let (st, sw, dash) = stroke_attr(&dc);
                    if rt == 4 {
                        if st != "none" {
                            svg_elements.push(format!(
                                r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="{:.2}"{}/>"#,
                                pt_str.join(" "),
                                st,
                                sw,
                                dash
                            ));
                        }
                    } else {
                        let fill = fill_attr(&dc);
                        svg_elements.push(format!(
                            r#"<polygon points="{}" fill="{}" stroke="{}" stroke-width="{:.2}"{}/>"#,
                            pt_str.join(" "),
                            fill,
                            st,
                            sw,
                            dash
                        ));
                    }
                }
            }
            // EMR_EXTCREATEPEN (95)
            95 if rs >= 48 => {
                let ih = read_u32(rec, 8).unwrap_or(0);
                let style = read_u32(rec, 28).unwrap_or(0);
                let width = read_u32(rec, 32).unwrap_or(1) as f32;
                let color = u32_colorref_to_rgb(read_u32(rec, 40).unwrap_or(0));
                handles.insert(
                    ih,
                    EmfGdiObject::Pen(GdiPen {
                        style,
                        width: width.max(1.0),
                        color,
                    }),
                );
            }
            _ => {}
        }

        offset += rs;
    }

    if svg_elements.is_empty() {
        return None;
    }

    // Xác định viewBox với lề nhỏ cho text / nét vẽ ngoài rìa rclBounds
    let (vx, vy, vw, vh) = if b_right > b_left && b_bottom > b_top {
        let mut x0 = b_left;
        let mut y0 = b_top;
        let mut x1 = b_right;
        let mut y1 = b_bottom;
        if obs_min_x.is_finite() {
            x0 = x0.min(obs_min_x);
            y0 = y0.min(obs_min_y);
            x1 = x1.max(obs_max_x);
            y1 = y1.max(obs_max_y);
        }
        let pad_x = ((x1 - x0) * 0.04).max(6.0);
        let pad_y = ((y1 - y0) * 0.08).max(6.0);
        (
            x0 - pad_x,
            y0 - pad_y,
            (x1 - x0 + 2.0 * pad_x).max(4.0),
            (y1 - y0 + 2.0 * pad_y).max(4.0),
        )
    } else if obs_min_x.is_finite() && obs_max_x > obs_min_x {
        (
            obs_min_x - 8.0,
            obs_min_y - 8.0,
            (obs_max_x - obs_min_x + 16.0).max(8.0),
            (obs_max_y - obs_min_y + 16.0).max(8.0),
        )
    } else {
        (0.0, 0.0, 200.0, 100.0)
    };

    let (w_px, h_px) = if f_right > f_left && f_bottom > f_top {
        (
            ((f_right - f_left) / 2540.0 * 96.0).max(8.0),
            ((f_bottom - f_top) / 2540.0 * 96.0).max(8.0),
        )
    } else {
        (vw.max(8.0), vh.max(8.0))
    };

    Some(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{:.1} {:.1} {:.1} {:.1}" width="{:.1}" height="{:.1}" style="max-width: 100%; vertical-align: middle; display: inline-block;">{}</svg>"#,
        vx,
        vy,
        vw,
        vh,
        w_px,
        h_px,
        svg_elements.join("")
    ))
}
