use super::{CompositorClient, KeyboardLayouts};
use std::env;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

const IPC_MAGIC: &[u8; 6] = b"i3-ipc";

const IPC_HEADER_SIZE: usize = 14;

const IPC_GET_INPUTS: u32 = 100;

#[allow(dead_code)]
const IPC_SUBSCRIBE: u32 = 2;

pub const MAX_IPC_PAYLOAD_SIZE: u32 = 10 * 1024 * 1024;

#[derive(Debug)]
pub struct SwayClient {
    socket_path: String,
}

impl SwayClient {
    fn validate_payload_len(len: u32) -> anyhow::Result<usize> {
        if len > MAX_IPC_PAYLOAD_SIZE {
            anyhow::bail!(
                "Sway IPC payload too large: {} bytes (max: {} bytes)",
                len,
                MAX_IPC_PAYLOAD_SIZE
            );
        }
        Ok(len as usize)
    }

    #[must_use]
    pub fn new() -> Option<Self> {
        let socket_path = env::var("SWAYSOCK").ok()?;

        if std::path::Path::new(&socket_path).exists() {
            Some(Self { socket_path })
        } else {
            tracing::debug!("Sway socket not found at {}", socket_path);
            None
        }
    }

    fn build_header(payload_len: u32, message_type: u32) -> [u8; IPC_HEADER_SIZE] {
        let mut header = [0u8; IPC_HEADER_SIZE];

        header[0..6].copy_from_slice(IPC_MAGIC);

        header[6..10].copy_from_slice(&payload_len.to_le_bytes());

        header[10..14].copy_from_slice(&message_type.to_le_bytes());

        header
    }

    fn send_message(&self, message_type: u32, payload: &[u8]) -> anyhow::Result<String> {
        let mut stream = UnixStream::connect(&self.socket_path)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;

        let header = Self::build_header(payload.len() as u32, message_type);
        stream.write_all(&header)?;

        if !payload.is_empty() {
            stream.write_all(payload)?;
        }

        let mut resp_header = [0u8; IPC_HEADER_SIZE];
        stream.read_exact(&mut resp_header)?;

        if &resp_header[0..6] != IPC_MAGIC {
            anyhow::bail!("Invalid i3-IPC response: magic mismatch");
        }

        let payload_len =
            Self::validate_payload_len(u32::from_le_bytes(resp_header[6..10].try_into()?))?;

        let mut payload = vec![0u8; payload_len];
        stream.read_exact(&mut payload)?;

        String::from_utf8(payload).map_err(Into::into)
    }

    fn parse_inputs_response(&self, json: &str) -> KeyboardLayouts {
        let mut layouts = KeyboardLayouts::default();
        let mut seen_layouts = std::collections::HashSet::new();

        if let Some(layouts_array) = self.extract_layout_names_array(json) {
            for name in layouts_array {
                if !name.is_empty() && seen_layouts.insert(name.clone()) {
                    layouts.names.push(name);
                }
            }
        }

        if let Some(idx) = self.extract_active_layout_index(json) {
            layouts.current_idx = idx;
        }

        if layouts.names.is_empty() {
            if let Some(name) = self.extract_active_layout_name(json) {
                layouts.names.push(name);
            }
        }

        layouts
    }

    fn extract_layout_names_array(&self, json: &str) -> Option<Vec<String>> {
        let key = "\"xkb_layout_names\"";
        let key_pos = json.find(key)?;
        let after_key = &json[key_pos + key.len()..];

        let bracket_pos = after_key.find('[')?;
        let array_start = &after_key[bracket_pos + 1..];

        let bracket_end = array_start.find(']')?;
        let array_content = &array_start[..bracket_end];

        let mut names = Vec::new();
        let mut in_string = false;
        let mut current = String::new();

        for ch in array_content.chars() {
            match ch {
                '"' if !in_string => {
                    in_string = true;
                    current.clear();
                }
                '"' if in_string => {
                    in_string = false;
                    if !current.is_empty() {
                        names.push(current.clone());
                    }
                }
                _ if in_string => {
                    current.push(ch);
                }
                _ => {}
            }
        }

        if names.is_empty() {
            None
        } else {
            Some(names)
        }
    }

    fn extract_active_layout_index(&self, json: &str) -> Option<usize> {
        let key = "\"xkb_active_layout_index\"";
        let key_pos = json.find(key)?;
        let after_key = &json[key_pos + key.len()..];

        let colon_pos = after_key.find(':')?;
        let after_colon = &after_key[colon_pos + 1..];

        let num_str: String = after_colon
            .chars()
            .skip_while(|c| c.is_whitespace())
            .take_while(|c| c.is_ascii_digit())
            .collect();

        num_str.parse().ok()
    }

    fn extract_active_layout_name(&self, json: &str) -> Option<String> {
        let key = "\"xkb_active_layout_name\"";
        let key_pos = json.find(key)?;
        let after_key = &json[key_pos + key.len()..];

        let colon_pos = after_key.find(':')?;
        let after_colon = &after_key[colon_pos + 1..];

        let quote_start = after_colon.find('"')?;
        let after_quote = &after_colon[quote_start + 1..];

        let quote_end = after_quote.find('"')?;
        let name = &after_quote[..quote_end];

        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }

    #[allow(dead_code)]
    pub fn subscribe_events(&self) -> anyhow::Result<UnixStream> {
        let mut stream = UnixStream::connect(&self.socket_path)?;

        let payload = br#"["input"]"#;
        let header = Self::build_header(payload.len() as u32, IPC_SUBSCRIBE);

        stream.write_all(&header)?;
        stream.write_all(payload)?;

        let mut resp_header = [0u8; IPC_HEADER_SIZE];
        stream.read_exact(&mut resp_header)?;

        let payload_len =
            Self::validate_payload_len(u32::from_le_bytes(resp_header[6..10].try_into()?))?;
        let mut _response = vec![0u8; payload_len];
        stream.read_exact(&mut _response)?;

        Ok(stream)
    }
}

impl CompositorClient for SwayClient {
    fn get_keyboard_layouts(&self) -> anyhow::Result<KeyboardLayouts> {
        let response = self.send_message(IPC_GET_INPUTS, &[])?;
        Ok(self.parse_inputs_response(&response))
    }

    fn is_available(&self) -> bool {
        std::path::Path::new(&self.socket_path).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_client() -> SwayClient {
        SwayClient {
            socket_path: String::new(),
        }
    }

    #[test]
    fn test_build_header() {
        let header = SwayClient::build_header(0, IPC_GET_INPUTS);

        assert_eq!(&header[0..6], b"i3-ipc");

        assert_eq!(u32::from_le_bytes(header[6..10].try_into().unwrap()), 0);

        assert_eq!(u32::from_le_bytes(header[10..14].try_into().unwrap()), 100);
    }

    #[test]
    fn test_parse_inputs_response() {
        let client = create_test_client();

        let json = r#"[
            {
                "type": "keyboard",
                "xkb_layout_names": ["English (US)", "German", "French"],
                "xkb_active_layout_index": 1,
                "xkb_active_layout_name": "German"
            }
        ]"#;

        let layouts = client.parse_inputs_response(json);
        assert_eq!(layouts.names.len(), 3);
        assert_eq!(layouts.names[0], "English (US)");
        assert_eq!(layouts.names[1], "German");
        assert_eq!(layouts.names[2], "French");
        assert_eq!(layouts.current_idx, 1);
        assert_eq!(layouts.current_name(), Some("German"));
    }

    #[test]
    fn test_parse_inputs_response_no_array() {
        let client = create_test_client();

        let json = r#"[
            {
                "type": "keyboard",
                "xkb_active_layout_name": "English (US)"
            }
        ]"#;

        let layouts = client.parse_inputs_response(json);
        assert_eq!(layouts.names.len(), 1);
        assert_eq!(layouts.names[0], "English (US)");
    }

    #[test]
    fn test_extract_layout_names_array() {
        let client = create_test_client();

        let json = r#"{"xkb_layout_names": ["English", "Deutsch", "Francais"]}"#;
        let names = client.extract_layout_names_array(json);

        assert!(names.is_some());
        let names = names.unwrap();
        assert_eq!(names.len(), 3);
        assert_eq!(names[0], "English");
        assert_eq!(names[1], "Deutsch");
        assert_eq!(names[2], "Francais");
    }

    #[test]
    fn test_extract_active_layout_index() {
        let client = create_test_client();

        let json = r#"{"xkb_active_layout_index": 2}"#;
        let idx = client.extract_active_layout_index(json);
        assert_eq!(idx, Some(2));
    }

    #[test]
    fn test_validate_payload_len() {
        assert!(SwayClient::validate_payload_len(1024).is_ok());
        assert_eq!(SwayClient::validate_payload_len(1024).unwrap(), 1024);

        assert!(SwayClient::validate_payload_len(MAX_IPC_PAYLOAD_SIZE).is_ok());
        assert_eq!(
            SwayClient::validate_payload_len(MAX_IPC_PAYLOAD_SIZE).unwrap(),
            MAX_IPC_PAYLOAD_SIZE as usize
        );

        let too_large = MAX_IPC_PAYLOAD_SIZE + 1;
        let result = SwayClient::validate_payload_len(too_large);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Sway IPC payload too large"));
    }
}
