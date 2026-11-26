use mw_nexus::api::{AppState, build_router};
use mw_nexus::graph::NexusGraphStore;
use mw_nexus::mtime::MTIME_PROFILE;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let bind_addr_raw = env::var("MW_NEXUS_ADDR").unwrap_or_else(|_| "127.0.0.1:4430".to_owned());
    let bind_addr: SocketAddr = bind_addr_raw.parse()?;
    let db_path = PathBuf::from(
        env::var("MW_NEXUS_DB_PATH").unwrap_or_else(|_| "mw-nexus.sqlite".to_owned()),
    );

    let store = NexusGraphStore::open(&db_path)?;
    let journal_mode = store.journal_mode()?;
    let app_state = AppState::new(store);
    let router = build_router(app_state);

    let listener = TcpListener::bind(bind_addr).await?;
    info!(
        address = %bind_addr,
        database = %db_path.display(),
        journal_mode = %journal_mode,
        mtime_profile = %MTIME_PROFILE,
        "mw-nexus server started"
    );

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sigterm) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sigterm.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
