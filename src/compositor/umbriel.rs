use super::{CompositorClient, KeyboardLayouts, LayoutEvent};
use serde::Deserialize;
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// Reply to the `keyboard-layouts` query: `{"ok":{"names":[...],"current_index":N}}`.
#[derive(Deserialize, Debug)]
struct UmbrielReply {
    ok: Option<UmbrielLayouts>,
}

/// Payload shared by the query reply and the `keyboard_layout` event.
#[derive(Deserialize, Debug, Clone)]
struct UmbrielLayouts {
    #[serde(default)]
    names: Vec<String>,
    #[serde(default)]
    current_index: usize,
}

impl UmbrielLayouts {
    fn into_keyboard_layouts(self) -> KeyboardLayouts {
        KeyboardLayouts {
            names: self.names,
            current_idx: self.current_index,
        }
    }
}

/// Event line: `{"event":"keyboard_layout","data":{"names":[...],"current_index":N}}`.
#[derive(Deserialize, Debug)]
struct UmbrielEvent {
    event: String,
    data: Option<UmbrielLayouts>,
}

#[derive(Debug)]
pub struct UmbrielClient {
    socket_path: String,
}

impl UmbrielClient {
    /// Resolve the compositor socket the same way the `umbriel` CLI does:
    /// `UMBRIEL_SOCKET`, or `$XDG_RUNTIME_DIR/umbriel-$WAYLAND_DISPLAY.sock`.
    fn resolve_socket_path() -> Option<String> {
        if let Ok(path) = env::var("UMBRIEL_SOCKET") {
            if !path.is_empty() {
                return Some(path);
            }
        }

        let runtime_dir = env::var("XDG_RUNTIME_DIR").ok()?;
        let display = env::var("WAYLAND_DISPLAY").ok()?;

        if runtime_dir.is_empty() || display.is_empty() {
            return None;
        }

        Some(format!("{}/umbriel-{}.sock", runtime_dir, display))
    }

    #[must_use]
    pub fn new() -> Option<Self> {
        let socket_path = Self::resolve_socket_path()?;

        if std::path::Path::new(&socket_path).exists() {
            Some(Self { socket_path })
        } else {
            tracing::debug!("Umbriel socket not found at {}", socket_path);
            None
        }
    }

    /// Send a one-shot JSON request (one object per line) and read a single reply line.
    fn send_request(&self, request: &str) -> anyhow::Result<String> {
        let mut stream = UnixStream::connect(&self.socket_path)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;

        writeln!(stream, "{}", request)?;
        stream.flush()?;

        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader.read_line(&mut response)?;

        Ok(response)
    }

    fn parse_layouts_response(&self, json: &str) -> KeyboardLayouts {
        if let Ok(reply) = serde_json::from_str::<UmbrielReply>(json) {
            if let Some(ok) = reply.ok {
                return ok.into_keyboard_layouts();
            }
        }

        KeyboardLayouts::default()
    }

    /// Subscribe to the `keyboard_layout` event family. The stream first delivers
    /// the current state, then a fresh snapshot whenever the active layout changes.
    pub fn subscribe_events(&self) -> anyhow::Result<BufReader<UnixStream>> {
        let mut stream = UnixStream::connect(&self.socket_path)?;

        writeln!(
            stream,
            r#"{{"cmd":"subscribe","events":["keyboard_layout"]}}"#
        )?;
        stream.flush()?;

        Ok(BufReader::new(stream))
    }

    #[must_use]
    pub fn parse_event(&self, line: &str) -> Option<LayoutEvent> {
        let event: UmbrielEvent = serde_json::from_str(line).ok()?;

        if event.event != "keyboard_layout" {
            return None;
        }

        let layouts = event.data?.into_keyboard_layouts();

        Some(LayoutEvent::LayoutsChanged { layouts })
    }
}

impl CompositorClient for UmbrielClient {
    fn get_keyboard_layouts(&self) -> anyhow::Result<KeyboardLayouts> {
        let response = self.send_request(r#"{"cmd":"keyboard-layouts"}"#)?;
        Ok(self.parse_layouts_response(&response))
    }

    fn is_available(&self) -> bool {
        std::path::Path::new(&self.socket_path).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_client() -> UmbrielClient {
        UmbrielClient {
            socket_path: String::new(),
        }
    }

    #[test]
    fn test_parse_layouts_response() {
        let client = create_test_client();

        let json = r#"{"ok":{"names":["us","de","fr"],"current_index":1}}"#;

        let layouts = client.parse_layouts_response(json);
        assert_eq!(layouts.names.len(), 3);
        assert_eq!(layouts.names[0], "us");
        assert_eq!(layouts.names[1], "de");
        assert_eq!(layouts.names[2], "fr");
        assert_eq!(layouts.current_idx, 1);
        assert_eq!(layouts.current_name(), Some("de"));
    }

    #[test]
    fn test_parse_layouts_response_no_keyboard() {
        let client = create_test_client();

        let json = r#"{"err":"no keyboard"}"#;

        let layouts = client.parse_layouts_response(json);
        assert!(layouts.is_empty());
        assert_eq!(layouts.current_idx, 0);
    }

    #[test]
    fn test_parse_layouts_response_single() {
        let client = create_test_client();

        let json = r#"{"ok":{"names":["us"],"current_index":0}}"#;

        let layouts = client.parse_layouts_response(json);
        assert_eq!(layouts.names.len(), 1);
        assert_eq!(layouts.names[0], "us");
        assert_eq!(layouts.current_idx, 0);
    }

    #[test]
    fn test_parse_event_layouts_changed() {
        let client = create_test_client();

        let line = r#"{"event":"keyboard_layout","data":{"names":["us","de"],"current_index":1}}"#;
        let event = client.parse_event(line);

        assert!(event.is_some());
        if let Some(LayoutEvent::LayoutsChanged { layouts }) = event {
            assert_eq!(layouts.names, vec!["us".to_string(), "de".to_string()]);
            assert_eq!(layouts.current_idx, 1);
            assert_eq!(layouts.current_name(), Some("de"));
        } else {
            panic!("Expected LayoutsChanged event");
        }
    }

    #[test]
    fn test_parse_event_unrelated() {
        let client = create_test_client();

        let line = r#"{"event":"workspaces","data":[]}"#;
        assert!(client.parse_event(line).is_none());
    }

    #[test]
    fn test_parse_event_missing_data() {
        let client = create_test_client();

        let line = r#"{"event":"keyboard_layout"}"#;
        assert!(client.parse_event(line).is_none());
    }
}
