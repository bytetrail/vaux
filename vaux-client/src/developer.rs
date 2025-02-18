use rustls::pki_types::{CertificateDer, ServerName, UnixTime};

#[cfg(feature = "developer")]
#[derive(Default, Debug, Clone)]
pub(crate) struct Verifier;

#[cfg(feature = "developer")]
impl rustls::client::danger::ServerCertVerifier for Verifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer,
        intermediates: &[CertificateDer],
        server_name: &ServerName,
        ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        println!("Host name {:?}", server_name);
        println!("End entity certificate {:?}", end_entity);
        println!("Intermediate certificates {:?}", intermediates);
        println!("OCSP response {:?}", ocsp_response);
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        todo!()
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        todo!()
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        todo!()
    }
}
