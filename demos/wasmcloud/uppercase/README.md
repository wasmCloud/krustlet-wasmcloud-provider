# Uppercase

An example that will respond with the uppercased version of the querystring
sent in.

It is meant to be a simple demo for the wasmcloud-provider with Krustlet.

## Video

You can watch a video of the creation of this actor on
[Youtube](https://www.youtube.com/watch?v=uy91W7OxHcQ).

## Running the example

This example has already been pre-built, so you only need to install it into
your Kubernetes cluster.

Create the pod and configmap with `kubectl`:

```shell
$ kubectl apply -f uppercase-wasmcloud.yaml
```

Once the pod is running, you can run a `curl` command and the service will
return your query string uppercased:

```shell
$ curl http://localhost:8080?hello=world
{"original":"hello=world","uppercased":"HELLO=WORLD"}
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
$ wash claims sign --http_server --logging --blob_store ./target/wasm32-unknown-unknown/release/uppercase.wasm --name uppercase
No keypair found in "/Users/foobar/.wash/keys/uppercase_module.nk".
We will generate one for you and place it there.
If you'd like to use alternative keys, you can supply them as a flag.

Successfully signed ./target/wasm32-unknown-unknown/release/uppercase_s.wasm with capabilities: wasmcloud:httpserver,wasmcloud:blobstore,wasmcloud:logging
```

Once signed, you can push it to an OCI registry. Please note that you'll need to be signed into that
registry in order to push:

```shell
$ wasm-to-oci push ./target/wasm32-unknown-unknown/release/uppercase_s.wasm webassembly.azurecr.io/uppercase-wasmcloud:v0.4.0  
```
