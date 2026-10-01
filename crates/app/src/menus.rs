use crate::actions::*;
use crate::i18n::t;
use gpui::{App, Menu, MenuItem, OsAction};

pub fn set(cx: &mut App) {
  cx.set_menus(vec![
    Menu {
      name: "Asylum".into(),
      items: vec![
        MenuItem::action(t("About Asylum"), About),
        MenuItem::separator(),
        MenuItem::action(t("Settings…"), OpenSettings),
        MenuItem::separator(),
        MenuItem::action(t("Quit Asylum"), Quit),
      ],
      disabled: false,
    },
    Menu {
      name: t("File").into(),
      items: vec![
        MenuItem::action(t("New Chat"), NewChat),
        MenuItem::action(t("Search…"), Palette),
        MenuItem::separator(),
        MenuItem::action(t("Close Window"), CloseWindow),
      ],
      disabled: false,
    },
    Menu {
      name: t("Edit").into(),
      items: vec![
        MenuItem::os_action(t("Undo"), gpui::NoAction, OsAction::Undo),
        MenuItem::os_action(t("Redo"), gpui::NoAction, OsAction::Redo),
        MenuItem::separator(),
        MenuItem::os_action(t("Cut"), gpui::NoAction, OsAction::Cut),
        MenuItem::os_action(t("Copy"), gpui::NoAction, OsAction::Copy),
        MenuItem::os_action(t("Paste"), gpui::NoAction, OsAction::Paste),
        MenuItem::os_action(t("Select All"), gpui::NoAction, OsAction::SelectAll),
        MenuItem::separator(),
        MenuItem::action(t("Find in Chat"), FindInChat),
      ],
      disabled: false,
    },
    Menu {
      name: t("View").into(),
      items: vec![
        MenuItem::action(t("Toggle Sidebar"), ToggleSidebar),
        MenuItem::action(t("Conversation Details"), Details),
        MenuItem::action(t("Open Computer"), OpenComputer),
        MenuItem::action(t("Marketplace"), Marketplace),
        MenuItem::separator(),
        MenuItem::action(t("Zoom In"), ZoomIn),
        MenuItem::action(t("Zoom Out"), ZoomOut),
        MenuItem::action(t("Actual Size"), ZoomReset),
        MenuItem::separator(),
        MenuItem::action(t("Enter Full Screen"), Fullscreen),
      ],
      disabled: false,
    },
    Menu {
      name: t("Go").into(),
      items: vec![
        MenuItem::action(t("Back"), Back),
        MenuItem::action(t("Forward"), Forward),
        MenuItem::separator(),
        MenuItem::action(t("Previous Bot"), PrevBot),
        MenuItem::action(t("Next Bot"), NextBot),
      ],
      disabled: false,
    },
  ]);
}
