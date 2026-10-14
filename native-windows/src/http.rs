//! WinHTTP transport for the helper. Certificates are verified, redirects and ambient auth disabled.
use crate::remote::{Endpoint, RemoteError};
use std::ffi::c_void;
use windows::Win32::Networking::WinHttp::*;
use windows::core::{HSTRING, PCWSTR};

pub struct Response {
    pub status: u32,
    pub bytes: Vec<u8>,
}
pub trait Transport {
    fn request(
        &self,
        url: &str,
        method: &str,
        token: Option<&str>,
        body: Option<&[u8]>,
        local: bool,
        limit: usize,
    ) -> Result<Response, RemoteError>;
}
struct Internet(*mut c_void);
impl Internet {
    fn new(handle: *mut c_void) -> Result<Self, RemoteError> {
        if handle.is_null() {
            Err(RemoteError::Network)
        } else {
            Ok(Self(handle))
        }
    }
}
impl Drop for Internet {
    fn drop(&mut self) {
        unsafe {
            let _ = WinHttpCloseHandle(self.0);
        }
    }
}
pub struct NativeHTTP;
impl Transport for NativeHTTP {
    fn request(
        &self,
        url: &str,
        method: &str,
        token: Option<&str>,
        body: Option<&[u8]>,
        local: bool,
        limit: usize,
    ) -> Result<Response, RemoteError> {
        crate::helper::require_unpacked()?;
        let endpoint = Endpoint::parse(url, local)?;
        if !matches!(method, "GET" | "POST")
            || limit > 8_000_000
            || body.is_some_and(|b| b.len() > 8_000_000)
            || token.is_some_and(|t| {
                t.is_empty() || t.len() > 2560 || t.bytes().any(|b| !(33..127).contains(&b))
            })
        {
            return Err(RemoteError::Configuration);
        }
        let agent = HSTRING::from(concat!("LingoMate/", env!("CARGO_PKG_VERSION")));
        let session = Internet::new(unsafe {
            WinHttpOpen(
                &agent,
                if local {
                    WINHTTP_ACCESS_TYPE_NO_PROXY
                } else {
                    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY
                },
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            )
        })?;
        unsafe { WinHttpSetTimeouts(session.0, 5000, 5000, 20000, 20000) }
            .map_err(|_| RemoteError::Network)?;
        let connection = Internet::new(unsafe {
            WinHttpConnect(session.0, &HSTRING::from(&endpoint.host), endpoint.port, 0)
        })?;
        let request = Internet::new(unsafe {
            WinHttpOpenRequest(
                connection.0,
                &HSTRING::from(method),
                &HSTRING::from(&endpoint.path),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                if endpoint.secure {
                    WINHTTP_FLAG_SECURE
                } else {
                    WINHTTP_OPEN_REQUEST_FLAGS(0)
                },
            )
        })?;
        let disabled =
            (WINHTTP_DISABLE_REDIRECTS | WINHTTP_DISABLE_COOKIES | WINHTTP_DISABLE_AUTHENTICATION)
                .to_ne_bytes();
        unsafe {
            WinHttpSetOption(
                Some(request.0),
                WINHTTP_OPTION_DISABLE_FEATURE,
                Some(&disabled),
            )
        }
        .map_err(|_| RemoteError::Network)?;
        let mut headers = String::from("Accept: application/json\r\n");
        if body.is_some() {
            headers.push_str("Content-Type: application/json\r\n");
        }
        if let Some(token) = token {
            headers.push_str("Authorization: Bearer ");
            headers.push_str(token);
            headers.push_str("\r\n");
        }
        let headers: Vec<u16> = headers.encode_utf16().collect();
        let body_size = body.map_or(0, |v| v.len() as u32);
        unsafe {
            WinHttpSendRequest(
                request.0,
                Some(&headers),
                body.map(|v| v.as_ptr().cast()),
                body_size,
                body_size,
                0,
            )
        }
        .map_err(|_| RemoteError::Network)?;
        unsafe { WinHttpReceiveResponse(request.0, std::ptr::null_mut()) }
            .map_err(|_| RemoteError::Network)?;
        let mut status = 0u32;
        let mut size = 4u32;
        let mut index = 0u32;
        unsafe {
            WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some((&mut status as *mut u32).cast()),
                &mut size,
                &mut index,
            )
        }
        .map_err(|_| RemoteError::Network)?;
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            let mut read = 0;
            unsafe {
                WinHttpReadData(
                    request.0,
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    &mut read,
                )
            }
            .map_err(|_| RemoteError::Network)?;
            if read == 0 {
                break;
            }
            if bytes.len() + read as usize > limit {
                return Err(RemoteError::Network);
            }
            bytes.extend_from_slice(&buffer[..read as usize]);
        }
        Ok(Response { status, bytes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    #[test]
    fn redirects_are_not_followed_and_body_is_bounded() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        );
        let server = std::thread::spawn(move || {
            for reply in [
                "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\n12345678",
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut bytes = [0; 4096];
                let _ = stream.read(&mut bytes).unwrap();
                stream.write_all(reply.as_bytes()).unwrap();
            }
        });
        assert_eq!(
            NativeHTTP
                .request(&url, "GET", None, None, true, 32)
                .unwrap()
                .status,
            302
        );
        assert!(
            NativeHTTP
                .request(&url, "GET", None, None, true, 4)
                .is_err()
        );
        server.join().unwrap();
    }
}
