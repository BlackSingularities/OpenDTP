//! Integracja z Windows: ciemny pasek tytułu w kolorze tła (DWM) i pełny ekran.

use slint::winit_030::WinitWindowAccessor;
use slint::ComponentHandle;

use crate::AppWindow;

pub fn hwnd(ui: &AppWindow) -> isize {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    ui.window()
        .with_winit_window(|w| match w.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
            _ => 0,
        })
        .unwrap_or(0)
}

/// Pasek tytułu: ciemny (AMOLED – czarny, grafit – grafitowy) albo jasny.
pub fn dark_title(ui: &AppWindow, dark: bool) {
    let h = hwnd(ui);
    if h == 0 {
        return;
    }
    unsafe {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE};
        let hw = HWND(h as *mut _);
        let on: i32 = if dark { 1 } else { 0 };
        let _ = DwmSetWindowAttribute(hw, DWMWA_USE_IMMERSIVE_DARK_MODE, &on as *const _ as *const _, 4);
        let mode = ui.global::<crate::Theme>().get_mode();
        // COLORREF: 0x00BBGGRR.
        let caption: u32 = match mode {
            0 => 0x0000_0000,
            1 => 0x0028_2423,
            _ => 0x00FF_FFFF,
        };
        let text: u32 = if dark { 0x00FF_FFFF } else { 0x001C_1818 };
        let _ = DwmSetWindowAttribute(hw, DWMWA_CAPTION_COLOR, &caption as *const _ as *const _, 4);
        let _ = DwmSetWindowAttribute(hw, DWMWA_TEXT_COLOR, &text as *const _ as *const _, 4);
    }
    backdrop(ui, h);
}

/// Rozmycie pod przezroczystym interfejsem: akryl Windows 11 (DWMSBT_TRANSIENTWINDOW) rozciągnięty
/// na całe okno. Bez przezroczystości – zwykłe, nieprzepuszczające tło. Starsze Windows ignorują.
fn backdrop(ui: &AppWindow, h: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWINDOWATTRIBUTE};
    use windows::Win32::UI::Controls::MARGINS;
    let on = ui.global::<crate::Theme>().get_clear() > 0.0;
    let hw = HWND(h as *mut _);
    // 38 – DWMWA_SYSTEMBACKDROP_TYPE; 1 – brak, 3 – akryl.
    let kind: i32 = if on { 3 } else { 1 };
    let m = if on { -1 } else { 0 };
    let margins = MARGINS { cxLeftWidth: m, cxRightWidth: m, cyTopHeight: m, cyBottomHeight: m };
    unsafe {
        let _ = DwmExtendFrameIntoClientArea(hw, &margins);
        let _ = DwmSetWindowAttribute(hw, DWMWINDOWATTRIBUTE(38), &kind as *const _ as *const _, 4);
    }
}

/// Polecenia okna bez ramki: przeciąganie, minimalizacja, maksymalizacja, zamknięcie, zmiana rozmiaru.
pub fn action(ui: &AppWindow, a: i32) {
    use slint::winit_030::winit::window::ResizeDirection;
    match a {
        0 => {
            ui.window().with_winit_window(|w| {
                let _ = w.drag_window();
            });
        }
        1 => ui.window().set_minimized(true),
        2 => {
            let m = !ui.window().is_maximized();
            ui.window().set_maximized(m);
            ui.global::<crate::App>().set_maximized(m);
        }
        3 => ui.window().dispatch_event(slint::platform::WindowEvent::CloseRequested),
        11..=18 => {
            let dir = match a {
                11 => ResizeDirection::North,
                12 => ResizeDirection::South,
                13 => ResizeDirection::West,
                14 => ResizeDirection::East,
                15 => ResizeDirection::NorthWest,
                16 => ResizeDirection::NorthEast,
                17 => ResizeDirection::SouthWest,
                _ => ResizeDirection::SouthEast,
            };
            ui.window().with_winit_window(|w| {
                let _ = w.drag_resize_window(dir);
            });
        }
        _ => {}
    }
}

pub fn fullscreen(ui: &AppWindow, on: bool) {
    ui.window().set_fullscreen(on);
}

/// Tryby natywne monitorów z EDID zapisanych w rejestrze: (szerokość px, wysokość px, szerokość mm,
/// wysokość mm). Uruchamia `reg.exe` – wołać w tle.
pub static EDID: std::sync::OnceLock<Vec<(u32, u32, u32, u32)>> = std::sync::OnceLock::new();

pub fn edid_modes() -> Vec<(u32, u32, u32, u32)> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Ok(out) = std::process::Command::new("reg")
        .args(["query", r"HKLM\SYSTEM\CurrentControlSet\Enum\DISPLAY", "/s", "/v", "EDID"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut modes = Vec::new();
    for line in text.lines() {
        let Some(hex) = line.split("REG_BINARY").nth(1).map(str::trim) else { continue };
        let b: Vec<u8> = (0..hex.len() / 2).filter_map(|i| u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()).collect();
        if b.len() < 128 || b[..8] != [0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0] {
            continue;
        }
        // Pierwszy opis trybu (bajt 54) – tryb natywny i fizyczny rozmiar obrazu w mm.
        let d = &b[54..72];
        let ha = d[2] as u32 | ((d[4] as u32 & 0xf0) << 4);
        let va = d[5] as u32 | ((d[7] as u32 & 0xf0) << 4);
        let hmm = d[12] as u32 | ((d[14] as u32 & 0xf0) << 4);
        let vmm = d[13] as u32 | ((d[14] as u32 & 0x0f) << 8);
        if hmm >= 100 && vmm >= 60 {
            modes.push((ha, va, hmm, vmm));
        }
    }
    modes
}

/// Fizyczna gęstość ekranu (piksele na cal) dla monitora o rozdzielczości `w`×`h` px fizycznych.
/// `None`, gdy żaden monitor jej nie ma albo kilka podaje różne wymiary.
pub fn screen_ppi(modes: &[(u32, u32, u32, u32)], w: u32, h: u32) -> Option<f32> {
    let found: Vec<f32> = modes
        .iter()
        .filter_map(|&(ha, va, hmm, vmm)| {
            // Ekran obrócony – pion i poziom zamienione.
            let (px, mm) = if (ha, va) == (w, h) { (ha, hmm) } else if (va, ha) == (w, h) { (va, vmm) } else { return None };
            Some(px as f32 / (mm as f32 / 25.4))
        })
        .filter(|p| (40.0..=600.0).contains(p))
        .collect();
    let first = *found.first()?;
    found.iter().all(|p| (p - first).abs() < 2.0).then_some(first)
}

/// Rozdzielczość monitora, na którym jest okno (piksele fizyczne).
pub fn monitor_size(ui: &AppWindow) -> Option<(u32, u32)> {
    ui.window().with_winit_window(|w| w.current_monitor().map(|m| (m.size().width, m.size().height))).flatten()
}

/// Komunikat po awarii programu: czy dokument udało się zachować i gdzie jest dziennik.
pub fn crash_message(saved: bool) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let what = if saved {
        crate::tr("Dokument został zachowany i otworzy się przy następnym uruchomieniu.")
    } else {
        crate::tr("Przy następnym uruchomieniu program otworzy ostatnią kopię zapisaną automatycznie.")
    };
    let text = format!(
        "{}\n\n{}\n\n{}\n{}",
        crate::tr("OpenDTP napotkał nieoczekiwany błąd i musi zostać zamknięty."),
        what,
        crate::tr("Szczegóły błędu zapisano w dzienniku:"),
        opendtp_core::log::file().display()
    );
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (t, c) = (wide(&text), wide("OpenDTP"));
    unsafe {
        MessageBoxW(None, PCWSTR(t.as_ptr()), PCWSTR(c.as_ptr()), MB_OK | MB_ICONERROR);
    }
}

/// Czy proces o tym numerze wciąż działa (np. druga kopia programu z własną kopią odzyskiwania).
pub fn process_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    if pid == std::process::id() {
        return true;
    }
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return false };
        let mut code = 0u32;
        let alive = GetExitCodeProcess(h, &mut code).is_ok() && code == STILL_ACTIVE.0 as u32;
        let _ = CloseHandle(h);
        alive
    }
}
