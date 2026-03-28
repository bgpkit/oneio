use crate::OneIoError;
#[cfg(feature = "http")]
use reqwest::blocking::Client;
#[cfg(feature = "http")]
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_LENGTH, USER_AGENT};
#[cfg(all(feature = "http", any(feature = "rustls", feature = "native-tls")))]
use reqwest::Certificate;
use std::time::Duration;

/// Builder for [`OneIo`], modeled after reqwest's client builder API.
pub struct OneIoBuilder {
    #[cfg(feature = "http")]
    http_client_builder: reqwest::blocking::ClientBuilder,
    #[cfg(feature = "http")]
    default_headers: HeaderMap,
}

impl Default for OneIoBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl OneIoBuilder {
    /// Creates a new [`OneIoBuilder`] with oneio's default HTTP behavior.
    pub fn new() -> Self {
        #[cfg(feature = "http")]
        let mut http_client_builder = Client::builder();

        #[cfg(all(feature = "http", any(feature = "rustls", feature = "native-tls")))]
        {
            http_client_builder =
                http_client_builder.danger_accept_invalid_certs(accept_invalid_certs_from_env());

            // Load ONEIO_CA_BUNDLE if set
            if let Ok(ca_bundle_path) = std::env::var("ONEIO_CA_BUNDLE") {
                if let Ok(pem) = std::fs::read(&ca_bundle_path) {
                    if let Ok(cert) = Certificate::from_pem(&pem) {
                        http_client_builder = http_client_builder.add_root_certificate(cert);
                    }
                }
            }
        }

        Self {
            #[cfg(feature = "http")]
            http_client_builder,
            #[cfg(feature = "http")]
            default_headers: default_http_headers(),
        }
    }

    /// Merges a set of default headers into this builder.
    #[cfg(feature = "http")]
    pub fn default_headers(mut self, headers: HeaderMap) -> Self {
        for (name, value) in headers.iter() {
            self.default_headers.insert(name.clone(), value.clone());
        }
        self
    }

    /// Adds or replaces a single default header for every HTTP request.
    #[cfg(feature = "http")]
    pub fn header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.default_headers.insert(name, value);
        self
    }

    /// Convenience method for string-based headers.
    /// Panics on invalid header name or value (same convention as reqwest).
    #[cfg(feature = "http")]
    pub fn header_str(mut self, name: &str, value: &str) -> Self {
        let name = HeaderName::from_bytes(name.as_bytes()).expect("invalid header name");
        let value = HeaderValue::from_str(value).expect("invalid header value");
        self.default_headers.insert(name, value);
        self
    }

    /// Overrides the default `User-Agent` header.
    #[cfg(feature = "http")]
    pub fn user_agent(mut self, value: HeaderValue) -> Self {
        self.default_headers.insert(USER_AGENT, value);
        self
    }

    /// Adds an additional trusted root certificate for HTTPS requests.
    #[cfg(all(feature = "http", any(feature = "rustls", feature = "native-tls")))]
    pub fn add_root_certificate(mut self, cert: Certificate) -> Self {
        self.http_client_builder = self.http_client_builder.add_root_certificate(cert);
        self
    }

    /// Adds an additional trusted PEM-encoded root certificate.
    #[cfg(all(feature = "http", any(feature = "rustls", feature = "native-tls")))]
    pub fn add_root_certificate_pem(self, pem: &[u8]) -> Result<Self, OneIoError> {
        let cert = Certificate::from_pem(pem)
            .map_err(|e| OneIoError::InvalidCertificate(e.to_string()))?;
        Ok(self.add_root_certificate(cert))
    }

    /// Adds an additional trusted DER-encoded root certificate.
    #[cfg(all(feature = "http", any(feature = "rustls", feature = "native-tls")))]
    pub fn add_root_certificate_der(self, der: &[u8]) -> Result<Self, OneIoError> {
        let cert = Certificate::from_der(der)
            .map_err(|e| OneIoError::InvalidCertificate(e.to_string()))?;
        Ok(self.add_root_certificate(cert))
    }

    /// Configures whether invalid HTTPS certificates should be accepted.
    #[cfg(all(feature = "http", any(feature = "rustls", feature = "native-tls")))]
    pub fn danger_accept_invalid_certs(mut self, accept_invalid_certs: bool) -> Self {
        self.http_client_builder = self
            .http_client_builder
            .danger_accept_invalid_certs(accept_invalid_certs);
        self
    }

    /// Escape hatch for configuring the underlying reqwest client builder.
    #[cfg(feature = "http")]
    pub fn configure_http<F>(mut self, f: F) -> Self
    where
        F: FnOnce(reqwest::blocking::ClientBuilder) -> reqwest::blocking::ClientBuilder,
    {
        self.http_client_builder = f(self.http_client_builder);
        self
    }

    /// Sets a timeout for the entire request.
    #[cfg(feature = "http")]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.http_client_builder = self.http_client_builder.timeout(timeout);
        self
    }

    /// Sets a timeout for connecting to a host.
    #[cfg(feature = "http")]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.http_client_builder = self.http_client_builder.connect_timeout(timeout);
        self
    }

    /// Sets a proxy for all HTTP requests.
    #[cfg(feature = "http")]
    pub fn proxy(mut self, proxy: reqwest::Proxy) -> Self {
        self.http_client_builder = self.http_client_builder.proxy(proxy);
        self
    }

    /// Disables proxy for all HTTP requests.
    #[cfg(feature = "http")]
    pub fn no_proxy(mut self) -> Self {
        self.http_client_builder = self.http_client_builder.no_proxy();
        self
    }

    /// Sets the redirect policy.
    #[cfg(feature = "http")]
    pub fn redirect(mut self, policy: reqwest::redirect::Policy) -> Self {
        self.http_client_builder = self.http_client_builder.redirect(policy);
        self
    }

    /// Builds a reusable [`OneIo`] instance.
    pub fn build(self) -> Result<crate::client::OneIo, OneIoError> {
        dotenvy::dotenv().ok();

        #[cfg(feature = "rustls")]
        crate::crypto::ensure_default_provider()?;

        Ok(crate::client::OneIo {
            #[cfg(feature = "http")]
            http_client: self
                .http_client_builder
                .default_headers(self.default_headers)
                .build()?,
        })
    }
}

#[cfg(feature = "http")]
fn default_http_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("oneio"));
    headers.insert(CONTENT_LENGTH, HeaderValue::from_static("0"));
    #[cfg(feature = "cli")]
    headers.insert(
        reqwest::header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache"),
    );
    headers
}

#[cfg(all(feature = "http", any(feature = "rustls", feature = "native-tls")))]
fn accept_invalid_certs_from_env() -> bool {
    dotenvy::dotenv().ok();
    matches!(
        std::env::var("ONEIO_ACCEPT_INVALID_CERTS")
            .unwrap_or_default()
            .to_lowercase()
            .as_str(),
        "true" | "yes" | "y" | "1"
    )
}

/// Global default client for free-standing functions.
pub(crate) fn default_oneio() -> Result<&'static crate::client::OneIo, OneIoError> {
    use std::sync::OnceLock;
    static DEFAULT_ONEIO: OnceLock<Result<crate::client::OneIo, String>> = OnceLock::new();

    match DEFAULT_ONEIO.get_or_init(|| OneIoBuilder::new().build().map_err(|e| e.to_string())) {
        Ok(oneio) => Ok(oneio),
        Err(message) => Err(OneIoError::Network(Box::new(std::io::Error::other(
            message.clone(),
        )))),
    }
}
