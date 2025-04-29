use dotenv::dotenv;
use lazy_static::lazy_static;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub(crate) struct TestEnvironment {
    pub(crate) access_key_id: String,
    pub(crate) access_key_secret: String,
    pub(crate) endpoint: String,
    pub(crate) project: String,
    pub(crate) logstore: String,
}

#[cfg(test)]
lazy_static! {
    pub(crate) static ref TEST_ENV: TestEnvironment = {
        dotenv().ok();
        envy::from_env::<TestEnvironment>()
            .expect("Error loading configuration from environment during tests")
    };
}
