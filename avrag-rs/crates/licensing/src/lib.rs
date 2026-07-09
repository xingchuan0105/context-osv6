mod api;
mod client;
mod config;
mod fulfillment;
mod types;

pub use api::{
    CreateLicenseCheckoutRequest, CreateLicenseCheckoutResponse, LicenseCheckoutResponse,
    LicenseListResponse, LicenseMachineListResponse, handle_create_license_checkout,
    handle_create_trial_license, handle_deactivate_machine, handle_list_license_machines,
    handle_list_user_licenses,
};
pub use client::KeygenClient;
pub use config::LicensingConfig;
pub use fulfillment::fulfill_desktop_license;
pub use types::{LicenseMachine, LicenseSummary};
