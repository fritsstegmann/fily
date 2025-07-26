pub mod auth;
pub mod auth_middleware;
mod create_bucket;
mod create_general_bucket;
mod delete_bucket;
mod delete_object;
pub mod encryption;
pub mod etag;
mod generate_presigned_url;
mod get_object;
mod list_buckets;
pub mod metadata;
mod put_object;
pub mod s3_app_error;
mod search_bucket;

use std::sync::Arc;

use axum::{
    routing::{delete, get, post, put},
    Extension, Router,
};
use serde::Deserialize;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing::info;

use auth::{AwsCredentials, AwsSignatureV4Validator};
use auth_middleware::AuthLayer;

#[derive(Deserialize)]
pub struct EncryptionConfig {
    pub enabled: bool,
    pub master_key: Option<String>,
}

#[derive(Deserialize)]
pub struct Config {
    location: String,
    port: String,
    address: String,
    // AWS credentials configuration
    aws_access_key_id: Option<String>,
    aws_secret_access_key: Option<String>,
    aws_region: Option<String>,
    // Encryption configuration
    pub encryption: Option<EncryptionConfig>,
}

pub async fn run(config: Config) -> anyhow::Result<()> {
    let config_state = Arc::new(config);

    let port = config_state.port.clone();
    let address = config_state.address.clone();

    // Setup AWS SigV4 authentication
    let mut validator = AwsSignatureV4Validator::new();

    // Add default credentials if provided in config
    if let (Some(access_key), Some(secret_key), Some(region)) = (
        &config_state.aws_access_key_id,
        &config_state.aws_secret_access_key,
        &config_state.aws_region,
    ) {
        match AwsCredentials::new(access_key.clone(), secret_key.clone(), region.clone()) {
            Ok(credentials) => {
                match validator.add_credentials(access_key.clone(), credentials) {
                    Ok(()) => {
                        info!("Added validated AWS credentials for access key: {}", access_key);
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!("Failed to add AWS credentials: {}", e));
                    }
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Invalid AWS credentials format: {}", e));
            }
        }
    } else {
        info!("No AWS credentials provided in config - authentication will be disabled");
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

    // Route for generating pre-signed URLs (no auth layer needed for generation)
    let presigned_routes = Router::new().route(
        "/_presign/{bucket}/{file}",
        post(generate_presigned_url::handle),
    );

    let app = Router::new()
        .merge(protected_routes)
        .merge(presigned_routes)
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
