//! Metapliki Windows (WMF, EMF) – np. wzory MathType i cliparty w starszych dokumentach Worda.
//! Odtwarzamy je przez GDI do bitmapy w wysokiej rozdzielczości i zapisujemy jako PNG.

/// Rozdzielczość odtwarzania: pikseli na punkt (3 ≈ 216 dpi – ostro w druku i na ekranie).
const SCALE: f32 = 3.0;
/// Największy bok bitmapy w pikselach.
const MAX_PX: f32 = 4000.0;

/// Czy dane wyglądają na metaplik (WMF z nagłówkiem „placeable” albo bez, EMF).
pub fn is_metafile(data: &[u8]) -> bool {
    data.starts_with(&[0xd7, 0xcd, 0xc6, 0x9a]) || is_plain_wmf(data) || is_emf(data)
}

fn is_plain_wmf(data: &[u8]) -> bool {
    data.len() > 18 && matches!(data[0], 1 | 2) && data[1] == 0 && data[2] == 9 && data[3] == 0
}

fn is_emf(data: &[u8]) -> bool {
    data.len() > 88 && data[..4] == [1, 0, 0, 0] && &data[40..44] == b" EMF"
}

fn le_i16(d: &[u8], at: usize) -> i32 {
    i16::from_le_bytes([d[at], d[at + 1]]) as i32
}

fn le_i32(d: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

/// Metaplik → (PNG, szerokość pt, wysokość pt). `None`, gdy to nie metaplik albo GDI go nie odtworzy.
pub fn to_png(data: &[u8]) -> Option<(Vec<u8>, f32, f32)> {
    if !is_metafile(data) {
        return None;
    }
    // Wymiary w punktach: z nagłówka placeable (jednostki na cal) albo z ramki EMF (0,01 mm).
    let (body, w_pt, h_pt): (&[u8], f32, f32) = if data.starts_with(&[0xd7, 0xcd, 0xc6, 0x9a]) && data.len() > 22 {
        let (l, t, r, b) = (le_i16(data, 6), le_i16(data, 8), le_i16(data, 10), le_i16(data, 12));
        let inch = u16::from_le_bytes([data[14], data[15]]).max(1) as f32;
        (&data[22..], ((r - l).abs() as f32 / inch * 72.0).max(1.0), ((b - t).abs() as f32 / inch * 72.0).max(1.0))
    } else if is_emf(data) {
        let (l, t, r, b) = (le_i32(data, 24), le_i32(data, 28), le_i32(data, 32), le_i32(data, 36));
        (data, ((r - l).abs() as f32 / 100.0 / 25.4 * 72.0).max(1.0), ((b - t).abs() as f32 / 100.0 / 25.4 * 72.0).max(1.0))
    } else {
        (data, 200.0, 150.0)
    };
    let k = SCALE.min(MAX_PX / w_pt.max(h_pt));
    let (w, h) = ((w_pt * k).round().max(1.0) as i32, (h_pt * k).round().max(1.0) as i32);
    let rgba = render(body, is_emf(data), w, h, w_pt, h_pt)?;
    let img = image::RgbaImage::from_raw(w as u32, h as u32, rgba)?;
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
    Some((png, w_pt, h_pt))
}

#[cfg(windows)]
fn render(body: &[u8], emf: bool, w: i32, h: i32, w_pt: f32, h_pt: f32) -> Option<Vec<u8>> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::DataExchange::{SetWinMetaFileBits, METAFILEPICT};
    unsafe {
        let hemf = if emf {
            SetEnhMetaFileBits(body)
        } else {
            // Rozmiar obrazu w 0,01 mm – GDI dopasuje do niego układ współrzędnych WMF.
            let mfp = METAFILEPICT { mm: MM_ANISOTROPIC.0, xExt: (w_pt / 72.0 * 2540.0) as i32, yExt: (h_pt / 72.0 * 2540.0) as i32, hMF: HMETAFILE::default() };
            SetWinMetaFileBits(body, None, Some(&mfp))
        };
        if hemf.0.is_null() {
            return None;
        }
        let dc = CreateCompatibleDC(None);
        if dc.0.is_null() {
            let _ = DeleteEnhMetaFile(Some(hemf));
            return None;
        }
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                // Ujemna wysokość – wiersze od góry.
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let Ok(bmp) = CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) else {
            let _ = DeleteDC(dc);
            let _ = DeleteEnhMetaFile(Some(hemf));
            return None;
        };
        let old = SelectObject(dc, HGDIOBJ(bmp.0));
        let _ = PatBlt(dc, 0, 0, w, h, WHITENESS);
        let rect = RECT { left: 0, top: 0, right: w, bottom: h };
        let ok = PlayEnhMetaFile(dc, hemf, &rect).as_bool();
        let _ = GdiFlush();
        let mut out = None;
        if ok && !bits.is_null() {
            let n = (w as usize) * (h as usize) * 4;
            let src = std::slice::from_raw_parts(bits as *const u8, n);
            let mut rgba = Vec::with_capacity(n);
            for px in src.chunks_exact(4) {
                // BGRA z GDI (kanał alfa nieużywany) → RGBA; białe tło staje się przezroczyste.
                let (b, g, r) = (px[0], px[1], px[2]);
                let a = if r == 255 && g == 255 && b == 255 { 0 } else { 255 };
                rgba.extend_from_slice(&[r, g, b, a]);
            }
            out = Some(rgba);
        }
        SelectObject(dc, old);
        let _ = DeleteObject(HGDIOBJ(bmp.0));
        let _ = DeleteDC(dc);
        let _ = DeleteEnhMetaFile(Some(hemf));
        out
    }
}

#[cfg(not(windows))]
fn render(_body: &[u8], _emf: bool, _w: i32, _h: i32, _w_pt: f32, _h_pt: f32) -> Option<Vec<u8>> {
    None
}
