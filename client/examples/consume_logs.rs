use std::env;
use std::error::Error;
use std::io::{Error as IoError, ErrorKind};

use aliyun_log_rust_sdk::consumer::{
    ConsumerConfig, ConsumerWorker, CursorPosition, ProcessFn, ProcessOutcome,
};
use aliyun_log_rust_sdk::{Client, Config, FromConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let endpoint = required_env("SLS_ENDPOINT")?;
    let access_key_id = required_env("ALIBABA_CLOUD_ACCESS_KEY_ID")?;
    let access_key_secret = required_env("ALIBABA_CLOUD_ACCESS_KEY_SECRET")?;
    let project = required_env("SLS_PROJECT")?;
    let logstore = required_env("SLS_LOGSTORE")?;
    let consumer_group = required_env("SLS_CONSUMER_GROUP")?;
    let consumer_name = env::var("SLS_CONSUMER_NAME")
        .or_else(|_| env::var("POD_NAME"))
        .map_err(|_| {
            IoError::new(
                ErrorKind::InvalidInput,
                "SLS_CONSUMER_NAME or POD_NAME must be set",
            )
        })?;

    let config_builder = Config::builder().endpoint(endpoint);
    let config_builder = match env::var("ALIBABA_CLOUD_SECURITY_TOKEN") {
        Ok(token) if !token.is_empty() => {
            config_builder.sts(access_key_id, access_key_secret, token)
        }
        _ => config_builder.access_key(access_key_id, access_key_secret),
    };
    let client = Client::from_config(config_builder.build()?)?;

    let consumer_config = ConsumerConfig::new(project, logstore, consumer_group, consumer_name)
        // This value is only used when the consumer group has no saved checkpoint.
        .cursor_position(CursorPosition::Begin);

    let processor = ProcessFn::new(|shard_id, log_groups, checkpoint| async move {
        for log_group in log_groups.iter() {
            for log in log_group.logs() {
                let contents = log
                    .contents()
                    .iter()
                    .map(|content| format!("{}={:?}", content.key(), content.value()))
                    .collect::<Vec<_>>()
                    .join(" ");

                println!(
                    "shard={shard_id} time={} time_ns={:?} topic={:?} source={:?} {contents}",
                    log.time(),
                    log.time_ns(),
                    log_group.topic(),
                    log_group.source(),
                );
            }
        }

        // Mark this batch's next cursor after every log has been processed.
        // `false` lets the worker periodically flush the checkpoint to SLS.
        checkpoint.save_checkpoint(false).await?;
        Ok::<_, aliyun_log_rust_sdk::consumer::Error>(ProcessOutcome::Continue)
    });

    let mut worker = ConsumerWorker::new(client, consumer_config, processor)?;
    worker.start().await?;
    println!("consumer started; press Ctrl-C to stop");

    shutdown_signal().await?;
    worker.stop_and_wait().await?;
    Ok(())
}

fn required_env(name: &str) -> Result<String, IoError> {
    env::var(name).map_err(|_| {
        IoError::new(
            ErrorKind::InvalidInput,
            format!("required environment variable {name} is not set"),
        )
    })
}

#[cfg(unix)]
async fn shutdown_signal() -> Result<(), IoError> {
    use tokio::signal::unix::{signal, SignalKind};

    let mut terminate = signal(SignalKind::terminate())?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => result?,
        _ = terminate.recv() => {},
    }
    Ok(())
}

#[cfg(not(unix))]
async fn shutdown_signal() -> Result<(), IoError> {
    tokio::signal::ctrl_c().await
}
