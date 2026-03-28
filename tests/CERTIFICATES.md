# Test Certificate Generation

This document describes how the test certificates in this directory were generated.

## Files

- `test-cert.pem` - Self-signed CA certificate in PEM format
- `test-cert.der` - Same certificate in DER format
- `test-key.pem` - Private key (not used in tests, generated as side effect)

## Generation Commands

### 1. Generate self-signed certificate and key:

```bash
openssl req -x509 -newkey rsa:2048 \
    -keyout tests/test-key.pem \
    -out tests/test-cert.pem \
    -days 365 \
    -nodes \
    -subj "/C=US/ST=Test/L=Test/O=Test/CN=test.example.com"
```

Parameters:
- `-x509`: Generate self-signed certificate instead of CSR
- `-newkey rsa:2048`: Generate new RSA key with 2048 bits
- `-keyout`: Output file for private key
- `-out`: Output file for certificate
- `-days 365`: Certificate validity period (1 year)
- `-nodes`: No DES encryption (no password on private key)
- `-subj`: Subject fields for certificate

### 2. Convert PEM to DER format:

```bash
openssl x509 -in tests/test-cert.pem -outform DER -out tests/test-cert.der
```

Parameters:
- `-in`: Input PEM certificate
- `-outform DER`: Output in DER (binary) format
- `-out`: Output file

## Certificate Details

- **Subject**: /C=US/ST=Test/L=Test/O=Test/CN=test.example.com
- **Issuer**: Same as subject (self-signed)
- **Validity**: 1 year from generation date
- **Key Type**: RSA 2048-bit
- **Signature Algorithm**: SHA256 with RSA

## Usage in Tests

These certificates are used to test:
- `OneIoBuilder::add_root_certificate_pem()`
- `OneIoBuilder::add_root_certificate_der()`
- `ONEIO_CA_BUNDLE` environment variable support
- Custom TLS certificate loading for corporate proxies (e.g., Cloudflare WARP)

## Regenerating Certificates

If the certificate expires or you need to regenerate it:

1. Delete the old files: `rm tests/test-cert.pem tests/test-cert.der tests/test-key.pem`
2. Run the commands above
3. Commit the new files

Note: Since this is a self-signed test certificate, it should NOT be used for production or any real TLS connections.
