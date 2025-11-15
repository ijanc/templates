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

use std::net::SocketAddr;
use std::sync::Arc;

use minijinja::Environment;
use tokio::net::TcpListener;
use tracing::info;

mod helpers;
mod metric;
mod router;
mod settings;
mod state;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    helpers::init_tracing();

    let _settings = settings::Settings::new();

    let (_main_server, _metrics_server) =
        tokio::join!(start_main_server(), metric::start_metrics_server());
    Ok(())
}

async fn start_main_server() -> anyhow::Result<()> {
    let mut env = Environment::new();
    env.add_template("layout", include_str!("../templates/layout.jinja"))?;
    env.add_template("home", include_str!("../templates/home.jinja"))?;
    env.add_template("content", include_str!("../templates/content.jinja"))?;
    env.add_template("about", include_str!("../templates/about.jinja"))?;
    env.add_template("csrf", include_str!("../templates/csrf.jinja"))?;
    env.add_template(
        "validation",
        include_str!("../templates/validation.jinja"),
    )?;

    let app_state = Arc::new(state::AppState { env });

    let app = router::route(app_state);

    // TODO(msi): from config
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    info!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(helpers::shutdown_signal())
    .await?;
    Ok(())
}
