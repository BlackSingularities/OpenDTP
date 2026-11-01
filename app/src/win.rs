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
