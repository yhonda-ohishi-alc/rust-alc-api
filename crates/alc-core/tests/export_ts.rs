#![cfg(feature = "ts-export")]

use std::path::PathBuf;
use ts_rs::TS;

/// Export all ts-rs annotated types to the alc-app frontend types directory.
///
/// Run: cargo test -p alc-core --test export_ts -- --nocapture
#[test]
fn export_bindings() {
    // シンボリックリンク alc-app → ~/js/alc-app を考慮して絶対パスで指定
    let out_dir = PathBuf::from(
        option_env!("TS_EXPORT_DIR").unwrap_or("/home/yhonda/js/alc-app/web/app/types/generated"),
    );

    std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");

    // Export all types — ts-rs export_all! requires listing them
    macro_rules! export {
        ($($t:ty),* $(,)?) => {
            $(
                <$t>::export_all_to(&out_dir).unwrap_or_else(|e| {
                    panic!("Failed to export {}: {}", stringify!($t), e);
                });
            )*
        };
    }

    use alc_core::models::*;

    export!(
        // DB models (Serialize)
        Tenant,
        TenantAllowedEmail,
        Employee,
        FaceDataEntry,
        User,
        Measurement,
        MeasurementsResponse,
        TenkoSchedule,
        TenkoSchedulesResponse,
        TenkoSession,
        TenkoSessionsResponse,
        TenkoRecord,
        TenkoRecordsResponse,
        WebhookConfig,
        WebhookDelivery,
        TenkoDashboard,
        TimecardCard,
        TimePunch,
        TimePunchWithEmployee,
        TimePunchWithDevice,
        TimePunchesResponse,
        EmployeeHealthBaseline,
        SelfDeclaration,
        SafetyJudgment,
        MedicalDiffs,
        EquipmentFailure,
        EquipmentFailuresResponse,
        DtakoOffice,
        DtakoVehicle,
        DtakoEventClassification,
        DtakoOperation,
        DtakoOperationListItem,
        DtakoOperationsResponse,
        DtakoUploadHistory,
        DtakoDailyWorkHours,
        DtakoDailyHoursResponse,
        DtakoDailyWorkSegment,
        DtakoSegmentsResponse,
        NfcTag,
        CarryingItem,
        CarryingItemVehicleCondition,
        VehicleConditionInput,
        CarryingItemCheck,
        GuidanceRecord,
        GuidanceRecordAttachment,
        CommunicationItem,
        // Request DTOs (Deserialize)
        CreateEmployee,
        UpdateFace,
        UpdateNfcId,
        UpdateEmployee,
        UpdateLicense,
        CreateMeasurement,
        StartMeasurement,
        UpdateMeasurement,
        MeasurementFilter,
        CreateTenkoSchedule,
        BatchCreateTenkoSchedules,
        UpdateTenkoSchedule,
        TenkoScheduleFilter,
        StartTenkoSession,
        SubmitAlcoholResult,
        SubmitMedicalData,
        SubmitOperationReport,
        CancelTenkoSession,
        TenkoSessionFilter,
        TenkoRecordFilter,
        CreateWebhookConfig,
        CreateTimecardCard,
        CreateTimePunchByCard,
        TimePunchFilter,
        CreateHealthBaseline,
        UpdateHealthBaseline,
        SubmitSelfDeclaration,
        SubmitDailyInspection,
        InterruptSession,
        ResumeSession,
        CreateEquipmentFailure,
        UpdateEquipmentFailure,
        EquipmentFailureFilter,
        UpdateDtakoClassification,
        DtakoOperationFilter,
        DtakoDailyHoursFilter,
        CreateCarryingItem,
        UpdateCarryingItem,
        SubmitCarryingItemCheck,
        SubmitCarryingItemChecks,
        CreateGuidanceRecord,
        UpdateGuidanceRecord,
        CreateCommunicationItem,
        UpdateCommunicationItem,
    );

    println!("Types exported to {}", out_dir.display());
}
