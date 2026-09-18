// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Signing in, invitations and the guard around the writing pages.

mod common;

use chrono::Utc;
use reqwest::StatusCode;

use crate::common::{
    BOOTSTRAP, Invite, PASSWORD, TestServer, auth, redirected_to,
};

/// The first `/register?token=` link on a page.
fn invite_in(html: &str) -> String {
    let rest = html
        .split_once("/register?token=")
        .unwrap_or_else(|| panic!("no invitation link: {html}"))
        .1;
    rest.chars().take_while(char::is_ascii_hexdigit).collect()
}

/// Sign in with the password form.
async fn sign_in(s: &TestServer, email: &str) -> reqwest::Response {
    s.post(
        "/login",
        "/login",
        &[("email", email), ("password", PASSWORD)],
    )
    .await
}

async fn sign_out(s: &TestServer) {
    let res = s.post("/logout", "/items", &[]).await;
    assert_eq!(redirected_to(&res), "/items");
}

#[tokio::test]
async fn the_bootstrap_invite_creates_the_first_account() {
    let Some(s) = common::spawn().await else {
        return;
    };
    common::register(&s, BOOTSTRAP).await;

    let html = s.html("/items").await;
    assert!(html.contains("welcome"), "{html}");
    assert!(html.contains("Test User"), "{html}");
    assert!(html.contains("Sign out"), "{html}");
}

#[tokio::test]
async fn the_bootstrap_invite_stops_working() {
    let Some(s) = common::spawn().await else {
        return;
    };
    common::register(&s, BOOTSTRAP).await;
    sign_out(&s).await;

    let res = s
        .post(
            "/register",
            "/register",
            &[
                ("token", BOOTSTRAP),
                ("name", "Second"),
                ("email", "second@example.test"),
                ("password", PASSWORD),
            ],
        )
        .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let html = res.text().await.unwrap();
    assert!(html.contains("not valid any more"), "{html}");
}

#[tokio::test]
async fn registering_needs_an_invitation() {
    let Some(s) = common::spawn().await else {
        return;
    };
    for token in ["", &auth::new_token()] {
        let res = s
            .post(
                "/register",
                "/register",
                &[
                    ("token", token),
                    ("name", "Nobody"),
                    ("email", "nobody@example.test"),
                    ("password", PASSWORD),
                ],
            )
            .await;
        assert_eq!(
            res.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "token {token:?}"
        );
    }
    assert_eq!(s.store.user_count().await.unwrap(), 0);
}

#[tokio::test]
async fn invitations_work_once() {
    let Some(s) = common::spawn().await else {
        return;
    };
    common::register(&s, BOOTSTRAP).await;

    let res = s
        .post("/invites", "/invites", &[("email", "guest@example.test")])
        .await;
    assert_eq!(redirected_to(&res), "/invites");
    let html = s.html("/invites").await;
    assert!(html.contains("invitation created"), "{html}");
    assert!(html.contains("guest@example.test"), "{html}");
    let token = invite_in(&html);
    assert_eq!(token.len(), 64);

    sign_out(&s).await;
    common::register(&s, &token).await;
    sign_out(&s).await;

    let res = s
        .post(
            "/register",
            "/register",
            &[
                ("token", &token),
                ("name", "Third"),
                ("email", "third@example.test"),
                ("password", PASSWORD),
            ],
        )
        .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(s.store.user_count().await.unwrap(), 2);
}

#[tokio::test]
async fn expired_invitations_are_refused() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let email = common::register(&s, BOOTSTRAP).await;
    let user = s.store.user_by_email(&email).await.unwrap().unwrap();
    sign_out(&s).await;

    let now = Utc::now();
    let invite = Invite {
        token: auth::new_token(),
        email: None,
        created_by: user.id,
        created_at: now - chrono::Duration::days(30),
        expires_at: now - chrono::Duration::days(1),
        used_at: None,
        used_by: None,
    };
    let token = invite.token.clone();
    s.store.create_invite(invite).await.unwrap();

    let res = s
        .post(
            "/register",
            "/register",
            &[
                ("token", &token),
                ("name", "Late"),
                ("email", "late@example.test"),
                ("password", PASSWORD),
            ],
        )
        .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(s.store.user_count().await.unwrap(), 1);
}

#[tokio::test]
async fn known_emails_are_not_registered_twice() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let email = common::register(&s, BOOTSTRAP).await;
    let res = s.post("/invites", "/invites", &[]).await;
    assert_eq!(redirected_to(&res), "/invites");
    let token = invite_in(&s.html("/invites").await);
    sign_out(&s).await;

    let res = s
        .post(
            "/register",
            "/register",
            &[
                ("token", &token),
                ("name", "Again"),
                ("email", &email),
                ("password", PASSWORD),
            ],
        )
        .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let html = res.text().await.unwrap();
    assert!(html.contains("already registered"), "{html}");

    // The invitation was handed back, so it still works.
    common::register(&s, &token).await;
}

#[tokio::test]
async fn sign_in_and_out() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let email = common::register(&s, BOOTSTRAP).await;
    sign_out(&s).await;

    let html = s.html("/items").await;
    assert!(html.contains("signed out"), "{html}");
    assert!(html.contains("Sign in"), "{html}");

    let res = sign_in(&s, &email).await;
    assert_eq!(redirected_to(&res), "/items");
    let html = s.html("/items").await;
    assert!(html.contains("signed in"), "{html}");
    assert!(html.contains("Test User"), "{html}");
}

#[tokio::test]
async fn bad_credentials_say_nothing_useful() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let email = common::register(&s, BOOTSTRAP).await;
    sign_out(&s).await;

    let wrong_password = s
        .post(
            "/login",
            "/login",
            &[("email", &email), ("password", "not the password")],
        )
        .await;
    assert_eq!(wrong_password.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let first = wrong_password.text().await.unwrap();

    let wrong_email = s
        .post(
            "/login",
            "/login",
            &[("email", "nobody@example.test"), ("password", PASSWORD)],
        )
        .await;
    assert_eq!(wrong_email.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let second = wrong_email.text().await.unwrap();

    for html in [&first, &second] {
        assert!(html.contains(auth::BAD_CREDENTIALS), "{html}");
        assert!(!html.contains("no such"), "{html}");
        assert!(!html.contains("unknown"), "{html}");
    }
}

#[tokio::test]
async fn writing_pages_need_an_account() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.get("/items/new").await;
    assert_eq!(redirected_to(&res), "/login?next=%2Fitems%2Fnew");

    let res = s.get("/invites").await;
    assert_eq!(redirected_to(&res), "/login?next=%2Finvites");

    // Reading stays public.
    assert_eq!(s.get("/items").await.status(), StatusCode::OK);
}

#[tokio::test]
async fn the_login_page_returns_you_where_you_were() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let email = common::register(&s, BOOTSTRAP).await;
    sign_out(&s).await;

    let html = s.html("/login?next=%2Fitems%2Fnew").await;
    assert!(html.contains("name=\"next\""), "{html}");

    let res = s
        .post(
            "/login",
            "/login?next=%2Fitems%2Fnew",
            &[
                ("email", &email),
                ("password", PASSWORD),
                ("next", "/items/new"),
            ],
        )
        .await;
    assert_eq!(redirected_to(&res), "/items/new");
}

#[tokio::test]
async fn other_sites_are_not_followed_after_a_login() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let email = common::register(&s, BOOTSTRAP).await;
    sign_out(&s).await;

    let res = s
        .post(
            "/login",
            "/login",
            &[
                ("email", &email),
                ("password", PASSWORD),
                ("next", "https://evil.test/"),
            ],
        )
        .await;
    assert_eq!(redirected_to(&res), "/items");
}
{%- if auth == "google" %}

#[tokio::test]
async fn google_sign_in_starts_at_google() {
    let Some(s) = common::spawn().await else {
        return;
    };
    let res = s.get("/auth/google").await;
    let to = redirected_to(&res);
    assert!(to.starts_with(auth::GOOGLE_AUTH_URL), "{to}");
    assert!(to.contains("code_challenge="), "{to}");
    assert!(to.contains("client_id=test-client"), "{to}");
}

#[tokio::test]
async fn a_callback_with_the_wrong_state_is_refused() {
    let Some(s) = common::spawn().await else {
        return;
    };
    // Nothing in flight at all.
    let res = s.get("/auth/google/callback?code=x&state=y").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // In flight, but the state does not match the one handed out.
    s.get("/auth/google").await;
    let res = s.get("/auth/google/callback?code=x&state=y").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let html = res.text().await.unwrap();
    assert!(html.contains("403"), "{html}");
}
{%- endif %}
