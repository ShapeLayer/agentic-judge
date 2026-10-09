//! The solving app is embedded so installation never depends on a checkout.
pub fn asset(path: &str) -> Option<(&'static [u8], &'static str)> {
    Some(match path.split('?').next()? {
        "/" | "/index.html" => (
            include_bytes!("../ui/app/index.html"),
            "text/html; charset=utf-8",
        ),
        "/app.js" => (
            include_bytes!("../ui/dist/app/app.js"),
            "text/javascript; charset=utf-8",
        ),
        "/editor.css" => (
            include_bytes!("../ui/dist/app/app.css"),
            "text/css; charset=utf-8",
        ),
        "/editor.worker.js" => (
            include_bytes!("../ui/dist/app/editor.worker.js"),
            "text/javascript; charset=utf-8",
        ),
        "/app.css" => (
            include_bytes!("../ui/app/app.css"),
            "text/css; charset=utf-8",
        ),
        "/tokens.css" => (
            include_bytes!("../ui/styles/tokens.css"),
            "text/css; charset=utf-8",
        ),
        _ => return None,
    })
}
