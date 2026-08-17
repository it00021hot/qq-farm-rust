//! 本地游戏配置静态资源协议 `farmcfg://`（对齐面板 `/game-config/*`）。

use std::path::{Component, Path, PathBuf};

use tauri::http::{header::CONTENT_TYPE, Request, Response, StatusCode};

use qq_farm_core::config::paths::game_config_static_dir;

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

fn safe_join(root: &Path, rel: &str) -> Option<PathBuf> {
    let mut out = root.to_path_buf();
    for part in Path::new(rel).components() {
        match part {
            Component::Normal(seg) => out.push(seg),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    let root_canon = root.canonicalize().ok()?;
    let out_canon = out.canonicalize().ok()?;
    if out_canon.starts_with(&root_canon) {
        Some(out_canon)
    } else {
        None
    }
}

fn relative_from_request(request: &Request<Vec<u8>>) -> String {
    let uri = request.uri();
    let path = uri.path().trim_start_matches('/');
    // Some platforms put the path in the host form `farmcfg.localhost/...`
    if path.is_empty() {
        uri.host().unwrap_or("").trim_start_matches("farmcfg.").to_string()
    } else {
        path.to_string()
    }
}

/// 注册自定义协议处理器。
pub fn handle_request(request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    if request.method() != tauri::http::Method::GET && request.method() != tauri::http::Method::HEAD
    {
        return Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Vec::new())
            .unwrap_or_default();
    }

    let rel = relative_from_request(&request);
    if rel.is_empty() || rel.contains("..") {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Vec::new())
            .unwrap_or_default();
    }

    let root = game_config_static_dir();
    let Some(file) = safe_join(&root, &rel) else {
        tracing::debug!(%rel, root = %root.display(), "farmcfg: path rejected or missing");
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new())
            .unwrap_or_default();
    };

    match std::fs::read(&file) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, content_type(&file))
            .header("Cache-Control", "public, max-age=86400")
            .header("Access-Control-Allow-Origin", "*")
            .body(bytes)
            .unwrap_or_default(),
        Err(err) => {
            tracing::debug!(error = %err, file = %file.display(), "farmcfg: read failed");
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Vec::new())
                .unwrap_or_default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::http::Method;

    fn get(uri: &str) -> Request<Vec<u8>> {
        Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Vec::new())
            .expect("request")
    }

    #[test]
    fn relative_from_farmcfg_localhost_path() {
        let req = get("farmcfg://localhost/seed_images_named/20218.png");
        assert_eq!(
            relative_from_request(&req),
            "seed_images_named/20218.png"
        );
    }

    #[test]
    fn serves_vendored_seed_png() {
        let res = handle_request(get("farmcfg://localhost/seed_images_named/20218.png"));
        assert_eq!(res.status(), StatusCode::OK);
        assert!(!res.body().is_empty());
        let ct = res
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert_eq!(ct, "image/png");
    }
}
