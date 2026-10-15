use super::{CompositorClient, KeyboardLayouts, LayoutEvent};
use serde::Deserialize;
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
enum NiriMessage {
    Ok(serde_json::Value),
    Event(NiriEvent),
    Handled,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
enum NiriResponse {
    KeyboardLayouts(NiriKeyboardLayouts),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
enum NiriEvent {
    KeyboardLayoutSwitched {
        idx: usize,
    },
    #[serde(rename_all = "snake_case")]
    KeyboardLayoutsChanged {
        layouts: NiriKeyboardLayouts,
    },
}

#[derive(Deserialize, Debug, Clone)]
struct NiriKeyboardLayouts {
    #[serde(default)]
    names: Vec<String>,
    #[serde(default)]
    current_idx: usize,
}

#[derive(Debug)]
pub struct NiriClient {
    socket_path: String,
}

impl NiriClient {
    #[must_use]
    pub fn new() -> Option<Self> {
        let socket_path = env::var("NIRI_SOCKET")
            .or_else(|_| env::var("NIRI_SOCKET_PATH"))
            .ok()?;

        if std::path::Path::new(&socket_path).exists() {
            Some(Self { socket_path })
        } else {
            tracing::debug!("Niri socket not found at {}", socket_path);
            None
        }
    }

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
        if let Ok(msg) = serde_json::from_str::<NiriMessage>(json) {
            match msg {
                NiriMessage::Ok(v) => {
                    if let Ok(NiriResponse::KeyboardLayouts(l)) =
                        serde_json::from_value::<NiriResponse>(v)
                    {
                        return KeyboardLayouts {
                            names: l.names,
                            current_idx: l.current_idx,
                        };
                    }
                }
                NiriMessage::Event(NiriEvent::KeyboardLayoutsChanged { layouts: l }) => {
                    return KeyboardLayouts {
                        names: l.names,
                        current_idx: l.current_idx,
                    };
                }
                _ => {}
            }
        }

        if let Ok(l) = serde_json::from_str::<NiriKeyboardLayouts>(json) {
            return KeyboardLayouts {
                names: l.names,
                current_idx: l.current_idx,
            };
        }

        KeyboardLayouts::default()
    }

    pub fn subscribe_events(&self) -> anyhow::Result<BufReader<UnixStream>> {
        let mut stream = UnixStream::connect(&self.socket_path)?;

        writeln!(stream, r#""EventStream""#)?;
        stream.flush()?;

        let mut reader = BufReader::new(stream);

        let mut ack = String::new();
        reader.read_line(&mut ack)?;

        match serde_json::from_str::<NiriMessage>(&ack) {
            Ok(NiriMessage::Ok(_)) | Ok(NiriMessage::Handled) => Ok(reader),
            _ => anyhow::bail!("Failed to subscribe to Niri events: {}", ack.trim()),
        }
    }

    #[must_use]
    pub fn parse_event(&self, line: &str) -> Option<LayoutEvent> {
        if let Ok(NiriMessage::Event(event)) = serde_json::from_str::<NiriMessage>(line) {
            match event {
                NiriEvent::KeyboardLayoutSwitched { idx } => {
                    return Some(LayoutEvent::LayoutSwitched {
                        name: String::new(),
                        index: idx,
                    });
                }
                NiriEvent::KeyboardLayoutsChanged { layouts } => {
                    return Some(LayoutEvent::LayoutsChanged {
                        layouts: KeyboardLayouts {
                            names: layouts.names,
                            current_idx: layouts.current_idx,
                        },
                    });
                }
            }
        }
        None
    }
}

impl CompositorClient for NiriClient {
    fn get_keyboard_layouts(&self) -> anyhow::Result<KeyboardLayouts> {
        let response = self.send_request(r#""KeyboardLayouts""#)?;
        Ok(self.parse_layouts_response(&response))
    }

    fn is_available(&self) -> bool {
        std::path::Path::new(&self.socket_path).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_client() -> NiriClient {
        NiriClient {
            socket_path: String::new(),
        }
    }

    #[test]
    fn test_parse_layouts_response() {
        let client = create_test_client();

        let json = r#"{"Ok":{"KeyboardLayouts":{"names":["English (US)","German","French"],"current_idx":1}}}"#;

        let layouts = client.parse_layouts_response(json);
        assert_eq!(layouts.names.len(), 3);
        assert_eq!(layouts.names[0], "English (US)");
        assert_eq!(layouts.names[1], "German");
        assert_eq!(layouts.names[2], "French");
        assert_eq!(layouts.current_idx, 1);
        assert_eq!(layouts.current_name(), Some("German"));
    }

    #[test]
    fn test_parse_layouts_response_single() {
        let client = create_test_client();

        let json = r#"{"Ok":{"KeyboardLayouts":{"names":["English (US)"],"current_idx":0}}}"#;

        let layouts = client.parse_layouts_response(json);
        assert_eq!(layouts.names.len(), 1);
        assert_eq!(layouts.current_idx, 0);
    }

    #[test]
    fn test_extract_names_array() {
        let client = create_test_client();

        let json = r#"{"names":["English","Deutsch","Francais"]}"#;
        let layouts = client.parse_layouts_response(json);

        assert_eq!(layouts.names.len(), 3);
        assert_eq!(layouts.names[0], "English");
        assert_eq!(layouts.names[1], "Deutsch");
        assert_eq!(layouts.names[2], "Francais");
    }

    #[test]
    fn test_extract_names_with_special_chars() {
        let client = create_test_client();

        let json = r#"{"names":["English (US)","German (Qwertz)"]}"#;
        let layouts = client.parse_layouts_response(json);

        assert_eq!(layouts.names.len(), 2);
        assert_eq!(layouts.names[0], "English (US)");
        assert_eq!(layouts.names[1], "German (Qwertz)");
    }

    #[test]
    fn test_extract_current_idx() {
        let client = create_test_client();

        let json = r#"{"current_idx":2}"#;
        let layouts = client.parse_layouts_response(json);
        assert_eq!(layouts.current_idx, 2);
    }

    #[test]
    fn test_parse_event_layout_switched() {
        let client = create_test_client();

        let line = r#"{"Event":{"KeyboardLayoutSwitched":{"idx":1}}}"#;
        let event = client.parse_event(line);

        assert!(event.is_some());
        if let Some(LayoutEvent::LayoutSwitched { index, .. }) = event {
            assert_eq!(index, 1);
        } else {
            panic!("Expected LayoutSwitched event");
        }
    }

    #[test]
    fn test_parse_event_unrelated() {
        let client = create_test_client();

        let line = r#"{"Event":{"WindowFocused":{"id":123}}}"#;
        let event = client.parse_event(line);

        assert!(event.is_none());
    }
}
