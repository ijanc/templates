// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! MiniJinja rendering. [`Templates`] owns the environment, [`Renderer`]
//! is the extractor handlers use: it carries the session bound values
//! (flash message, CSRF token, current user) and merges them into
//! every context, so pages never have to pass them by hand.

use std::{path::PathBuf, sync::Arc};

use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{Html, IntoResponse, Redirect, Response},
};
use minijinja::{Environment, context, path_loader, value::merge_maps};
use minijinja_autoreload::AutoReloader;
use tower_sessions::Session;

use crate::{
    AppState,
    auth::CurrentUser,
    csrf::{self, CsrfToken},
    error::Error,
    flash::{self, Flash},
    model::User,
};

/// Where templates live and whether they are watched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Directory passed to MiniJinja's path loader.
    pub dir: PathBuf,
    /// Reload templates when the directory changes.
    pub reload: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dir: PathBuf::from("templates"),
            // Convenient while developing, pointless once deployed.
            reload: cfg!(debug_assertions),
        }
    }
}

/// The MiniJinja environment, rebuilt on change when watching.
#[derive(Clone)]
pub struct Templates(Arc<AutoReloader>);

impl std::fmt::Debug for Templates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Templates").finish_non_exhaustive()
    }
}

impl Templates {
    /// Load templates from `cfg.dir`. Nothing is read until the first
    /// render, so a missing directory surfaces as a render error.
    pub fn new(cfg: &Config) -> Self {
        let dir = cfg.dir.clone();
        let reload = cfg.reload;
        Self(Arc::new(AutoReloader::new(move |notifier| {
            let mut env = Environment::new();
            env.set_loader(path_loader(&dir));
            if reload {
                notifier.set_fast_reload(true);
                notifier.watch_path(&dir, true);
            }
            Ok(env)
        })))
    }

    /// Render `name` with `ctx`.
    pub fn render(
        &self,
        name: &str,
        ctx: minijinja::Value,
    ) -> anyhow::Result<String> {
        let env = self.0.acquire_env()?;
        Ok(env.get_template(name)?.render(ctx)?)
    }

    /// Render the first of `names` that exists.
    pub fn render_any(
        &self,
        names: &[&str],
        ctx: minijinja::Value,
    ) -> anyhow::Result<String> {
        let env = self.0.acquire_env()?;
        let mut last = None;
        for name in names {
            match env.get_template(name) {
                Ok(t) => return Ok(t.render(ctx)?),
                Err(e) => last = Some(e),
            }
        }
        match last {
            Some(e) => Err(e.into()),
            None => anyhow::bail!("no template named"),
        }
    }
}

/// Per request rendering handle. Pages get `app_name`, `version`,
/// `path`, `csrf_token`, `flash` and `user` for free; keys in the
/// context passed to [`Renderer::render`] win over those.
#[derive(Debug)]
pub struct Renderer {
    templates: Templates,
    session: Session,
    flash: Option<Flash>,
    csrf_token: CsrfToken,
    path: String,
    user: Option<User>,
}

impl<S> FromRequestParts<S> for Renderer
where
    AppState: axum::extract::FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Error> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|(_, e)| Error::Internal(anyhow::anyhow!("{e}")))?;
        // Reading it clears it: a flash shows exactly once.
        let flash = flash::take(&session).await?;
        let csrf_token = csrf::token(&session).await?;
        let CurrentUser(user) =
            CurrentUser::from_request_parts(parts, state).await?;
        let app = axum::extract::FromRef::from_ref(state);
        let AppState { templates, .. } = app;
        Ok(Self {
            templates,
            session,
            flash,
            csrf_token,
            path: crate::request_path(&parts.extensions, &parts.uri),
            user,
        })
    }
}

impl Renderer {
    /// The CSRF token of this session; forms post it back.
    pub fn csrf_token(&self) -> &str {
        self.csrf_token.as_str()
    }

    /// The session behind the request, for handlers that log a user in
    /// or out.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// The logged in user, if any.
    pub fn user(&self) -> Option<&User> {
        self.user.as_ref()
    }

    /// Values merged into every context.
    fn base(&self) -> minijinja::Value {
        context! {
            app_name => crate::PROG,
            version => crate::version(),
            auth => crate::AUTH,
            auth_google => crate::AUTH_GOOGLE,
            path => self.path,
            csrf_token => self.csrf_token,
            flash => self.flash,
            user => self.user,
        }
    }

    /// Render `name` as a `200` HTML page.
    pub fn render(
        &self,
        name: &str,
        ctx: minijinja::Value,
    ) -> Result<Html<String>, Error> {
        let ctx = merge_maps([ctx, self.base()]);
        Ok(Html(self.templates.render(name, ctx)?))
    }

    /// [`Renderer::render`] with an explicit status, for re-rendering a
    /// form that failed validation as `422`.
    pub fn render_status(
        &self,
        status: StatusCode,
        name: &str,
        ctx: minijinja::Value,
    ) -> Result<Response, Error> {
        Ok((status, self.render(name, ctx)?).into_response())
    }

    /// `303 See Other` to `to`, showing `flash` on the next page. Every
    /// successful form post ends here, so a reload does not repost.
    pub async fn redirect(
        &self,
        to: &str,
        flash: Flash,
    ) -> Result<Response, Error> {
        flash::set(&self.session, flash).await?;
        Ok(Redirect::to(to).into_response())
    }
}
