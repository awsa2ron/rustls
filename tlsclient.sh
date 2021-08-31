cargo run --example tlsclient -- \
--cafile ./certs/rootCA.pem \
--auth-key ./certs/private.pem.key \
--auth-certs ./certs/certificate.pem.crt \
--http --post -p 8443 \
a3k9okwzah2uac-ats.iot.ap-northeast-1.amazonaws.com
