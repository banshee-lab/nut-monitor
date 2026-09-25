use std::sync::Arc;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use crate::AppState;
use crate::metrics::{fetch_ups_metrics, UpsMetrics};
use crate::alerts::{get_registered_tokens, send_fcm_v1_notification};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct RegisterDeviceRequest {
    pub device_token: String,
    pub device_name: String,
    pub device_id: String,
}

pub async fn register_device_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegisterDeviceRequest>,
) -> StatusCode {
    info!("Registration attempt received for device ID: {}", payload.device_id);
    let db = state.db_conn.lock().unwrap();
    
    let res = db.execute(
        "INSERT OR REPLACE INTO devices (device_id, device_name, device_token) VALUES (?1, ?2, ?3)",
        rusqlite::params![payload.device_id, payload.device_name, payload.device_token],
    );

    match res {
        Ok(_) => {
            info!("Successfully registered/updated device: {}", payload.device_name);
            StatusCode::OK
        }
        Err(e) => {
            error!("Database write failure during device registration: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub async fn get_devices_handler(State(state): State<Arc<AppState>>) -> (StatusCode, Json<Vec<RegisterDeviceRequest>>) {
    let db = state.db_conn.lock().unwrap();
    let mut stmt = db.prepare("SELECT device_id, device_name, device_token FROM devices").unwrap();
    
    let device_iter = stmt.query_map([], |row| {
        Ok(RegisterDeviceRequest {
            device_id: row.get(0)?,
            device_name: row.get(1)?,
            device_token: row.get(2)?,
        })
    }).unwrap();

    let devices: Vec<RegisterDeviceRequest> = device_iter.filter_map(|d| d.ok()).collect();
    (StatusCode::OK, Json(devices))
}

pub async fn delete_device_handler(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> StatusCode {
    info!("Request to delete device received for ID: {}", id);
    let db = state.db_conn.lock().unwrap();
    
    match db.execute("DELETE FROM devices WHERE device_id = ?1", rusqlite::params![id]) {
        Ok(rows) if rows > 0 => {
            info!("Successfully removed device record matching ID: {}", id);
            StatusCode::OK
        }
        Ok(_) => {
            warn!("No device match found for ID deletion candidate: {}", id);
            StatusCode::NOT_FOUND
        }
        Err(e) => {
            error!("Failed to complete transactional device delete execution sequence: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub async fn test_fcm_handler(State(state): State<Arc<AppState>>) -> (StatusCode, Json<serde_json::Value>) {
    info!("Manual FCM test triggered via /api/test-fcm");
    let config = match &state.fcm_config {
        Some(c) => c,
        None => {
            warn!("FCM test skipped: Missing configurations");
            return (
                StatusCode::BAD_REQUEST, 
                Json(serde_json::json!({ "error": "FCM configurations are missing from environment vars" }))
            );
        }
    };

    let tokens = get_registered_tokens(&state);
    if tokens.is_empty() {
        info!("FCM test run canceled: 0 devices found in database.");
        return (
            StatusCode::OK,
            Json(serde_json::json!({ "message": "FCM is configured but no devices are registered in database yet." }))
        );
    }

    let title = "⚠️ Test Notification";
    let message = "This is a test notification generated from your NUT Monitor endpoint.";

    let mut successful_sends = 0;
    for token in &tokens {
        if send_fcm_v1_notification(config, token, title, message).await.is_ok() {
            successful_sends += 1;
        }
    }

    info!("FCM Test broadcast completed. Sent {} successfully out of {} targets.", successful_sends, tokens.len());

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "Test completed",
            "targets_found": tokens.len(),
            "successfully_sent": successful_sends
        }))
    )
}

/// Returns the most recent history entry where the UPS switched to battery (OB/LB).
/// An "incident" is defined as a transition into a battery-powered state.
pub fn get_last_incident(state: &AppState) -> Option<StatusHistoryEntry> {
    let db = state.db_conn.lock().unwrap();
    let mut stmt = db.prepare(
        "SELECT id, status, description, changed_at FROM status_history \
         WHERE status LIKE 'OB%' OR status LIKE 'LB%' \
         ORDER BY id DESC LIMIT 1"
    ).ok()?;
    let mut rows = stmt.query([]).ok()?;
    if let Some(row) = rows.next().ok()? {
        Some(StatusHistoryEntry {
            id: row.get(0).ok()?,
            status: row.get(1).ok()?,
            description: row.get(2).ok()?,
            changed_at: row.get(3).ok()?,
        })
    } else {
        None
    }
}

/// Response shape for GET /api/status — live metrics enriched with the last recorded incident.
#[derive(Serialize, Clone, Debug)]
pub struct StatusResponse {
    #[serde(flatten)]
    pub metrics: UpsMetrics,
    pub last_incident: Option<StatusHistoryEntry>,
}

pub async fn json_handler(State(state): State<Arc<AppState>>) -> Json<StatusResponse> {
    Json(StatusResponse {
        metrics: fetch_ups_metrics(&state),
        last_incident: get_last_incident(&state),
    })
}

#[derive(Serialize, Clone, Debug)]
pub struct StatusHistoryEntry {
    pub id: i64,
    pub status: String,
    pub description: String,
    pub changed_at: String,
}

/// Reads the latest `limit` status history entries from the DB, most recent first.
pub fn get_status_history(state: &AppState, limit: usize) -> Vec<StatusHistoryEntry> {
    let db = state.db_conn.lock().unwrap();
    let mut stmt = db.prepare(
        "SELECT id, status, description, changed_at FROM status_history ORDER BY id DESC LIMIT ?1"
    ).unwrap();
    let iter = stmt.query_map(rusqlite::params![limit as i64], |row| {
        Ok(StatusHistoryEntry {
            id: row.get(0)?,
            status: row.get(1)?,
            description: row.get(2)?,
            changed_at: row.get(3)?,
        })
    }).unwrap();
    iter.filter_map(|e| e.ok()).collect()
}

pub async fn status_history_handler(State(state): State<Arc<AppState>>) -> (StatusCode, Json<Vec<StatusHistoryEntry>>) {
    let entries = get_status_history(&state, 5);
    (StatusCode::OK, Json(entries))
}
