use super::paths::{self, ServerInfo};
use super::{http, ServeArgs};
use crate::control;
use crate::db;
use crate::spawn;
use crate::state::App;
use crate::transport;
use anyhow::{anyhow, Result};
use axum::extract::State;
use axum::http::{header::AUTHORIZATION, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Bind address and DB path from flags alone — relay reads no environment
/// variables; the app passes `--addr`/`--db` explicitly. The DB defaults into
/// the state dir (`--home`), resolved absolute so a daemon whose cwd is `/`
/// still lands in the right place.
fn resolve(args: &ServeArgs) -> (String, String) {
    let addr = args.addr.clone().unwrap_or_else(|| "127.0.0.1:7777".into());
    let db = args.db.clone().unwrap_or_else(|| {
        paths::abs_dir()
            .join("relay.db")
            .to_string_lossy()
            .into_owned()
    });
    (addr, db)
}

/// Run the server in the foreground (the actual daemon body).
pub async fn serve(args: ServeArgs) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("relay=info")),
        )
        .init();

    let (addr, db_path) = resolve(&args);
    paths::ensure_dir()?;
    paths::reap_stray_workers();
    let pool = db::open(&db_path).await?;
    paths::lock_file(std::path::Path::new(&db_path));
    let token = gen_token();
    let app = App::new(pool, paths::endpoint(&addr), token.clone());

    // Presence sweep: re-emit the roster exactly when the next agent's activity
    // window lapses, so the app's liveness dot (computed from `last_seen` + the
    // parked set) ages a quiet or crashed agent out (issue #9) — without a
    // blind fixed tick that keeps waking idle subscribers forever.
    {
        let app = app.clone();
        tokio::spawn(async move {
            let window = crate::state::ACTIVE_WINDOW_SECS;
            loop {
                let horizon = crate::protocol::now() - window;
                match db::next_expiry(&app.db, horizon).await {
                    Ok(Some(seen)) => {
                        // Sleep until just past that agent's expiry, then bump
                        // once: the roster flips at most one snapshot per lapse.
                        let due = seen + window + 1;
                        let wait = (due - crate::protocol::now()).max(1) as u64;
                        tokio::time::sleep(Duration::from_secs(wait)).await;
                        app.bump();
                    }
                    // Nothing inside the window (or a read hiccup): idle until
                    // it is worth re-checking; no bump, so subscribers sleep too.
                    _ => tokio::time::sleep(Duration::from_secs(window as u64)).await,
                }
            }
        });
    }

    let guarded = Router::new()
        .route("/mcp", post(transport::handle).delete(transport::end))
        .merge(control::routes())
        .layer(axum::middleware::from_fn_with_state(app.clone(), auth))
        .with_state(app.clone());
    let router = Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(guarded);

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    // Rehydrate background workers persisted by a previous daemon (issue #4).
    // The socket is bound (connections queue), so respawned workers can reach the
    // bus. The bearer token is regenerated each run, so refresh each worker's MCP
    // config in place first, then relaunch it resuming its prior claude session.
    for w in db::load_workers(&app.db).await.unwrap_or_default() {
        let _ = paths::write_mcp_config(&paths::endpoint(&addr), &w.name, &token);
        let spec = spawn::Spec {
            name: w.name,
            role: w.role,
            program: w.program,
            args: w.args,
            cwd: w.cwd,
            keep_alive: w.keep_alive,
            session_id: w.session_id,
            resume: true,
        };
        if let Err(e) = spawn::launch(&app, spec).await {
            tracing::warn!("relay: could not rehydrate worker: {e}");
        }
    }

    paths::write_info(&ServerInfo {
        pid: std::process::id(),
        addr: addr.clone(),
        db: db_path,
        token,
    })?;
    tracing::info!("relay listening on {}", paths::endpoint(&addr));

    let shutdown_app = app.clone();
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown(shutdown_app))
        .await?;
    paths::clear_info();
    Ok(())
}

/// A fresh 256-bit token as hex (two v4 UUIDs concatenated).
fn gen_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// Reject any request whose `Authorization: Bearer …` doesn't match the token.
async fn auth(State(app): State<App>, req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let presented = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    match presented {
        Some(t) if constant_time_eq(t, &app.token) => next.run(req).await,
        _ => StatusCode::UNAUTHORIZED.into_response(),
    }
}

/// Length-independent byte comparison, so a wrong token leaks no timing signal.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn shutdown(app: App) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate()).expect("sigterm");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(windows)]
    {
        // No SIGTERM on Windows; Ctrl-C (or a hard terminate) ends the daemon.
        let _ = tokio::signal::ctrl_c().await;
    }
    tracing::info!("shutting down; stopping workers");
    spawn::stop_all(&app).await;
}

/// Spawn the server as a detached background process.
pub fn start(args: ServeArgs) -> Result<()> {
    if let Ok(info) = paths::read_info() {
        if paths::alive(info.pid) {
            println!("relay already running (pid {}) on {}", info.pid, info.addr);
            return Ok(());
        }
        paths::clear_info();
    }

    let (addr, db_path) = resolve(&args);
    paths::ensure_dir()?;
    let exe = std::env::current_exe()?;
    let log = std::fs::File::create(paths::log_path())?;
    paths::lock_file(&paths::log_path());
    let errlog = log.try_clone()?;

    let mut cmd = Command::new(exe);
    cmd.arg("--home")
        .arg(paths::dir())
        .arg("serve")
        .arg("--addr")
        .arg(&addr)
        .arg("--db")
        .arg(&db_path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(errlog));
    // Detach into a new session/group so the daemon survives the launching
    // shell exiting.
    #[cfg(unix)]
    unsafe {
        use std::os::unix::process::CommandExt;
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW.
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    cmd.spawn()?;

    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(75));
        if http::get(&addr, "/health").is_ok() {
            println!("relay started on {}", paths::endpoint(&addr));
            return Ok(());
        }
    }
    Err(anyhow!(
        "server did not come up — see {}",
        paths::log_path().display()
    ))
}

pub fn stop() -> Result<()> {
    let info = paths::read_info()?;
    if !paths::alive(info.pid) {
        paths::clear_info();
        println!("relay was not running (cleaned stale record)");
        return Ok(());
    }
    crate::proc::terminate(info.pid);
    for _ in 0..40 {
        if !paths::alive(info.pid) {
            paths::clear_info();
            println!("relay stopped");
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(75));
    }
    Err(anyhow!("server (pid {}) did not stop", info.pid))
}

pub fn restart(args: ServeArgs) -> Result<()> {
    let _ = stop();
    start(args)
}

pub fn status() -> Result<()> {
    match paths::read_info() {
        Ok(info) if paths::alive(info.pid) => {
            let health = http::get(&info.addr, "/health").is_ok();
            println!(
                "running · pid {} · {} · health {}",
                info.pid,
                paths::endpoint(&info.addr),
                if health { "ok" } else { "unreachable" }
            );
        }
        Ok(_) => println!("not running (stale record present)"),
        Err(_) => println!("not running"),
    }
    Ok(())
}
