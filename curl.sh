curl --tlsv1.2 -v \
--cacert ./certs/rootCA.pem \
--cert ./certs/certificate.pem.crt \
--key ./certs/private.pem.key \
--request POST \
--data "{ \"message\": \"Hello, world\" }" \
"https://a3k9okwzah2uac-ats.iot.ap-northeast-1.amazonaws.com:8443/topics/topic?qos=1"
