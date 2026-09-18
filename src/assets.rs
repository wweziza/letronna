use crate::prelude::*;

pub(crate) struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        macro_rules! svg { ($($n:literal),*) => { match path {
            $(concat!("icons/", $n, ".svg") => Some(Cow::Borrowed(include_bytes!(concat!("../assets/icons/", $n, ".svg")))),)*
            "images/letronna.png" => Some(Cow::Borrowed(&include_bytes!("../assets/letronna-front.png")[..])),
            _ => None,
        } } }
        Ok(svg!(
            "plus",
            "close",
            "window-close",
            "window-minimize",
            "window-maximize",
            "window-restore",
            "minus",
            "settings",
            "arrow-up",
            "copy",
            "panel-left-close",
            "panel-left-open",
            "chevron-down",
            "check",
            "loader-circle",
            "bot",
            "mic",
            "chevrons-up-down",
            "inbox",
            "send",
            "sparkles",
            "message-square",
            "file",
            "file-text",
            "clock",
            "search",
            "pin",
            "folder",
            "image",
            "square-pen",
            "info",
            "pin",
            "trash",
            "export",
            "pencil",
            "plugs",
            "palette",
            "list-filter"
        ))
    }
    fn list(&self, _: &str) -> Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

#[derive(Clone, Copy)]
pub(crate) enum AppIcon {
    Mic,
    Sparkles,
    MessageSquare,
    File,
    FileText,
    Clock,
    Search,
    Folder,
    Image,
    SquarePen,
    Info,
    Pin,
    Trash,
    Export,
    Pencil,
    Robot,
    Plugs,
    Palette,
}
impl IconNamed for AppIcon {
    fn path(self) -> SharedString {
        match self {
            AppIcon::Mic => "icons/mic.svg",
            AppIcon::Sparkles => "icons/sparkles.svg",
            AppIcon::MessageSquare => "icons/message-square.svg",
            AppIcon::File => "icons/file.svg",
            AppIcon::FileText => "icons/file-text.svg",
            AppIcon::Clock => "icons/clock.svg",
            AppIcon::Search => "icons/search.svg",
            AppIcon::Folder => "icons/folder.svg",
            AppIcon::Image => "icons/image.svg",
            AppIcon::SquarePen => "icons/square-pen.svg",
            AppIcon::Info => "icons/info.svg",
            AppIcon::Pin => "icons/pin.svg",
            AppIcon::Trash => "icons/trash.svg",
            AppIcon::Export => "icons/export.svg",
            AppIcon::Pencil => "icons/pencil.svg",
            AppIcon::Robot => "icons/bot.svg",
            AppIcon::Plugs => "icons/plugs.svg",
            AppIcon::Palette => "icons/palette.svg",
        }
        .into()
    }
}
