fn main() {
    let http = std::env::var("CARGO_FEATURE_HTTP").is_ok();
    let ftp = std::env::var("CARGO_FEATURE_FTP").is_ok();
    let remote = std::env::var("CARGO_FEATURE_REMOTE").is_ok();
    let rustls = std::env::var("CARGO_FEATURE_RUSTLS").is_ok();
    let native_tls = std::env::var("CARGO_FEATURE_NATIVE_TLS").is_ok();

    let needs_tls = http || ftp || remote;
    let has_tls = rustls || native_tls;

    if needs_tls && !has_tls {
        panic!(
            "Feature `http`, `ftp`, or `remote` requires at least one of `rustls` or `native-tls` features to be enabled."
        );
    }
}
