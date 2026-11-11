//! Denki security patch: bound every buffered HTTP control response before parsing.
use reqwest::Response;
use serde::de::DeserializeOwned;

use crate::tapo_vendor::{Error, TapoResponseError};

pub(super) const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

fn oversized() -> Error {
    Error::ResponseTooLarge
}

/// Content-Length is only an early rejection hint, never the allocation size or
/// the sole enforcement mechanism. Drop the response immediately on overflow.
pub(super) async fn bytes(mut response: Response) -> Result<Vec<u8>, Error> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(oversized());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if chunk.len() > MAX_RESPONSE_BYTES - body.len() {
            return Err(oversized());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

pub(super) async fn json<T: DeserializeOwned>(response: Response) -> Result<T, Error> {
    let body = bytes(response).await?;
    // Do not reflect device-controlled field values through serde's errors.
    serde_json::from_slice(&body).map_err(|_| {
        Error::Tapo(TapoResponseError::ResponseError {
            description: "Invalid JSON HTTP response".to_string(),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn response(headers: &str, body: Vec<u8>) -> Response {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let headers = format!("HTTP/1.1 200 OK\r\nConnection: close\r\n{headers}\r\n");
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 2048];
            assert!(socket.read(&mut request).await.unwrap() > 0);
            if socket.write_all(headers.as_bytes()).await.is_ok() {
                let _ = socket.write_all(&body).await;
            }
        });
        reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(format!("http://{address}"))
            .send()
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn rejects_announced_oversize_before_waiting_for_body() {
        let r = response(
            &format!("Content-Length: {}\r\n", MAX_RESPONSE_BYTES + 1),
            vec![],
        )
        .await;
        assert!(
            bytes(r)
                .await
                .unwrap_err()
                .to_string()
                .contains("1048576-byte limit")
        );
    }

    #[tokio::test]
    async fn accepts_exact_limit_and_rejects_one_more_without_length() {
        assert_eq!(
            bytes(response("", vec![0; MAX_RESPONSE_BYTES]).await)
                .await
                .unwrap()
                .len(),
            MAX_RESPONSE_BYTES
        );
        let error = bytes(response("", vec![b'x'; MAX_RESPONSE_BYTES + 1]).await)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("1048576-byte limit"));
        assert!(error.to_string().len() < 100);
    }

    #[tokio::test]
    async fn rejects_chunked_oversize_even_with_false_content_length() {
        for headers in [
            "Transfer-Encoding: chunked\r\n",
            "Transfer-Encoding: chunked\r\nContent-Length: 1\r\n",
        ] {
            let mut body = Vec::new();
            for _ in 0..257 {
                body.extend_from_slice(b"1000\r\n");
                body.extend_from_slice(&[b'x'; 4096]);
                body.extend_from_slice(b"\r\n");
            }
            body.extend_from_slice(b"0\r\n\r\n");
            let error = bytes(response(headers, body).await).await.unwrap_err();
            assert!(error.to_string().contains("1048576-byte limit"));
        }
    }

    #[tokio::test]
    async fn truncated_declared_body_fails_and_small_valid_json_succeeds() {
        assert!(
            bytes(response("Content-Length: 10\r\n", b"x".to_vec()).await)
                .await
                .is_err()
        );
        let value: serde_json::Value = json(response("", b"{\"ok\":true}".to_vec()).await)
            .await
            .unwrap();
        assert_eq!(value["ok"], true);
    }

    #[tokio::test]
    async fn invalid_json_error_does_not_echo_device_content() {
        let error =
            json::<u8>(response("", format!("\"{}\"", "secret".repeat(1000)).into_bytes()).await)
                .await
                .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Response error: Invalid JSON HTTP response"
        );
    }
}
