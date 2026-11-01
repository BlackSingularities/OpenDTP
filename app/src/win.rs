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
}

pub fn fullscreen(ui: &AppWindow, on: bool) {
    ui.window().set_fullscreen(on);
}
