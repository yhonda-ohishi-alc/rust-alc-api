use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// --- Tenant ---

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

// --- Employee ---

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Employee {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub code: Option<String>,
    pub nfc_id: Option<String>,
    pub name: String,
    pub face_photo_url: Option<String>,
    #[serde(skip_serializing)]
    pub face_embedding: Option<Vec<f64>>,
    pub face_embedding_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateEmployee {
    pub code: Option<String>,
    pub nfc_id: Option<String>,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFace {
    pub face_photo_url: Option<String>,
    pub face_embedding: Option<Vec<f64>>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct FaceDataEntry {
    pub id: Uuid,
    pub face_embedding: Option<Vec<f64>>,
    pub face_embedding_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNfcId {
    pub nfc_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEmployee {
    pub name: String,
    pub code: Option<String>,
}

// --- User ---

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub google_sub: String,
    pub email: String,
    pub name: String,
    pub role: String,
    pub refresh_token_hash: Option<String>,
    pub refresh_token_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

// --- Measurement ---

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Measurement {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub employee_id: Uuid,
    #[serde(rename = "alcohol_value")]
    pub alcohol_level: f64,
    #[serde(rename = "result_type")]
    pub result: String,
    pub device_use_count: i32,
    pub face_photo_url: Option<String>,
    pub measured_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    // Medical data (BLE Medical Gateway)
    pub temperature: Option<f64>,
    pub systolic: Option<i32>,
    pub diastolic: Option<i32>,
    pub pulse: Option<i32>,
    pub medical_measured_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMeasurement {
    pub employee_id: Uuid,
    #[serde(alias = "alcohol_level")]
    pub alcohol_value: f64,
    #[serde(alias = "result")]
    pub result_type: String,
    pub face_photo_url: Option<String>,
    pub measured_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub device_use_count: Option<i32>,
    // Medical data (BLE Medical Gateway)
    pub temperature: Option<f64>,
    pub systolic: Option<i32>,
    pub diastolic: Option<i32>,
    pub pulse: Option<i32>,
    pub medical_measured_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct MeasurementFilter {
    pub employee_id: Option<Uuid>,
    #[serde(alias = "result")]
    pub result_type: Option<String>,
    #[serde(alias = "from")]
    pub date_from: Option<DateTime<Utc>>,
    #[serde(alias = "to")]
    pub date_to: Option<DateTime<Utc>>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct MeasurementsResponse {
    pub measurements: Vec<Measurement>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}
