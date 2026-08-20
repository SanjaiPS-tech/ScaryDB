// TLS and Authentication module

use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::RwLock;
use tracing::{error, info, warn};

/// TLS configuration holder
pub struct TlsConfig {
    pub acceptor: Option<TlsAcceptor>,
    pub enabled: bool,
}

impl TlsConfig {
    /// Create TLS config from settings
    pub fn from_settings(settings: &crate::config::TlsSettings) -> Result<Self, String> {
        if !settings.enabled {
            return Ok(Self { acceptor: None, enabled: false });
        }

        // Load certificate chain
        let cert_file = File::open(&settings.cert_file)
            .map_err(|e| format!("Failed to open cert file {}: {}", settings.cert_file, e))?;
        let mut cert_reader = BufReader::new(cert_file);
        let cert_chain = certs(&mut cert_reader)
            .map_err(|e| format!("Failed to parse cert file: {}", e))?
            .into_iter()
            .map(CertificateDer::from)
            .collect::<Vec<_>>();

        if cert_chain.is_empty() {
            return Err("No certificates found in cert file".to_string());
        }

        // Load private key
        let key_file = File::open(&settings.key_file)
            .map_err(|e| format!("Failed to open key file {}: {}", settings.key_file, e))?;
        let mut key_reader = BufReader::new(key_file);
        let keys = pkcs8_private_keys(&mut key_reader)
            .map_err(|e| format!("Failed to parse key file: {}", e))?;

        if keys.is_empty() {
            return Err("No private keys found in key file".to_string());
        }

        // Store the raw key bytes for cloning
        let key_bytes = keys[0].clone();

        // Build server config
        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                cert_chain.clone(),
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_bytes.clone()))
            )
            .map_err(|e| format!("Failed to build TLS config: {}", e))?;

        // If client cert required, configure client auth
        if let Some(ca_file) = &settings.ca_file {
            if settings.require_client_cert {
                let ca_file = File::open(ca_file)
                    .map_err(|e| format!("Failed to open CA file {}: {}", ca_file, e))?;
                let mut ca_reader = BufReader::new(ca_file);
                let ca_certs = certs(&mut ca_reader)
                    .map_err(|e| format!("Failed to parse CA file: {}", e))?
                    .into_iter()
                    .map(CertificateDer::from)
                    .collect::<Vec<_>>();

                let mut root_store = rustls::RootCertStore::empty();
                for cert in ca_certs {
                    root_store.add(cert).map_err(|e| format!("Failed to add CA cert: {}", e))?;
                }

                // Use WebPkiClientVerifier builder for client cert verification
                let verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(root_store))
                    .build()
                    .map_err(|e| format!("Failed to build client cert verifier: {}", e))?;

                // verifier is already Arc<dyn ClientCertVerifier> from build()
                let config = ServerConfig::builder()
                    .with_client_cert_verifier(verifier)
                    .with_single_cert(cert_chain, PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_bytes.clone())))
                    .map_err(|e| format!("Failed to build TLS config with client auth: {}", e))?;
            }
        }

        let acceptor = TlsAcceptor::from(Arc::new(config));
        
        Ok(Self {
            acceptor: Some(acceptor),
            enabled: true,
        })
    }
}

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,              // subject (api key or user id)
    pub exp: usize,               // expiration timestamp
    pub iat: usize,               // issued at timestamp
    pub permissions: Vec<String>, // permissions/roles
}

/// Authentication manager
pub struct AuthManager {
    jwt_secret: String,
    token_expiry_hours: u64,
    api_keys: RwLock<HashSet<String>>,
    enabled: bool,
}

impl AuthManager {
    pub fn new(settings: &crate::config::AuthSettings) -> Self {
        let mut keys = HashSet::new();
        for key in &settings.api_keys {
            keys.insert(key.clone());
        }
        
        Self {
            jwt_secret: settings.jwt_secret.clone(),
            token_expiry_hours: settings.token_expiry_hours,
            api_keys: RwLock::new(keys),
            enabled: settings.enabled,
        }
    }

    /// Check if authentication is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Validate an API key
    pub fn validate_api_key(&self, api_key: &str) -> bool {
        if !self.enabled {
            return true; // Auth disabled, allow all
        }
        self.api_keys.read().unwrap().contains(api_key)
    }

    /// Add an API key
    pub fn add_api_key(&self, api_key: String) {
        self.api_keys.write().unwrap().insert(api_key);
    }

    /// Remove an API key
    pub fn remove_api_key(&self, api_key: &str) {
        self.api_keys.write().unwrap().remove(api_key);
    }

    /// List all API keys
    pub fn list_api_keys(&self) -> Vec<String> {
        self.api_keys.read().unwrap().iter().cloned().collect()
    }

    /// Generate a JWT token for an API key
    pub fn generate_token(
        &self,
        api_key: &str,
        permissions: Vec<String>,
    ) -> Result<String, String> {
        let now = chrono::Utc::now().timestamp() as usize;
        let exp = now + (self.token_expiry_hours as usize * 3600);

        let claims = Claims {
            sub: api_key.to_string(),
            exp,
            iat: now,
            permissions,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_ref()),
        )
        .map_err(|e| format!("Failed to generate token: {}", e))
    }

    /// Validate a JWT token
    pub fn validate_token(&self, token: &str) -> Result<TokenData<Claims>, String> {
        if !self.enabled {
            return Err("Authentication disabled".to_string());
        }

        let validation = Validation::default();
        decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_ref()),
            &validation,
        )
        .map_err(|e| format!("Invalid token: {}", e))
    }
}

/// Connection context with auth info
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub authenticated: bool,
    pub api_key: Option<String>,
    pub permissions: Vec<String>,
}

impl Default for AuthContext {
    fn default() -> Self {
        Self {
            authenticated: false,
            api_key: None,
            permissions: vec![],
        }
    }
}