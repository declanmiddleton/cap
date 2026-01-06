use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::core::{
    audit::AuditLogger,
    config::Config,
    session::{Session, SessionManager},
};
use crate::modules::ModuleExecutor;

#[derive(Clone)]
struct AppState {
    config: Config,
    session_manager: SessionManager,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    uptime: String,
}

#[derive(Serialize)]
struct SessionsResponse {
    sessions: Vec<Session>,
}

#[derive(Deserialize)]
struct CreateSessionRequest {
    name: String,
    purpose: Option<String>,
    authorization_ref: Option<String>,
}

#[derive(Serialize)]
struct SessionResponse {
    session: Session,
}

#[derive(Deserialize)]
struct ExecuteModuleRequest {
    module: String,
    target: String,
    wordlist: Option<String>,
    threads: Option<usize>,
}

#[derive(Serialize)]
struct ModuleResultsResponse {
    results: Vec<String>,
    count: usize,
}

#[derive(Serialize)]
struct ScopeResponse {
    targets: Vec<String>,
}

#[derive(Deserialize)]
struct AddScopeRequest {
    target: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

pub async fn start_listener(
    host: String,
    port: u16,
    config: Config,
    session_manager: SessionManager,
) -> Result<()> {
    let state = AppState {
        config: config.clone(),
        session_manager,
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/:id", get(get_session).delete(terminate_session))
        .route("/api/modules/execute", post(execute_module))
        .route("/api/scope", get(list_scope).post(add_scope))
        .route("/api/scope/:target", axum::routing::delete(remove_scope))
        .route("/api/audit", get(get_audit_logs))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(Arc::new(state));

    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("🚀 CAP listener started on {}", addr);
    tracing::info!("📊 API endpoints:");
    tracing::info!("   GET  /health");
    tracing::info!("   GET  /api/sessions");
    tracing::info!("   POST /api/sessions");
    tracing::info!("   POST /api/modules/execute");
    tracing::info!("   GET  /api/scope");
    tracing::info!("   GET  /api/audit");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime: "running".to_string(),
    })
}

async fn list_sessions(
    State(state): State<Arc<AppState>>,
) -> Json<SessionsResponse> {
    let sessions = state.session_manager.list_sessions().await;
    Json(SessionsResponse { sessions })
}

async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<SessionResponse>, (StatusCode, Json<ErrorResponse>)> {
    match state.session_manager.get_session(&id).await {
        Some(session) => Ok(Json(SessionResponse { session })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Session not found: {}", id),
            }),
        )),
    }
}

async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<SessionResponse>, (StatusCode, Json<ErrorResponse>)> {
    match state.session_manager.create_session(req.name).await {
        Ok(mut session) => {
            if req.purpose.is_some() || req.authorization_ref.is_some() {
                let _ = state
                    .session_manager
                    .update_metadata(
                        &session.id,
                        req.purpose.clone(),
                        req.authorization_ref.clone(),
                        None,
                    )
                    .await;
                
                if let Some(purpose) = req.purpose {
                    session.metadata.purpose = purpose;
                }
                if let Some(auth_ref) = req.authorization_ref {
                    session.metadata.authorization_ref = Some(auth_ref);
                }
            }

            Ok(Json(SessionResponse { session }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

async fn terminate_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    match state.session_manager.terminate_session(&id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

async fn execute_module(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExecuteModuleRequest>,
) -> Result<Json<ModuleResultsResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check scope
    if !state.config.scope.is_in_scope(&req.target) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: format!("Target '{}' is not in authorized scope", req.target),
            }),
        ));
    }

    let executor = ModuleExecutor::new(
        state.config.clone(),
        state.session_manager.clone(),
    );

    match executor
        .execute(
            &req.module,
            &req.target,
            req.wordlist,
            req.threads.unwrap_or(10),
        )
        .await
    {
        Ok(results) => {
            let count = results.len();
            Ok(Json(ModuleResultsResponse { results, count }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

async fn list_scope(
    State(state): State<Arc<AppState>>,
) -> Json<ScopeResponse> {
    let targets = state.config.scope.list_targets();
    Json(ScopeResponse { targets })
}

async fn add_scope(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddScopeRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    match state.config.scope.add_target(&req.target) {
        Ok(_) => Ok(StatusCode::CREATED),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

async fn remove_scope(
    State(state): State<Arc<AppState>>,
    Path(target): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    match state.config.scope.remove_target(&target) {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

#[derive(Deserialize)]
struct AuditQuery {
    session_id: Option<String>,
}

async fn get_audit_logs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuditQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let audit_logger = match AuditLogger::new(&state.config.audit.log_path) {
        Ok(logger) => logger,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            ))
        }
    };

    match audit_logger.read_logs(query.session_id.as_deref()) {
        Ok(logs) => Ok(Json(serde_json::json!({ "logs": logs, "count": logs.len() }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

