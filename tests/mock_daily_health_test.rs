mod common;
mod mock_helpers;

use mock_helpers::app_state::setup_mock_app_state;

#[tokio::test]
async fn mock_daily_health_smoke() {
    let _state = setup_mock_app_state().await;
}
