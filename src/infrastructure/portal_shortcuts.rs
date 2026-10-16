use crate::domain::Command;
use async_channel::{Receiver, Sender};
use dbus::arg::{AppendAll, RefArg, Variant};
use dbus::blocking::{BlockingSender, Connection};
use dbus::message::MatchRule;
use dbus::Path as DbusPath;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tracing::info;

const PORTAL_DEST: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const GS_IFACE: &str = "org.freedesktop.portal.GlobalShortcuts";
const REQUEST_IFACE: &str = "org.freedesktop.portal.Request";

/// D-Bus type `a{sv}` used for the portal's option dictionaries.
type Options = HashMap<String, Variant<Box<dyn RefArg>>>;

#[derive(Debug, Clone)]
pub struct Shortcut {
    pub id: &'static str,
    pub description: &'static str,
    pub trigger: Option<String>,
}

/// Keeps the portal listener thread alive for as long as it is held.
pub struct PortalHandle {
    stop: Arc<AtomicBool>,
    _thread: JoinHandle<()>,
}

impl Drop for PortalHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Returns immediately. `ready` yields once: `Ok(())` when the shortcuts were
/// bound, `Err` when the portal is not usable. `commands` yields activations.
pub fn start(
    shortcuts: Vec<Shortcut>,
) -> (
    PortalHandle,
    Receiver<Result<(), String>>,
    Receiver<Command>,
) {
    let (ready_tx, ready_rx) = async_channel::bounded(1);
    let (cmd_tx, cmd_rx) = async_channel::unbounded();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);

    let thread =
        thread::spawn(
            move || match portal_thread(shortcuts, cmd_tx, &ready_tx, &stop_thread) {
                Ok(()) => {}
                Err(e) => {
                    // Ignored if setup already sent `Ok` (bounded channel is full).
                    let _ = ready_tx.try_send(Err(e));
                }
            },
        );

    (
        PortalHandle {
            stop,
            _thread: thread,
        },
        ready_rx,
        cmd_rx,
    )
}

fn portal_thread(
    shortcuts: Vec<Shortcut>,
    cmd_tx: Sender<Command>,
    ready_tx: &Sender<Result<(), String>>,
    stop: &AtomicBool,
) -> Result<(), String> {
    let conn = Connection::new_session().map_err(|e| format!("session bus unavailable: {e}"))?;

    let unique = conn.unique_name().to_string();
    let sender = unique.trim_start_matches(':').replace('.', "_");
    let pid = std::process::id();

    let session_token = format!("hibiki_session_{pid}");
    let session_path_str =
        format!("/org/freedesktop/portal/desktop/session/{sender}/{session_token}");
    let fallback_session = DbusPath::new(session_path_str).map_err(|e| e.to_string())?;

    let create_token = format!("hibiki_create_{pid}");
    let create_request = format!("/org/freedesktop/portal/desktop/request/{sender}/{create_token}");

    let mut create_opts: Options = Options::new();
    create_opts.insert("handle_token".into(), Variant(Box::new(create_token)));
    create_opts.insert(
        "session_handle_token".into(),
        Variant(Box::new(session_token)),
    );

    let results = call_request(
        &conn,
        GS_IFACE,
        "CreateSession",
        (create_opts,),
        &create_request,
        Duration::from_secs(20),
    )?;

    let session_handle = results
        .get("session_handle")
        .and_then(|value| value.0.as_str())
        .and_then(|path| DbusPath::new(path.to_string()).ok())
        .unwrap_or(fallback_session);

    let bind_token = format!("hibiki_bind_{pid}");
    let bind_request = format!("/org/freedesktop/portal/desktop/request/{sender}/{bind_token}");

    let mut bind_opts: Options = Options::new();
    bind_opts.insert("handle_token".into(), Variant(Box::new(bind_token)));

    let mut list: Vec<(String, Options)> = Vec::new();
    for shortcut in &shortcuts {
        let mut opts: Options = Options::new();
        opts.insert(
            "description".into(),
            Variant(Box::new(shortcut.description.to_string())),
        );
        if let Some(trigger) = &shortcut.trigger {
            opts.insert(
                "preferred_trigger".into(),
                Variant(Box::new(trigger.clone())),
            );
        }
        list.push((shortcut.id.to_string(), opts));
    }

    call_request(
        &conn,
        GS_IFACE,
        "BindShortcuts",
        (session_handle, list, String::new(), bind_opts),
        &bind_request,
        Duration::from_secs(60),
    )?;

    let activated_rule = MatchRule::new_signal(GS_IFACE, "Activated")
        .with_path(DbusPath::new(PORTAL_PATH).map_err(|e| e.to_string())?);
    let cmd_tx_c = cmd_tx.clone();
    conn.add_match(
        activated_rule,
        move |args: (DbusPath, String, u64, Options), _: &Connection, _: &dbus::Message| {
            if let Some(command) = command_for(&args.1) {
                let _ = cmd_tx_c.try_send(command);
            }
            true
        },
    )
    .map_err(|e| format!("could not subscribe to activations: {e}"))?;

    let _ = ready_tx.send_blocking(Ok(()));
    info!("Global shortcuts are now managed by the XDG desktop portal");

    while !stop.load(Ordering::SeqCst) {
        let _ = conn.process(Duration::from_millis(250));
    }

    Ok(())
}

fn command_for(id: &str) -> Option<Command> {
    Some(match id {
        "keystroke-mode" => Command::ActivateKeystrokeMode,
        "bubble-mode" => Command::ActivateBubbleMode,
        "toggle-mode" => Command::ToggleMode,
        "pause" => Command::TogglePause,
        "toggle-focus" => Command::ToggleFocus,
        _ => return None,
    })
}

/// Send a portal request and block until its `Response` signal arrives.
fn call_request<A: AppendAll>(
    conn: &Connection,
    iface: &str,
    method: &str,
    args: A,
    request_path: &str,
    timeout: Duration,
) -> Result<Options, String> {
    let (tx, rx) = mpsc::channel::<(u32, Options)>();

    // Registering before the call avoids missing a fast response.
    let path = DbusPath::new(request_path.to_string()).map_err(|e| e.to_string())?;
    let rule = MatchRule::new_signal(REQUEST_IFACE, "Response").with_path(path);
    conn.add_match(
        rule,
        move |response: (u32, Options), _: &Connection, _: &dbus::Message| {
            let _ = tx.send(response);
            false
        },
    )
    .map_err(|e| format!("could not subscribe to portal response: {e}"))?;

    let mut message = dbus::Message::new_method_call(PORTAL_DEST, PORTAL_PATH, iface, method)
        .map_err(|e| format!("could not build {method} call: {e}"))?;
    message.append_all(args);

    // Wait for the method reply so a missing portal fails fast instead of
    // waiting for a response signal that never arrives.
    conn.send_with_reply_and_block(message, Duration::from_secs(10))
        .map_err(|e| format!("{method} call failed: {e}"))?;

    let deadline = Instant::now() + timeout;
    loop {
        match rx.try_recv() {
            Ok((code, results)) => {
                return match code {
                    0 => Ok(results),
                    1 => Err(format!("{method} cancelled by user")),
                    other => Err(format!("{method} failed (response code {other})")),
                };
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err(format!("{method} response channel closed"));
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }

        if Instant::now() >= deadline {
            return Err(format!("{method} timed out"));
        }

        let _ = conn.process(Duration::from_millis(50));
    }
}

/// Convert a GTK accelerator (`<Shift><Control>b`) to a shortcuts-spec trigger
/// (`SHIFT+CTRL+b`). `None` when empty or unparseable.
#[must_use]
pub fn accelerator_to_trigger(accelerator: &str) -> Option<String> {
    let accelerator = accelerator.trim();
    if accelerator.is_empty() {
        return None;
    }

    let mut rest = accelerator;
    let mut parts: Vec<String> = Vec::new();

    while let Some(stripped) = rest.strip_prefix('<') {
        let end = stripped.find('>')?;
        let modifier = match stripped[..end].to_ascii_lowercase().as_str() {
            "control" | "ctrl" => "CTRL",
            "shift" => "SHIFT",
            "alt" => "ALT",
            "super" | "meta" | "logo" => "LOGO",
            _ => return None,
        };
        if !parts.iter().any(|p| p == modifier) {
            parts.push(modifier.to_string());
        }
        rest = &stripped[end + 1..];
    }

    if rest.is_empty() {
        return None;
    }

    parts.push(normalize_keysym(rest)?);
    Some(parts.join("+"))
}

fn normalize_keysym(key: &str) -> Option<String> {
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c.is_ascii_alphanumeric() {
            return Some(c.to_ascii_lowercase().to_string());
        }
        return match c {
            ' ' => Some("space".to_string()),
            '-' => Some("minus".to_string()),
            '=' => Some("equal".to_string()),
            ',' => Some("comma".to_string()),
            '.' => Some("period".to_string()),
            '/' => Some("slash".to_string()),
            '\\' => Some("backslash".to_string()),
            ';' => Some("semicolon".to_string()),
            '\'' => Some("apostrophe".to_string()),
            '`' => Some("grave".to_string()),
            '[' => Some("bracketleft".to_string()),
            ']' => Some("bracketright".to_string()),
            _ => None,
        };
    }

    let lower = key.to_ascii_lowercase();
    let named = match lower.as_str() {
        "space" => "space",
        "return" | "enter" => "Return",
        "escape" | "esc" => "Escape",
        "backspace" => "Backspace",
        "delete" => "Delete",
        "tab" => "Tab",
        "insert" => "Insert",
        "home" => "Home",
        "end" => "End",
        "left" => "Left",
        "right" => "Right",
        "up" => "Up",
        "down" => "Down",
        "page_up" | "pageup" => "Prior",
        "page_down" | "pagedown" => "Next",
        "caps_lock" | "capslock" => "Caps_Lock",
        "print" => "Print",
        "pause" => "Pause",
        _ => {
            if lower.starts_with('f') && lower[1..].chars().all(|c| c.is_ascii_digit()) {
                return Some(key.to_ascii_uppercase());
            }
            return None;
        }
    };
    Some(named.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_simple_accelerators() {
        assert_eq!(
            accelerator_to_trigger("<Shift><Control>b").as_deref(),
            Some("SHIFT+CTRL+b")
        );
        assert_eq!(
            accelerator_to_trigger("<Control>p").as_deref(),
            Some("CTRL+p")
        );
        assert_eq!(
            accelerator_to_trigger("<Control><Alt>Delete").as_deref(),
            Some("CTRL+ALT+Delete")
        );
    }

    #[test]
    fn converts_special_keys() {
        assert_eq!(
            accelerator_to_trigger("<Super>Return").as_deref(),
            Some("LOGO+Return")
        );
        assert_eq!(
            accelerator_to_trigger("<Shift>F5").as_deref(),
            Some("SHIFT+F5")
        );
        assert_eq!(
            accelerator_to_trigger("<Control>space").as_deref(),
            Some("CTRL+space")
        );
    }

    #[test]
    fn rejects_empty_and_unknown() {
        assert_eq!(accelerator_to_trigger(""), None);
        assert_eq!(accelerator_to_trigger("   "), None);
        assert_eq!(accelerator_to_trigger("<Shift>"), None);
        assert_eq!(accelerator_to_trigger("<Hyper>k"), None);
        assert_eq!(accelerator_to_trigger("<Shift>mystery"), None);
    }

    #[test]
    fn maps_ids_to_commands() {
        assert_eq!(command_for("toggle-mode"), Some(Command::ToggleMode));
        assert_eq!(command_for("pause"), Some(Command::TogglePause));
        assert_eq!(command_for("nope"), None);
    }
}
