//! Reqwest-backed HTTP client for GPUI's image cache.
//!
//! GPUI's default client rejects every request, so remote avatars (the WeChat
//! profile pictures and friend avatars served as `https://…` image sources)
//! silently fell back to letter placeholders. Requests execute on the
//! application's Tokio runtime because GPUI's executors do not provide a Tokio
//! reactor of their own.

use futures::future::BoxFuture;
use gpui_kit::http_client::{
    http, AsyncBody, HttpClient, RedirectPolicy, RequestTimeout, Response, Result, Url,
};
use std::sync::Arc;

/// Redirect budget for `RedirectPolicy::FollowAll`, matching reqwest's default.
const MAX_REDIRECTS: usize = 10;

/// Serves remote images for `img(https://…)` sources through reqwest.
pub(crate) struct ReqwestHttpClient {
    follow_client: reqwest::Client,
    no_follow_client: reqwest::Client,
    runtime: tokio::runtime::Handle,
    user_agent: Option<http::HeaderValue>,
}

impl ReqwestHttpClient {
    pub(crate) fn new(runtime: &Arc<tokio::runtime::Runtime>) -> Self {
        // reqwest models redirect policy per client, so keep one client per
        // policy instead of rebuilding a client for every image request.
        let follow_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
            .build()
            .expect("build the follow-redirect image client");
        let no_follow_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("build the no-redirect image client");
        Self {
            follow_client,
            no_follow_client,
            runtime: runtime.handle().clone(),
            user_agent: http::HeaderValue::from_str(concat!(
                "qq-farm-gpui/",
                env!("CARGO_PKG_VERSION")
            ))
            .ok(),
        }
    }
}

impl HttpClient for ReqwestHttpClient {
    fn user_agent(&self) -> Option<&http::HeaderValue> {
        self.user_agent.as_ref()
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn send(
        &self,
        req: http::Request<AsyncBody>,
    ) -> BoxFuture<'static, Result<Response<AsyncBody>>> {
        let redirect = req.extensions().get::<RedirectPolicy>().cloned().unwrap_or_default();
        let client = match redirect {
            RedirectPolicy::NoFollow => self.no_follow_client.clone(),
            // `FollowLimit` has no per-request form in reqwest; every bounded
            // policy shares the limited follow client.
            RedirectPolicy::FollowLimit(_) | RedirectPolicy::FollowAll => {
                self.follow_client.clone()
            }
        };
        let request = self.runtime.spawn(dispatch(client, req));
        Box::pin(async move { request.await? })
    }
}

async fn dispatch(
    client: reqwest::Client,
    req: http::Request<AsyncBody>,
) -> Result<Response<AsyncBody>> {
    let (parts, mut body) = req.into_parts();
    let timeout = parts.extensions.get::<RequestTimeout>().copied();
    let mut bytes = Vec::new();
    futures::AsyncReadExt::read_to_end(&mut body, &mut bytes).await?;
    let mut builder = client.request(parts.method, parts.uri.to_string()).headers(parts.headers);
    if let Some(RequestTimeout(duration)) = timeout {
        builder = builder.timeout(duration);
    }
    let response = builder.body(bytes).send().await?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await?;
    let mut response = Response::builder().status(status).body(AsyncBody::from_bytes(body))?;
    *response.headers_mut() = headers;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::{http, AsyncBody, HttpClient, ReqwestHttpClient};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;

    #[test]
    fn remote_requests_return_the_fetched_bytes_and_status() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test server");
        let address = listener.local_addr().expect("local server address");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept one request");
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nContent-Type: image/png\r\nConnection: close\r\n\r\nabcd",
                )
                .expect("write test response");
        });

        let runtime = Arc::new(tokio::runtime::Runtime::new().expect("test runtime"));
        let client = ReqwestHttpClient::new(&runtime);
        let request = http::Request::builder()
            .uri(format!("http://{address}/avatar.png"))
            .body(AsyncBody::empty())
            .expect("build test request");
        let response = runtime.block_on(client.send(request)).expect("fetch the remote image");
        assert_eq!(response.status(), http::StatusCode::OK);
        let mut response = response;
        let mut body = Vec::new();
        runtime
            .block_on(futures::AsyncReadExt::read_to_end(response.body_mut(), &mut body))
            .expect("read the response body");
        assert_eq!(body, b"abcd");
        server.join().expect("join the test server");
    }
}
