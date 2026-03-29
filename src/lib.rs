#[cfg(test)]
#[macro_use]
mod test_macros;

pub mod auth;
pub mod compare;
pub mod csv_parser;
pub mod db;
pub mod fcm;
pub mod middleware;
pub mod routes;
pub mod storage;
pub mod webhook;

use std::sync::Arc;

use db::repository::{
    AuthRepository, CarInspectionRepository, CommunicationItemsRepository, DeviceRepository,
    EmployeeRepository, GuidanceRecordsRepository, HealthBaselinesRepository,
    MeasurementsRepository, NfcTagRepository, TenantUsersRepository, TenkoCallRepository,
    TenkoRecordsRepository, TenkoSchedulesRepository, TenkoSessionRepository,
    TenkoWebhooksRepository, TimecardRepository,
};
use storage::StorageBackend;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub auth: Arc<dyn AuthRepository>,
    pub car_inspections: Arc<dyn CarInspectionRepository>,
    pub communication_items: Arc<dyn CommunicationItemsRepository>,
    pub devices: Arc<dyn DeviceRepository>,
    pub employees: Arc<dyn EmployeeRepository>,
    pub guidance_records: Arc<dyn GuidanceRecordsRepository>,
    pub health_baselines: Arc<dyn HealthBaselinesRepository>,
    pub measurements: Arc<dyn MeasurementsRepository>,
    pub timecard: Arc<dyn TimecardRepository>,
    pub tenko_call: Arc<dyn TenkoCallRepository>,
    pub tenko_records: Arc<dyn TenkoRecordsRepository>,
    pub tenko_schedules: Arc<dyn TenkoSchedulesRepository>,
    pub tenko_sessions: Arc<dyn TenkoSessionRepository>,
    pub tenko_webhooks: Arc<dyn TenkoWebhooksRepository>,
    pub tenant_users: Arc<dyn TenantUsersRepository>,
    pub nfc_tags: Arc<dyn NfcTagRepository>,
    pub storage: Arc<dyn StorageBackend>,
    pub carins_storage: Option<Arc<dyn StorageBackend>>,
    pub dtako_storage: Option<Arc<dyn StorageBackend>>,
    pub fcm: Option<Arc<dyn fcm::FcmSenderTrait>>,
}
