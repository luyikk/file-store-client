# file store client

Make config file
``` sh
fsc create
```

```toml
[server]
# server addr
addr="127.0.0.1:7556"
# used to verify whether the service_name
service_name="file-store-service"
# used to verify whether the verify_key
verify_key=""
# the timeout period for the client to request the server
request_out_time_ms=15000

# used to configure TLS communication encryption (optional).
# if not provided, TLS will not be used for communication encryption
[tls]
# ca file path (optional)
# if not provided, the server’s certificate will not be verified.
ca = "./tls/ca.crt"

# cert file path
cert = "./tls/client-crt.pem"

# key file path
key = "./tls/client-key.pem"
```

push file
```sh 
fsc push -d [directory] file
```

help
```sh 
fsc --help
```