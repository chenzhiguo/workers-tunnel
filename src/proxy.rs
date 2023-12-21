use std::io::{Error, ErrorKind, Result};
use std::net::{Ipv4Addr, Ipv6Addr};

use crate::websocket::WebSocketConnection;
use base64_url::decode;
use tokio::io::{copy_bidirectional, AsyncReadExt};
use worker::{console_debug, Socket};

pub fn parse_early_data(data: Option<String>) -> Result<Option<Vec<u8>>> {
    if let Some(data) = data {
        if data.len() > 0 {
            let data = data.replace("+", "-").replace("/", "_").replace("=", "");
            match decode(&data) {
                Ok(early_data) => return Ok(Some(early_data)),
                Err(_) => return Err(Error::new(ErrorKind::Other, "unsupported early data")),
            }
        }
    }
    Ok(None)
}

pub async fn run_tunnel(
    mut server_socket: WebSocketConnection<'_>,
    user_id: &str,
) -> Result<()> {
    // process request

    // read version
    let mut prefix = [0u8; 18];
    server_socket.read_exact(&mut prefix).await?;

    if prefix[0] != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "invalid client protocol version, expected 0, got {}",
                prefix[0]
            ),
        ));
    }

    // valid user id
    let target_id = &prefix[1..17];
    for (b1, b2) in parse_hex(user_id).iter().zip(target_id.iter()) {
        if b1 != b2 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Unknown user id",
            ));
        }
    }

    {
        // ignore addons
        let addon_length = prefix[17];
        let mut addon_bytes = allocate_vec(addon_length as usize).into_boxed_slice();
        server_socket.read_exact(&mut addon_bytes).await?;
    }

    // parse remote address
    let mut address_prefix = [0u8; 4];
    server_socket.read_exact(&mut address_prefix).await?;

    match address_prefix[0] {
        1 => {
            // tcp, noop.
        }
        2 => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "UDP was requested",
            ));
        }
        unknown_protocol_type => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unknown requested protocol: {}", unknown_protocol_type),
            ));
        }
    }

    let port = ((address_prefix[1] as u16) << 8) | (address_prefix[2] as u16);

    let remote_addr = match address_prefix[3] {
        1 => {
            // 4 byte ipv4 address
            let mut address_bytes = [0u8; 4];
            server_socket.read_exact(&mut address_bytes).await?;

            let v4addr: Ipv4Addr = Ipv4Addr::new(
                address_bytes[0],
                address_bytes[1],
                address_bytes[2],
                address_bytes[3],
            );
            v4addr.to_string()
        }
        2 => {
            // domain name
            let mut domain_name_len = [0u8; 1];
            server_socket.read_exact(&mut domain_name_len).await?;

            let mut domain_name_bytes = allocate_vec(domain_name_len[0] as usize);
            server_socket.read_exact(&mut domain_name_bytes).await?;

            let address_str = match std::str::from_utf8(&domain_name_bytes) {
                Ok(s) => s,
                Err(e) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Failed to decode address: {}", e),
                    ));
                }
            };
            address_str.to_string()
        }
        3 => {
            // 16 byte ipv6 address
            let mut address_bytes = [0u8; 16];
            server_socket.read_exact(&mut address_bytes).await?;

            let v6addr = Ipv6Addr::new(
                ((address_bytes[0] as u16) << 8) | (address_bytes[1] as u16),
                ((address_bytes[2] as u16) << 8) | (address_bytes[3] as u16),
                ((address_bytes[4] as u16) << 8) | (address_bytes[5] as u16),
                ((address_bytes[6] as u16) << 8) | (address_bytes[7] as u16),
                ((address_bytes[8] as u16) << 8) | (address_bytes[9] as u16),
                ((address_bytes[10] as u16) << 8) | (address_bytes[11] as u16),
                ((address_bytes[12] as u16) << 8) | (address_bytes[13] as u16),
                ((address_bytes[14] as u16) << 8) | (address_bytes[15] as u16),
            );
            format!("[{}]", v6addr.to_string())
        }
        invalid_type => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid address type: {}", invalid_type),
            ));
        }
    };

    // connect to remote socket
    let mut remote_socket = match Socket::builder().connect(remote_addr.clone(), port) {
        Ok(socket) => socket,
        Err(e) => {
            console_debug!(
                    "connect to remote {}:{} error: {}",
                    remote_addr,
                    port,
                    e.to_string()
                );

            return Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                e.to_string(),
            ));
        }
    };

    copy_bidirectional(&mut server_socket, &mut remote_socket).await?;

    Ok(())
}

fn parse_hex(hex_asm: &str) -> Box<[u8]> {
    let mut hex_bytes = hex_asm
        .as_bytes()
        .iter()
        .filter_map(|b| match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        })
        .fuse();

    let mut bytes = Vec::new();
    while let (Some(h), Some(l)) = (hex_bytes.next(), hex_bytes.next()) {
        bytes.push(h << 4 | l)
    }
    bytes.into_boxed_slice()
}

#[inline]
fn allocate_vec<T>(len: usize) -> Vec<T> {
    let mut ret = Vec::with_capacity(len);
    unsafe {
        ret.set_len(len);
    }
    ret
}