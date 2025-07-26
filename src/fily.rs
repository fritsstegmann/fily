pub mod auth;
pub mod auth_middleware;
mod create_bucket;
mod create_general_bucket;
mod delete_bucket;
mod delete_object;
pub mod encryption;
pub mod etag;
mod get_object;
mod list_buckets;
pub mod metadata;
pub mod path_security;
mod put_object;
pub mod s3_app_error;
mod search_bucket;

use std::sync::Arc;

use axum::{
    routing::{delete, get, put},
    Extension, Router,
};
use serde::Deserialize;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing::info;

use auth::{AwsCredentials, AwsSignatureV4Validator};
use auth_middleware::AuthLayer;

#[derive(Deserialize, Debug)]
pub struct EncryptionConfig {
    pub enabled: bool,
    pub master_key: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AwsCredentialConfig {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
}

#[derive(Debug)]
pub struct Config {
    pub location: String,
    pub port: String,
    pub address: String,
    pub log_level: String,
    // Multiple AWS credentials support
    pub aws_credentials: Vec<AwsCredentialConfig>,
    // Encryption configuration
    pub encryption: Option<EncryptionConfig>,
}

pub async fn run(config: Config) -> anyhow::Result<()> {
    let config_state = Arc::new(config);

    let port = config_state.port.clone();
    let address = config_state.address.clone();

    // Setup AWS SigV4 authentication
    let mut validator = AwsSignatureV4Validator::new();
    let mut credentials_added = 0;

    // Add all configured AWS credentials
    for (index, aws_config) in config_state.aws_credentials.iter().enumerate() {
        match AwsCredentials::new(
            aws_config.access_key_id.clone(),
            aws_config.secret_access_key.clone(),
            aws_config.region.clone(),
        ) {
            Ok(credentials) => {
                match validator.add_credentials(aws_config.access_key_id.clone(), credentials) {
                    Ok(()) => {
                        info!(
                            "Added AWS credentials #{} for access key: {} (region: {})",
                            index + 1,
                            aws_config.access_key_id,
                            aws_config.region
                        );
                        credentials_added += 1;
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!(
                            "Failed to add AWS credentials #{}: {}",
                            index + 1,
                            e
                        ));
                    }
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Invalid AWS credentials format for credential #{}: {}",
                    index + 1,
                    e
                ));
            }
        }
    }

    if credentials_added == 0 {
        info!("No AWS credentials provided - authentication will be disabled");
    } else {
        info!("Successfully loaded {} AWS credential set(s)", credentials_added);
    }

    let auth_validator = Arc::new(validator);
    let auth_layer = AuthLayer::new(auth_validator);

    // build our application with routes
    let protected_routes = Router::new()
        .route("/", get(list_buckets::handle))
        .route("/", put(create_general_bucket::handle))
        .route("/{bucket}", put(create_bucket::handle))
        .route("/{bucket}", get(search_bucket::handle))
        .route("/{bucket}", delete(delete_bucket::handle))
        .route("/{bucket}/{file}", get(get_object::handle))
        .route("/{bucket}/{file}", put(put_object::handle))
        .route("/{bucket}/{file}", delete(delete_object::handle))
        .layer(auth_layer); // Add AWS SigV4 authentication layer

    let app = Router::new()
        .merge(protected_routes)
        .layer(Extension(config_state))
        .layer(TraceLayer::new_for_http());

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", &address, &port))
        .await
        .unwrap();

    info!("running fily server on {}:{}", &address, &port);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
