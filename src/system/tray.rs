use crate::error::{FlashError, Result};
use image::ImageFormat;
use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub fn create_tray_icon() -> Result<TrayIcon> {
    let icon_data = include_bytes!("../../assets/logo.png");
    let image = image::load_from_memory_with_format(icon_data, ImageFormat::Png)
        .map_err(|e| FlashError::config("tray_icon", e.to_string()))?
        .into_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();
    let icon = Icon::from_rgba(rgba, width as u32, height as u32)
        .map_err(|e| FlashError::config("tray_icon", e.to_string()))?;

    let tray_menu = Menu::new();
    let show_i = MenuItem::with_id("show", "Show Flash Search", true, None);
    let quit_i = MenuItem::with_id("quit", "Quit", true, None);

    let _ = tray_menu.append(&show_i);
    let _ = tray_menu.append(&PredefinedMenuItem::separator());
    let _ = tray_menu.append(&quit_i);

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("Flash Search")
        .with_icon(icon)
        .build()
        .map_err(|e| FlashError::config("tray_icon", e.to_string()))?;

    Ok(tray_icon)
}
