//
// Copyright (c) 2025 murilo ijanc' <murilo@ijanc.org>
//
// Permission to use, copy, modify, and distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
//

use std::time::Duration;

use axum::{Router, response::Html, routing::get};
use tokio::net::TcpListener;
use tower_http::{
    services::ServeDir, timeout::TimeoutLayer, trace::TraceLayer,
};
use tracing::info;

mod helpers;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    helpers::init_tracing();

    let app = Router::new()
        .route("/", get(handler))
        // TODO(msi): from config folder asssets
        .nest_service("/assets", ServeDir::new("assets"))
        .layer((
            TraceLayer::new_for_http(),
            TimeoutLayer::new(Duration::from_secs(10)), // TODO(msi): from config
        ));

    // TODO(msi): from config
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    info!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app)
        .with_graceful_shutdown(helpers::shutdown_signal())
        .await?;

    Ok(())
}

const INDEX: &'static str = r#"<html>
<head>
<link href="/assets/css/styles.css" rel="stylesheet" type="text/css">
</head>
<body>
<h1><h1>Hello, World {{project-name}} =]</h1>
<p>Template form https://ijanc.org</p>
</body>
</html>
"#;

async fn handler() -> Html<&'static str> {
    Html(INDEX)
}
