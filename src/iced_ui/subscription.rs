//! Event streams: indexing progress, window and system events, the global
//! wake hotkey, the tray, and keyboard shortcuts.
//!
//! Grouped separately from dispatch because these are the *inputs* to the app,
//! and they have a different failure mode: a subscription that changes identity
//! every frame causes Iced to tear down and rebuild the underlying stream
//! continuously. Keeping the identity types next to the code that builds them is
//! what makes that visible.

use super::Message;
use super::hotkey::{SystemSubscriptionData, parse_hotkey};
use super::state::{App, SubscriptionData};
use iced::Subscription;
use iced::futures::SinkExt;

/// Every stream the application listens to, batched into one subscription.
///
/// Long because each stream is configured independently and none of them share
/// state; splitting them into helpers would only move the same setup into five
/// functions that still have to be called from here.
#[allow(clippy::too_many_lines)]
pub fn subscription(app: &App) -> Subscription<Message> {
    let progress_sub = app
        .progress_rx
        .as_ref()
        .map_or_else(Subscription::none, |rx| {
            Subscription::run_with(SubscriptionData { rx: rx.clone() }, |data| {
                let rx = data.rx.clone();
                iced::stream::channel(
                    100,
                    move |mut output: iced::futures::channel::mpsc::Sender<Message>| {
                        let rx = rx.clone();
                        async move {
                            while let Ok(event) = rx.recv_async().await {
                                let _ = output.send(Message::PollProgressResult(Some(event))).await;
                            }
                        }
                    },
                )
            })
        });

    let event_sub = iced::window::events().map(|(id, event)| match event {
        iced::window::Event::Unfocused => Message::WindowUnfocused(id),
        iced::window::Event::Opened { .. } | iced::window::Event::Focused => {
            Message::WindowIdCaptured(id)
        }
        // Intercept the window close so `minimize_to_tray` actually means
        // something. Previously there was no close handler at all, so the
        // checkbox labelled "minimize to system tray on window close" did
        // nothing except decide whether a tray icon was created at startup.
        iced::window::Event::CloseRequested => Message::WindowCloseRequested(id),
        _ => Message::NoOp,
    });

    let hotkey_str = app.settings.global_hotkey.clone();
    let minimize_to_tray = app.settings.minimize_to_tray;
    let system_sub = Subscription::run_with(
        SystemSubscriptionData {
            hotkey_str,
            minimize_to_tray,
        },
        |data| {
            let hotkey_str = data.hotkey_str.clone();
            iced::stream::channel(
                10,
                move |mut output: iced::futures::channel::mpsc::Sender<Message>| {
                    let hotkey_str = hotkey_str.clone();
                    async move {
                        let (tx, mut rx) = tokio::sync::mpsc::channel(10);

                        // The poller exits on shutdown and drops its sender, which
                        // closes `rx` and ends this stream. Previously it looped
                        // forever with no exit condition: changing the hotkey in
                        // Settings spawned a fresh thread each time and the old
                        // ones kept burning a core and re-registering the hotkey.
                        let shutdown = std::thread::spawn(move || {
                            let manager = global_hotkey::GlobalHotKeyManager::new().ok();
                            let registered_hotkey = manager.as_ref().and_then(|m| {
                                let hk = parse_hotkey(&hotkey_str)?;
                                m.register(hk).is_ok().then_some(hk)
                            });

                            loop {
                                if crate::is_shutting_down() {
                                    break;
                                }

                                if let Some(hk) = registered_hotkey
                                    && let Ok(event) =
                                        global_hotkey::GlobalHotKeyEvent::receiver().try_recv()
                                    && event.id == hk.id()
                                    && event.state == global_hotkey::HotKeyState::Released
                                    && tx.blocking_send(Message::ToggleWindow).is_err()
                                {
                                    break;
                                }

                                if let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv()
                                {
                                    let msg = match event.id.0.as_str() {
                                        "show" => Some(Message::RestoreWindow),
                                        "quit" => Some(Message::Quit),
                                        _ => None,
                                    };
                                    if let Some(msg) = msg
                                        && tx.blocking_send(msg).is_err()
                                    {
                                        break;
                                    }
                                }

                                if let Ok(tray_icon::TrayIconEvent::Click {
                                    button: tray_icon::MouseButton::Left,
                                    ..
                                }) = tray_icon::TrayIconEvent::receiver().try_recv()
                                    && tx.blocking_send(Message::ToggleWindow).is_err()
                                {
                                    break;
                                }

                                std::thread::sleep(std::time::Duration::from_millis(50));
                            }

                            // Unregister so the hotkey is released immediately
                            // rather than lingering until process exit.
                            if let (Some(manager), Some(hk)) = (manager, registered_hotkey) {
                                let _ = manager.unregister(hk);
                            }
                        });

                        while let Some(msg) = rx.recv().await {
                            let _ = output.send(msg).await;
                        }

                        let _ = shutdown.join();
                    }
                },
            )
        },
    );

    let keyboard_sub = iced::event::listen().map(|event| match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
            match key {
                iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp) => {
                    Message::SelectPreviousResult
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown) => {
                    Message::SelectNextResult
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => {
                    if modifiers.control() {
                        Message::ShowSelectedInFolder
                    } else {
                        Message::OpenSelectedResult
                    }
                }
                iced::keyboard::Key::Character(ref c)
                    if c.eq_ignore_ascii_case("c") && modifiers.control() =>
                {
                    Message::CopySelectedPath
                }
                // Advertised on the welcome screen but never bound.
                iced::keyboard::Key::Character(ref c)
                    if c.eq_ignore_ascii_case("f") && modifiers.control() =>
                {
                    Message::FocusSearch
                }
                // Dismisses the right-click menu. Without this a menu with no
                // clickable dismissal trap traps keyboard users.
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => Message::Escape,
                _ => Message::NoOp,
            }
        }
        _ => Message::NoOp,
    });

    Subscription::batch(vec![progress_sub, event_sub, system_sub, keyboard_sub])
}
