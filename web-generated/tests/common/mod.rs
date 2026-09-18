// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Start the app on an ephemeral port and talk to it over HTTP with a
//! cookie jar, so sessions, flash messages and form tokens behave the
//! way they do in a browser.

// Every test binary compiles the whole module, not just what it uses.
#![allow(dead_code)]

use std::path::PathBuf;

// Everything the tests take from the crate, so they only import here.
#[allow(unused_imports)]
pub use web_generated::{
    AppState, INVITE_TTL, app,
    auth::{self, Config as AuthConfig},
    csrf,
    model::Invite,
    render::{Config as TemplateConfig, Templates},
    routes::Health,
    session::{self, Config as SessionConfig},
    store::Store,
};

/// Accepted as a registration token while no account exists.
pub const BOOTSTRAP: &str = "bootstrap-token-for-tests";

/// Long enough that nothing expires mid-test.
pub const PASSWORD: &str = "correct horse battery staple";

pub struct TestServer {
    pub base_url: String,
    pub client: reqwest::Client,
    /// The same store the app uses, for arranging state directly.
    pub store: Store,
}

impl TestServer {
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    /// `GET path`.
    pub async fn get(&self, path: &str) -> reqwest::Response {
        self.client.get(self.url(path)).send().await.unwrap()
    }

    /// Body of `GET path`, which must have answered `200`.
    pub async fn html(&self, path: &str) -> String {
        let res = self.get(path).await;
        assert_eq!(res.status(), reqwest::StatusCode::OK, "GET {path}");
        res.text().await.unwrap()
    }

    /// The form token on `path`; every page with a form carries one.
    pub async fn token(&self, path: &str) -> String {
        let html = self.html(path).await;
        token_in(&html)
            .unwrap_or_else(|| panic!("no form token on {path}: {html}"))
    }

    /// `POST path` with `fields` plus a token taken from `from`.
    pub async fn post(
        &self,
        path: &str,
        from: &str,
        fields: &[(&str, &str)],
    ) -> reqwest::Response {
        let token = self.token(from).await;
        let mut form = vec![(csrf::FIELD, token.as_str())];
        form.extend(fields.iter().map(|(k, v)| (*k, *v)));
        self.post_raw(path, &form).await
    }

    /// `POST path` with exactly `fields`, token included or not.
    pub async fn post_raw(
        &self,
        path: &str,
        fields: &[(&str, &str)],
    ) -> reqwest::Response {
        self.client
            .post(self.url(path))
            .form(fields)
            .send()
            .await
            .unwrap()
    }
}

/// The `csrf_token` of the first form in `html`.
pub fn token_in(html: &str) -> Option<String> {
    let marker = format!("name=\"{}\" value=\"", csrf::FIELD);
    let rest = html.split_once(&marker)?.1;
    Some(rest.split_once('"')?.0.to_owned())
}

/// `Location` of a redirect, which must have answered `303`.
pub fn redirected_to(res: &reqwest::Response) -> String {
    assert_eq!(res.status(), reqwest::StatusCode::SEE_OTHER, "{res:?}");
    res.headers()["location"].to_str().unwrap().to_owned()
}

/// Spawn a server with a fresh store. Returns `None` when the backing
/// store is unavailable, so the test is skipped.
pub async fn spawn() -> Option<TestServer> {
    spawn_with(AuthConfig {
        invite_ttl: INVITE_TTL,
        bootstrap_invite: Some(BOOTSTRAP.to_string()),
    })
    .await
}

/// [`spawn`] with explicit account settings.
pub async fn spawn_with(auth: AuthConfig) -> Option<TestServer> {
    let store = Store::connect("sqlite::memory:").await.expect("connect");
    store.migrate().await.expect("migrate");

    let sessions = session::store(&store).await.expect("session store");
    let session = session::layer(sessions, &SessionConfig::default());
    let state = AppState {
        store: store.clone(),
        templates: Templates::new(&TemplateConfig {
            dir: root().join("templates"),
            reload: false,
        }),
        auth,
    };

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = app(state, session, &root().join("static"));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    Some(TestServer {
        base_url: format!("http://{addr}"),
        client,
        store,
    })
}

/// Sign in, so the guarded pages are reachable. Without accounts there
/// is nothing to do.
pub async fn sign_in(s: &TestServer) {
    register(s, BOOTSTRAP).await;
}

/// Create an account against `token` and sign it in. Returns the email.
pub async fn register(s: &TestServer, token: &str) -> String {
    let email = format!("{}@example.test", uuid::Uuid::new_v4().simple());
    let res = s
        .post(
            "/register",
            "/register",
            &[
                ("token", token),
                ("name", "Test User"),
                ("email", &email),
                ("password", PASSWORD),
            ],
        )
        .await;
    assert_eq!(redirected_to(&res), "/items", "register with {token}");
    email
}

/// The crate root; tests must not depend on the working directory.
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
