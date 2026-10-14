use futures::channel::mpsc::UnboundedSender;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

#[derive(Debug, Clone, Copy)]
pub enum Action {
    ShowWindow,
    Quit,
}

#[cfg(not(target_os = "linux"))]
pub struct TrayLifetime {
    icon: TrayIcon,
}

#[cfg(not(target_os = "linux"))]
impl TrayLifetime {
    pub fn set_language(&self, language: &str) {
        self.icon.set_menu(tray_menu(language));
        let _ = self.icon.set_tooltip(Some(tray_tooltip(language)));
    }
}

#[cfg(target_os = "linux")]
pub struct TrayLifetime {
    _thread: std::thread::JoinHandle<()>,
    language_sender: std::sync::mpsc::Sender<String>,
}

#[cfg(target_os = "linux")]
impl TrayLifetime {
    pub fn set_language(&self, language: &str) {
        let _ = self.language_sender.send(language.to_owned());
    }
}

#[cfg(not(target_os = "linux"))]
pub fn install(sender: UnboundedSender<Action>, language: &str) -> Option<TrayLifetime> {
    install_event_handlers(sender);
    build_tray(language).map(|icon| TrayLifetime { icon })
}

#[cfg(target_os = "linux")]
pub fn install(sender: UnboundedSender<Action>, language: &str) -> Option<TrayLifetime> {
    use std::sync::mpsc;
    use std::time::Duration;

    install_event_handlers(sender);
    let language = language.to_owned();
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let (language_sender, language_receiver) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("qq-farm-gpui-tray".into())
        .spawn(move || {
            if gtk::init().is_err() {
                let _ = ready_sender.send(false);
                return;
            }
            let Some(tray) = build_tray(&language) else {
                let _ = ready_sender.send(false);
                return;
            };
            if ready_sender.send(true).is_err() {
                return;
            }
            let _menu_language_poll =
                gtk::glib::timeout_add_local(Duration::from_millis(100), move || {
                    loop {
                        match language_receiver.try_recv() {
                            Ok(language) => tray.set_menu(tray_menu(&language)),
                            Err(mpsc::TryRecvError::Empty) => break,
                            Err(mpsc::TryRecvError::Disconnected) => {
                                gtk::main_quit();
                                return gtk::glib::ControlFlow::Break;
                            }
                        }
                    }
                    gtk::glib::ControlFlow::Continue
                });
            gtk::main();
        })
        .ok()?;
    if ready_receiver.recv_timeout(Duration::from_secs(2)).ok() == Some(true) {
        Some(TrayLifetime { _thread: thread, language_sender })
    } else {
        None
    }
}

fn install_event_handlers(sender: UnboundedSender<Action>) {
    let show_sender = sender.clone();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let activate = match event {
            TrayIconEvent::Click { button, button_state, .. } => {
                button == MouseButton::Left && button_state == MouseButtonState::Up
            }
            TrayIconEvent::DoubleClick { button, .. } => button == MouseButton::Left,
            _ => false,
        };
        if activate {
            let _ = show_sender.unbounded_send(Action::ShowWindow);
        }
    }));
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| match event.id().0.as_str() {
        "qqfarm-show" => {
            let _ = sender.unbounded_send(Action::ShowWindow);
        }
        "qqfarm-quit" => {
            let _ = sender.unbounded_send(Action::Quit);
        }
        _ => {}
    }));
}

fn tray_menu_labels(language: &str) -> (&'static str, &'static str) {
    (
        crate::i18n::text(language, "显示 QQ 农场", "Show QQ Farm"),
        crate::i18n::text(language, "退出", "Quit"),
    )
}

fn tray_tooltip(language: &str) -> &'static str {
    crate::i18n::text(language, "QQ 农场", "QQ Farm")
}

fn tray_menu(language: &str) -> Option<Box<dyn tray_icon::menu::ContextMenu>> {
    let (show_label, quit_label) = tray_menu_labels(language);
    let show = MenuItem::with_id("qqfarm-show", show_label, true, None);
    let separator = PredefinedMenuItem::separator();
    let quit = MenuItem::with_id("qqfarm-quit", quit_label, true, None);
    let menu = Menu::with_items(&[&show, &separator, &quit]).ok()?;
    Some(Box::new(menu))
}

fn build_tray(language: &str) -> Option<TrayIcon> {
    let mut builder = TrayIconBuilder::new()
        .with_tooltip(tray_tooltip(language))
        .with_menu(tray_menu(language)?)
        .with_icon(farm_icon()?);
    #[cfg(target_os = "macos")]
    {
        builder = builder.with_icon_as_template(true);
    }
    builder.build().ok()
}

fn farm_icon() -> Option<tray_icon::Icon> {
    const SIZE: u32 = 32;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let in_round_rect = (4..28).contains(&x)
                && (4..28).contains(&y)
                && ((8..=23).contains(&x) || (8..=23).contains(&y));
            let stem = (15..18).contains(&x) && (14..25).contains(&y);
            let left_leaf = (8..17).contains(&x) && (10..17).contains(&y) && x + y <= 24;
            let right_leaf = (15..24).contains(&x) && (8..16).contains(&y) && y + 5 <= x;
            let color = if !in_round_rect {
                [0, 0, 0, 0]
            } else if stem {
                [255, 255, 255, 255]
            } else if left_leaf || right_leaf {
                [191, 238, 161, 255]
            } else {
                [48, 112, 245, 255]
            };
            rgba.extend_from_slice(&color);
        }
    }
    tray_icon::Icon::from_rgba(rgba, SIZE, SIZE).ok()
}

#[cfg(test)]
mod tests {
    use super::{tray_menu_labels, tray_tooltip};

    #[test]
    fn tray_menu_labels_follow_the_selected_language() {
        assert_eq!(tray_menu_labels("zh-CN"), ("显示 QQ 农场", "退出"));
        assert_eq!(tray_menu_labels("en-US"), ("Show QQ Farm", "Quit"));
        assert_eq!(tray_tooltip("zh-CN"), "QQ 农场");
        assert_eq!(tray_tooltip("en-US"), "QQ Farm");
    }
}
