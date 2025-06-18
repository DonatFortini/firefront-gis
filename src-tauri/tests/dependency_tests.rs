mod common;

use firefront_gis_lib::{app_setup, dependency};

#[test]
fn test_dependencies_check() {
    let result = dependency::check_dependencies(&mut app_setup::CONFIG.lock().unwrap());
    common::assert_result_ok(&result, "Dependency check failed");
}
