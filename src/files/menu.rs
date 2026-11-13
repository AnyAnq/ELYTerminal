use std::path::PathBuf;

use ely_gpui_component::{
    i18n::text,
    menus::{Menu, MenuItem},
    primitives::IconName,
};
use gpui::{App, ClipboardItem, Entity};

use super::Files;

/// A Files menu action at its node's folder.
pub enum Here {
    NewSession(PathBuf),
    ChangeDirectory(PathBuf),
}

/// A right press's menu at `path` and its folder.
pub fn row(files: &Entity<Files>, path: PathBuf, here: PathBuf, cx: &App) -> Menu {
    let (open, host, copy) = (path.clone(), files.clone(), path);
    let (navigate, folder) = (files.clone(), here.clone());
    Menu::new()
        .item(
            MenuItem::new(text(cx, "et.files.open", &[]))
                .icon(IconName::ExternalLink)
                .on_click(move |_, cx| {
                    log::info!("files: opening {}", open.display());
                    cx.open_with_system(&open);
                }),
        )
        .item(
            MenuItem::new(text(cx, "et.files.change_directory", &[]))
                .icon(IconName::Folder)
                .on_click(move |_, cx| {
                    navigate.update(cx, |_, cx| cx.emit(Here::ChangeDirectory(folder.clone())))
                }),
        )
        .item(
            MenuItem::new(text(cx, "et.files.new_session", &[]))
                .icon(IconName::Terminal)
                .on_click(move |_, cx| {
                    host.update(cx, |_, cx| cx.emit(Here::NewSession(here.clone())))
                }),
        )
        .separator()
        .item(
            MenuItem::new(text(cx, "et.files.copy_path", &[]))
                .icon(IconName::Copy)
                .on_click(move |_, cx| match copy.to_str() {
                    Some(text) => {
                        log::info!("files: copied {text}");
                        cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
                    }
                    None => log::error!("files: {} is no UTF-8 to copy", copy.display()),
                }),
        )
}
