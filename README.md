# file store client


### Service address:[file store service](https://github.com/luyikk/file-store-server)

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

help
```shell 
Usage: fsc <COMMAND>

Commands:
  create  create config
  push    push file
  pull    pull file
  image   image path
  show    show remote directory contents
  info    show remote file info
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

push
```shell
Usage: fsc push [OPTIONS] <FILE>

Arguments:
  <FILE>  local file

Options:
  -d, --dir <DIR>      save dir
  -a, --async          async write
  -b, --block <BLOCK>  transfer block size default 131072 [default: 131072]
  -o, --overwrite      if service exists file, over write file
  -h, --help           Print help
```

fsc image push
```shell
Usage: fsc image push [OPTIONS] <PATH>

Arguments:
  <PATH>  local path

Options:
  -d, --dir <DIR>      save dir
  -a, --async          async write
  -b, --block <BLOCK>  transfer block size default 131072 [default: 131072]
  -o, --overwrite      if service exists file, over write file
  -h, --help           Print help
```

fsc pull
```shell
Usage: fsc pull [OPTIONS] <FILE>

Arguments:
  <FILE>  remote file path

Options:
  -s, --save <SAVE>    save file path
  -b, --block <BLOCK>  transfer block size default 131072 [default: 131072]
  -o, --overwrite      if exists file, over write file
  -h, --help           Print help
```

example
```shell
fsc push ./file
fsc image push ./dirctory
fsc pull ./file
fsc pull ./file -s ./save_file
```