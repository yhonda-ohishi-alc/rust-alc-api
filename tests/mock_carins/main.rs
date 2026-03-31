#[macro_use]
#[path = "../common/mod.rs"]
mod common;

extern crate alc_test_helpers as mock_helpers;

#[path = "../mock_tests/mock_car_inspection_files_test.rs"]
mod mock_car_inspection_files_test;
#[path = "../mock_tests/mock_car_inspections_test.rs"]
mod mock_car_inspections_test;
#[path = "../mock_tests/mock_carins_files_test.rs"]
mod mock_carins_files_test;
#[path = "../mock_tests/mock_nfc_tags_test.rs"]
mod mock_nfc_tags_test;
