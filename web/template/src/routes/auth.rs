// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Logging in, registering against an invite and handing invites out.
{%- if auth == "google" %}
//!
//! Google is an alternative on the same login page, not a replacement:
//! an account made through Google has no password, an account made with
//! the form has no provider id, and both are looked up by email.
{%- endif %}

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    middleware,
    response::{Html, IntoResponse, Response},
    routing::{MethodRouter, get, post},
};
use chrono::Utc;
use minijinja::context;
use serde::Deserialize;
use validator::Validate;

use crate::{
    AppState,
    auth::{self, BAD_CREDENTIALS},
    error::Error,
    extract::{Form, Query},
    flash::Flash,
    model::{InviteForm, LoginForm, NewUser, RegisterForm, field_errors},
    render::Renderer,
};

/// Merged into the root router.
pub fn router(state: &AppState) -> Router<AppState> {
    let guard =
        middleware::from_fn_with_state(state.clone(), auth::require_auth);
    let guarded = |m: MethodRouter<AppState>| m.route_layer(guard.clone());
    Router::new()
        .route("/login", get(login_form).merge(post(login)))
        .route("/logout", post(logout))
        .route("/register", get(register_form).merge(post(register)))
        .route("/invites", guarded(get(invites).merge(post(create_invite))))
{%- if auth == "google" %}
        .route("/auth/google", get(google_start))
        .route("/auth/google/callback", get(google_callback))
{%- endif %}
}

/// `?next=` of the login page.
#[derive(Debug, Default, Deserialize, Validate)]
pub struct NextQuery {
    pub next: Option<String>,
}

/// `?token=` of the registration page.
#[derive(Debug, Default, Deserialize, Validate)]
pub struct TokenQuery {
    pub token: Option<String>,
}

/// The login form; already logged in users skip it.
pub async fn login_form(
    r: Renderer,
    Query(q): Query<NextQuery>,
) -> Result<Response, Error> {
    if r.user().is_some() {
        return r.redirect("/items", Flash::info("already signed in")).await;
    }
    let next = auth::safe_next(q.next.as_deref());
    let form = LoginForm {
        next: next.clone(),
        ..LoginForm::default()
    };
    let ctx = context! { form, next, errors => no_errors() };
    Ok(r.render("auth/login.html", ctx)?.into_response())
}

/// Check the credentials and start a session.
pub async fn login(
    State(state): State<AppState>,
    r: Renderer,
    Form(input): Form<LoginForm>,
) -> Result<Response, Error> {
    let next = auth::safe_next(input.next.as_deref());
    if let Err(e) = input.validate() {
        return login_again(&r, &input, Some(field_errors(&e)));
    }

    let user = state.store.user_by_email(&input.email).await?;
    let ok = match user.as_ref().and_then(|u| u.password_hash.as_deref()) {
        Some(hash) => auth::verify_password(&input.password, hash)?,
        // Hash nothing when there is no account, but answer the same.
        None => false,
    };
    let Some(user) = user.filter(|_| ok) else {
        return login_again(&r, &input, None);
    };

    auth::login(r.session(), &user).await?;
    let to = next.unwrap_or_else(|| "/items".to_string());
    r.redirect(&to, Flash::success("signed in")).await
}

/// End the session.
pub async fn logout(r: Renderer) -> Result<Response, Error> {
    auth::logout(r.session()).await?;
    r.redirect("/items", Flash::info("signed out")).await
}

/// The registration form; the token usually arrives in the link.
pub async fn register_form(
    r: Renderer,
    Query(q): Query<TokenQuery>,
) -> Result<Response, Error> {
    if r.user().is_some() {
        return r.redirect("/items", Flash::info("already signed in")).await;
    }
    let form = RegisterForm {
        token: q.token.unwrap_or_default(),
        ..RegisterForm::default()
    };
    let ctx = context! { form, errors => no_errors() };
    Ok(r.render("auth/register.html", ctx)?.into_response())
}

/// Spend an invite on a new account and sign it in.
pub async fn register(
    State(state): State<AppState>,
    r: Renderer,
    Form(input): Form<RegisterForm>,
) -> Result<Response, Error> {
    if let Err(e) = input.validate() {
        return register_again(&r, &input, Some(field_errors(&e)), None);
    }

    let bootstrap = is_bootstrap(&state, &input.token).await?;
    if !bootstrap && !state.store.claim_invite(&input.token).await? {
        let message = "this invitation is not valid any more";
        return register_again(&r, &input, None, Some(message));
    }

    let hash = auth::hash_password(&input.password)?;
    let created = state
        .store
        .create_user(NewUser {
            email: input.email.clone(),
            name: input.name.clone(),
            avatar_url: None,
            password_hash: Some(hash),
            provider_id: None,
        })
        .await?;

    let Some(user) = created else {
        if !bootstrap {
            // Give the invite back, the account was never made.
            state.store.settle_invite(&input.token, None).await?;
        }
        let message = "that email address is already registered";
        return register_again(&r, &input, None, Some(message));
    };
    if !bootstrap {
        state
            .store
            .settle_invite(&input.token, Some(user.id))
            .await?;
    }

    auth::login(r.session(), &user).await?;
    r.redirect("/items", Flash::success("welcome")).await
}

/// Every invite, plus the form that makes another one.
pub async fn invites(
    State(state): State<AppState>,
    r: Renderer,
) -> Result<Html<String>, Error> {
    let invites = state.store.list_invites().await?;
    let ctx = context! {
        invites,
        now => Utc::now(),
        form => InviteForm::default(),
        errors => no_errors(),
    };
    r.render("invites/index.html", ctx)
}

/// Hand out a new invite.
pub async fn create_invite(
    State(state): State<AppState>,
    r: Renderer,
    Form(input): Form<InviteForm>,
) -> Result<Response, Error> {
    let user = r
        .user()
        .ok_or_else(|| Error::Forbidden("sign in first".into()))?;
    if let Err(e) = input.validate() {
        let invites = state.store.list_invites().await?;
        let ctx = context! {
            invites,
            now => Utc::now(),
            form => input,
            errors => field_errors(&e),
        };
        return r.render_status(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invites/index.html",
            ctx,
        );
    }
    let invite = auth::new_invite(
        user.id,
        input.email,
        state.auth.invite_ttl,
        Utc::now(),
    );
    state.store.create_invite(invite).await?;
    r.redirect("/invites", Flash::success("invitation created"))
        .await
}

/// Whether `token` is the configured bootstrap invite and there is
/// still nobody to hand out real ones.
async fn is_bootstrap(state: &AppState, token: &str) -> Result<bool, Error> {
    let configured = state.auth.bootstrap_invite.as_deref();
    if configured != Some(token) {
        return Ok(false);
    }
    Ok(state.store.user_count().await? == 0)
}

/// Re-render the login form. Which field was wrong is never said.
fn login_again(
    r: &Renderer,
    input: &LoginForm,
    errors: Option<FieldErrors>,
) -> Result<Response, Error> {
    let ctx = context! {
        form => input,
        next => input.next,
        errors => errors.unwrap_or_else(no_errors_map),
        message => BAD_CREDENTIALS,
    };
    r.render_status(StatusCode::UNPROCESSABLE_ENTITY, "auth/login.html", ctx)
}

/// Re-render the registration form with whatever went wrong.
fn register_again(
    r: &Renderer,
    input: &RegisterForm,
    errors: Option<FieldErrors>,
    message: Option<&str>,
) -> Result<Response, Error> {
    let ctx = context! {
        form => input,
        errors => errors.unwrap_or_else(no_errors_map),
        message,
    };
    r.render_status(StatusCode::UNPROCESSABLE_ENTITY, "auth/register.html", ctx)
}

type FieldErrors = std::collections::BTreeMap<String, Vec<String>>;

fn no_errors_map() -> FieldErrors {
    FieldErrors::new()
}

fn no_errors() -> minijinja::Value {
    minijinja::Value::from(no_errors_map())
}
{%- if auth == "google" %}

/// `?token=` and `?next=` carried through the round trip to Google.
#[derive(Debug, Default, Deserialize, Validate)]
pub struct GoogleQuery {
    pub token: Option<String>,
    pub next: Option<String>,
}

/// What Google sends back.
#[derive(Debug, Default, Deserialize, Validate)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// Send the visitor to Google, remembering the request in the session.
pub async fn google_start(
    State(state): State<AppState>,
    r: Renderer,
    Query(q): Query<GoogleQuery>,
) -> Result<Response, Error> {
    use oauth2::{PkceCodeChallenge, Scope};

    let client = auth::google_client(&state.auth.google)?;
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, csrf) = client
        .authorize_url(oauth2::CsrfToken::new_random)
        .add_scope(Scope::new("email".into()))
        .add_scope(Scope::new("profile".into()))
        .set_pkce_challenge(challenge)
        .url();

    let session = r.session();
    let next = auth::safe_next(q.next.as_deref());
    session.insert(auth::STATE_KEY, csrf.secret()).await?;
    session
        .insert(auth::VERIFIER_KEY, verifier.secret())
        .await?;
    session.insert(auth::INVITE_KEY, q.token).await?;
    session.insert(auth::NEXT_KEY, next).await?;

    Ok(axum::response::Redirect::to(url.as_str()).into_response())
}

/// Finish the round trip: check the state, swap the code for a token,
/// then sign in a known email or spend an invite on a new account.
pub async fn google_callback(
    State(state): State<AppState>,
    r: Renderer,
    Query(q): Query<CallbackQuery>,
) -> Result<Response, Error> {
    use oauth2::{AuthorizationCode, PkceCodeVerifier, TokenResponse};

    let session = r.session();
    let expected = session.remove::<String>(auth::STATE_KEY).await?;
    let verifier = session.remove::<String>(auth::VERIFIER_KEY).await?;
    // Both of these were stored as an `Option`, so unwrap one level.
    let invite = session
        .remove::<Option<String>>(auth::INVITE_KEY)
        .await?
        .flatten();
    let next = session
        .remove::<Option<String>>(auth::NEXT_KEY)
        .await?
        .flatten();

    if let Some(e) = q.error {
        return Err(Error::Forbidden(format!("google refused: {e}")));
    }
    let (Some(expected), Some(got)) = (expected, q.state) else {
        return Err(Error::Forbidden("no sign in is in progress".into()));
    };
    if expected != got {
        return Err(Error::Forbidden("sign in state did not match".into()));
    }
    let (Some(verifier), Some(code)) = (verifier, q.code) else {
        return Err(Error::Forbidden("no sign in is in progress".into()));
    };

    let client = auth::google_client(&state.auth.google)?;
    let http = auth::http_client()?;
    let token = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(PkceCodeVerifier::new(verifier))
        .request_async(&http)
        .await
        .map_err(|e| Error::Unavailable(format!("google token: {e}")))?;
    let info =
        auth::google_userinfo(&http, token.access_token().secret()).await?;

    let to = next.unwrap_or_else(|| "/items".to_string());
    if let Some(user) = state.store.user_by_email(&info.email).await? {
        auth::login(r.session(), &user).await?;
        return r.redirect(&to, Flash::success("signed in")).await;
    }

    let token = invite.unwrap_or_default();
    let bootstrap = is_bootstrap(&state, &token).await?;
    if !bootstrap && !state.store.claim_invite(&token).await? {
        return Err(Error::Forbidden(
            "an invitation is needed to create an account".into(),
        ));
    }
    let created = state
        .store
        .create_user(NewUser {
            email: info.email,
            name: info.name.unwrap_or_default(),
            avatar_url: info.picture,
            password_hash: None,
            provider_id: Some(info.sub),
        })
        .await?;
    let Some(user) = created else {
        if !bootstrap {
            state.store.settle_invite(&token, None).await?;
        }
        return Err(Error::Forbidden("that account already exists".into()));
    };
    if !bootstrap {
        state.store.settle_invite(&token, Some(user.id)).await?;
    }

    auth::login(r.session(), &user).await?;
    r.redirect(&to, Flash::success("welcome")).await
}
{%- endif %}
