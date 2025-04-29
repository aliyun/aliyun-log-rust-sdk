use dotenv::dotenv;
use lazy_static::lazy_static;
use serde::Deserialize;

#[cfg(test)]
#[derive(Deserialize, Debug)]
pub struct TestEnvironment {
    pub access_key_id: String,
    pub access_key_secret: String,
    pub endpoint: String,
    pub project: String,
    pub logstore: String,
}

#[cfg(test)]
lazy_static! {
    pub static ref TEST_ENV: TestEnvironment = {
        dotenv().ok();
        envy::from_env::<TestEnvironment>()
            .expect("Error loading configuration from environment during tests")
    };
}
