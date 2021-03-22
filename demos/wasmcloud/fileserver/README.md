# Fileserver

An example that will respond with the file metadata present in the volume,
based on the URL path provided.

If a POST request is made, the contents of the request body are written to the
file based on the URL path provided.

If a DELETE request is made, the file is removed based on the URL path provided.

It is meant to demonstrate how volumes work with the wasmcloud-provider.

## Running the example

This example has already been pre-built, so you only need to install it into
your Kubernetes cluster.

Create the pod and configmap with `kubectl`:

```shell
$ kubectl create -f k8s.yaml
```

Once the pod is running, you can upload data with the following command:

```shell
$ curl -X POST http://localhost:8080/foo -d 'foobar'
```

You can then get metadata by running:

```shell
$ curl http://localhost:8080/foo
OUTPUT TODO
```

And you can delete the file with:

```shell
$ curl -X DELETE http://localhost:8080/foo
OUTPUT TODO
```

## Building the example

To set up your development environment, you'll need the following tools:

- cargo
- wasm-to-oci
- wash

Instructions for [installing
`cargo`](https://doc.rust-lang.org/cargo/getting-started/installation.html) and
[`wasm-to-oci`](https://github.com/engineerd/wasm-to-oci) can be found in their
respective project's documentation. Once those are installed,
[`wash`](https://github.com/wasmCloud/wash#installing-wash) can be installed with

```shell
$ cargo install wash-cli
```

Once complete, you'll need to build the binary:

```shell
$ cargo build --release
```

After the binary is built, you'll need to sign the compiled Wasm binary so it can be trusted by
wasmCloud:

```shell
$ wash claims sign --http_server --logging --blob_store ./target/wasm32-unknown-unknown/release/fileserver.wasm --name fileserver
No keypair found in "/Users/foobar/.wash/keys/fileserver_module.nk".
We will generate one for you and place it there.
If you'd like to use alternative keys, you can supply them as a flag.

Successfully signed ./target/wasm32-unknown-unknown/release/fileserver_s.wasm with capabilities: wasmcloud:httpserver,wasmcloud:blobstore,wasmcloud:logging
```

Once signed, you can push it to an OCI registry. Please note that you'll need to be signed into that
registry in order to push:

```shell
$ wasm-to-oci push ./target/wasm32-unknown-unknown/release/fileserver_s.wasm webassembly.azurecr.io/fileserver-wasmcloud:v0.3.0  
```
