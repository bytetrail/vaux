#[cfg(feature = "developer")]
struct Verifier;

#[cfg(feature = "developer")]
impl rustls::client::ServerCertVerifier for Verifier {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::Certificate,
        intermediates: &[rustls::Certificate],
        server_name: &rustls::ServerName,
        scts: &mut dyn Iterator<Item = &[u8]>,
        ocsp_response: &[u8],
        now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        println!("Host name {:?}", server_name);
        println!("End entity certificate {:?}", end_entity);
        println!("Intermediate certificates {:?}", intermediates);
        println!("OCSP response {:?}", ocsp_response);
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}
