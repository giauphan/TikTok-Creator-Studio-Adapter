//! HTTP framing and response handlers for TikTok Creator Studio endpoints.

use crate::http::write_error;
use serde::Serialize;
use std::io::{self, Write};
use std::net::TcpStream;

/// Writes a JSON-serialized response with the given status code.
pub fn write_json_response<T: Serialize>(
    client: &mut TcpStream,
    status: u16,
    data: &T,
) -> io::Result<u16> {
    let body = serde_json::to_string(data)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let reason = match status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        _ => "OK",
    };

    write!(
        client,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    client.flush()?;
    Ok(status)
}

/// Writes a 302 redirect pointing to the target URL.
pub fn write_redirect_response(client: &mut TcpStream, location: &str) -> io::Result<u16> {
    write!(
        client,
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )?;
    client.flush()?;
    Ok(302)
}

/// Convenience wrapper for emitting a standard HTTP error.
pub fn write_tiktok_error(client: &mut TcpStream, status: u16, message: &str) -> io::Result<u16> {
    let reason = match status {
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        502 => "Bad Gateway",
        _ => "Internal Server Error",
    };
    write_error(client, status, reason, message)?;
    Ok(status)
}
