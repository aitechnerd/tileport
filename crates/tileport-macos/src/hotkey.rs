//! Global hotkey interception via CGEventTap on a dedicated thread.
//!
//! Creates a CGEventTap listening for kCGEventKeyDown, matches against
//! configured keybindings, and sends matched commands via crossbeam channel.
//!
//! CRITICAL (DevSecOps): The callback uses try_send, NEVER blocking send.
//! A blocked CGEventTap callback causes macOS to auto-disable the tap after
//! ~1 second, silently stopping all hotkey interception.

use core_graphics::event::{
    CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CallbackResult, EventField,
};
use crossbeam_channel::Sender;
use std::thread::{self, JoinHandle};
use tileport_core::command::Command;
use tileport_core::config::{Key, Keybinding, Modifier};

/// Virtual key code to string key mapping for matching against keybindings.
fn keycode_to_key_string(keycode: u16) -> Option<&'static str> {
    // macOS virtual key codes (ANSI layout)
    match keycode {
        0x26 => Some("j"),  // KeyCode::ANSI_J
        0x28 => Some("k"),  // KeyCode::ANSI_K
        0x04 => Some("h"),  // KeyCode::ANSI_H
        0x25 => Some("l"),  // KeyCode::ANSI_L
        0x03 => Some("f"),  // KeyCode::ANSI_F
        0x12 => Some("1"),  // KeyCode::ANSI_1
        0x13 => Some("2"),  // KeyCode::ANSI_2
        0x14 => Some("3"),  // KeyCode::ANSI_3
        0x15 => Some("4"),  // KeyCode::ANSI_4
        0x17 => Some("5"),  // KeyCode::ANSI_5
        0x16 => Some("6"),  // KeyCode::ANSI_6
        0x1A => Some("7"),  // KeyCode::ANSI_7
        0x1C => Some("8"),  // KeyCode::ANSI_8
        0x19 => Some("9"),  // KeyCode::ANSI_9
        0x24 => Some("enter"), // KeyCode::Return
        _ => None,
    }
}

/// Check if the event's modifier flags match the expected modifiers.
///
/// We check only for Alt and Shift since those are the only modifiers
/// used in the default keybindings.
fn modifiers_match(flags: CGEventFlags, expected: &[Modifier]) -> bool {
    let has_alt = flags.contains(CGEventFlags::CGEventFlagAlternate);
    let has_shift = flags.contains(CGEventFlags::CGEventFlagShift);
    let has_ctrl = flags.contains(CGEventFlags::CGEventFlagControl);
    let has_cmd = flags.contains(CGEventFlags::CGEventFlagCommand);

    let expects_alt = expected.contains(&Modifier::Alt);
    let expects_shift = expected.contains(&Modifier::Shift);
    let expects_ctrl = expected.contains(&Modifier::Ctrl);
    let expects_cmd = expected.contains(&Modifier::Cmd);

    has_alt == expects_alt
        && has_shift == expects_shift
        && has_ctrl == expects_ctrl
        && has_cmd == expects_cmd
}

/// Try to match a key event against the keybinding table.
///
/// Returns the matching Command if found.
fn match_keybinding(
    keycode: u16,
    flags: CGEventFlags,
    bindings: &[Keybinding],
) -> Option<Command> {
    let key_str = keycode_to_key_string(keycode)?;

    for binding in bindings {
        let binding_key = match &binding.key {
            Key::Char(s) => s.as_str(),
        };
        if binding_key == key_str && modifiers_match(flags, &binding.modifiers) {
            return Some(binding.command.clone());
        }
    }

    None
}

/// Start the hotkey listener thread.
///
/// Creates a CGEventTap intercepting key-down events and runs a CFRunLoop
/// on the thread to receive events. Matched hotkeys send the corresponding
/// Command through `cmd_tx` using try_send (never blocking).
///
/// Returns the thread's JoinHandle.
pub fn start_hotkey_thread(
    bindings: Vec<Keybinding>,
    cmd_tx: Sender<Command>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("tileport-hotkey".into())
        .spawn(move || {
            tracing::info!("hotkey thread starting");

            let result = CGEventTap::with_enabled(
                CGEventTapLocation::Session,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::Default,
                vec![CGEventType::KeyDown],
                move |_proxy, _event_type, event| {
                    let keycode =
                        event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE)
                            as u16;
                    let flags = event.get_flags();

                    if let Some(command) = match_keybinding(keycode, flags, &bindings) {
                        tracing::debug!(?command, keycode, "hotkey matched");

                        // CRITICAL (DevSecOps): Use try_send, NEVER blocking send.
                        // If the channel is full, discard the event rather than
                        // blocking. A blocked callback causes macOS to auto-disable
                        // the tap after ~1 second.
                        if let Err(e) = cmd_tx.try_send(command) {
                            tracing::warn!(
                                "hotkey command channel full or disconnected, \
                                 discarding event: {}",
                                e
                            );
                        }

                        // Suppress the event (don't pass to other apps).
                        CallbackResult::Drop
                    } else {
                        // Not our hotkey; pass through to the system.
                        CallbackResult::Keep
                    }
                },
                || {
                    tracing::info!("hotkey CFRunLoop running");
                    core_foundation::runloop::CFRunLoop::run_current();
                },
            );

            match result {
                Ok(()) => tracing::info!("hotkey thread exiting normally"),
                Err(()) => tracing::error!(
                    "failed to create CGEventTap -- \
                     check Input Monitoring permission"
                ),
            }
        })
        .expect("failed to spawn hotkey thread")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keycode_to_key_string() {
        assert_eq!(keycode_to_key_string(0x26), Some("j"));
        assert_eq!(keycode_to_key_string(0x28), Some("k"));
        assert_eq!(keycode_to_key_string(0x03), Some("f"));
        assert_eq!(keycode_to_key_string(0x12), Some("1"));
        assert_eq!(keycode_to_key_string(0x19), Some("9"));
        assert_eq!(keycode_to_key_string(0x24), Some("enter"));
        assert_eq!(keycode_to_key_string(0xFF), None);
    }

    #[test]
    fn test_modifiers_match_alt_only() {
        let flags = CGEventFlags::CGEventFlagAlternate;
        assert!(modifiers_match(flags, &[Modifier::Alt]));
        assert!(!modifiers_match(flags, &[Modifier::Alt, Modifier::Shift]));
        assert!(!modifiers_match(flags, &[Modifier::Shift]));
        assert!(!modifiers_match(flags, &[]));
    }

    #[test]
    fn test_modifiers_match_alt_shift() {
        let flags = CGEventFlags::CGEventFlagAlternate | CGEventFlags::CGEventFlagShift;
        assert!(modifiers_match(flags, &[Modifier::Alt, Modifier::Shift]));
        assert!(!modifiers_match(flags, &[Modifier::Alt]));
    }

    #[test]
    fn test_match_keybinding_focus_next() {
        let bindings = vec![Keybinding {
            modifiers: vec![Modifier::Alt],
            key: Key::Char("j".into()),
            command: Command::FocusNext,
        }];

        let flags = CGEventFlags::CGEventFlagAlternate;
        let result = match_keybinding(0x26, flags, &bindings);
        assert_eq!(result, Some(Command::FocusNext));
    }

    #[test]
    fn test_match_keybinding_no_match() {
        let bindings = vec![Keybinding {
            modifiers: vec![Modifier::Alt],
            key: Key::Char("j".into()),
            command: Command::FocusNext,
        }];

        // Wrong key
        let flags = CGEventFlags::CGEventFlagAlternate;
        assert_eq!(match_keybinding(0x28, flags, &bindings), None);

        // Wrong modifier
        let flags = CGEventFlags::CGEventFlagShift;
        assert_eq!(match_keybinding(0x26, flags, &bindings), None);
    }

    #[test]
    fn test_match_keybinding_workspace_switch() {
        let bindings = vec![Keybinding {
            modifiers: vec![Modifier::Alt],
            key: Key::Char("3".into()),
            command: Command::SwitchWorkspace { workspace: 3 },
        }];

        let flags = CGEventFlags::CGEventFlagAlternate;
        let result = match_keybinding(0x14, flags, &bindings);
        assert_eq!(result, Some(Command::SwitchWorkspace { workspace: 3 }));
    }

    #[test]
    fn test_match_keybinding_move_to_workspace() {
        let bindings = vec![Keybinding {
            modifiers: vec![Modifier::Alt, Modifier::Shift],
            key: Key::Char("5".into()),
            command: Command::MoveToWorkspace { workspace: 5 },
        }];

        let flags = CGEventFlags::CGEventFlagAlternate | CGEventFlags::CGEventFlagShift;
        let result = match_keybinding(0x17, flags, &bindings);
        assert_eq!(result, Some(Command::MoveToWorkspace { workspace: 5 }));
    }
}
