use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};
use tokio_rustls::{
    TlsAcceptor,
    rustls::{
        ServerConfig,
        crypto::ring,
        pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, pem::PemObject},
    },
};

/// Build a TLS acceptor from PEM-encoded certificate chain and private key files.
pub fn acceptor_from_files(cert: &Path, key: &Path) -> Result<TlsAcceptor> {
    let certs = CertificateDer::pem_file_iter(cert)
        .with_context(|| format!("reading TLS certificate {}", cert.display()))?
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("parsing TLS certificate {}", cert.display()))?;
    let key = PrivateKeyDer::from_pem_file(key)
        .with_context(|| format!("reading TLS private key {}", key.display()))?;
    build(certs, key)
}

/// Build a TLS acceptor with a freshly generated self-signed certificate.
pub fn self_signed_acceptor(hostname: &str) -> Result<TlsAcceptor> {
    let mut names = vec!["localhost".to_string(), "127.0.0.1".to_string()];
    if !names.iter().any(|n| n == hostname) {
        names.insert(0, hostname.to_string());
    }
    let generated = rcgen::generate_simple_self_signed(names).context("generating certificate")?;
    let cert = generated.cert.der().clone();
    let key = PrivatePkcs8KeyDer::from(generated.signing_key.serialize_der()).into();
    build(vec![cert], key)
}

fn build(certs: Vec<CertificateDer<'static>>, key: PrivateKeyDer<'static>) -> Result<TlsAcceptor> {
    let config = ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("invalid TLS certificate or key")?;
    Ok(TlsAcceptor::from(Arc::new(config)))
}
