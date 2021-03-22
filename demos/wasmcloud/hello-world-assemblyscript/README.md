# Hello World AssemblyScript for wasmCloud

A simple hello world example in AssemblyScript that will print "Hello World" as
an HTTP response.

It is meant to be a simple demo for the wasmcloud-provider with Krustlet.

## Running the example

This example has already been pre-built, so you only need to install it into
your Kubernetes cluster.

Create the pod and configmap with `kubectl`:

```shell
$ kubectl apply -f k8s.yaml
```

Once the pod is running, you can use `curl` to reach the pod:

```shell
$ curl http://localhost:8080
```

## Building from Source

If you want to compile the demo and inspect it, you'll need to do the following.

### Prerequisites

You'll need `npm` installed in order to install and build the dependencies.

You will also need to install [`wash`](https://github.com/wasmCloud/wash#installing-wash).
This tool is used for signing and managing your wasmCloud compatible modules.

If you are interested in starting your own AssemblyScript project, visit the
AssemblyScript
[getting started guide](https://docs.assemblyscript.org/quick-start).

### Compiling

Run:

```shell
$ npm install
$ npm run codegen
$ npm run asbuild
```

### Signing

Before pushing the actor module, you will need to sign it and grant it a few capabilities.

```shell
$ wash claims sign --http_server --logging --blob_store ./build/optimized.wasm --name hello-world-wasmcloud-assemblyscript
No keypair found in "/Users/foobar/.wash/keys/optimized_module.nk".
We will generate one for you and place it there.
If you'd like to use alternative keys, you can supply them as a flag.

Successfully signed ./build/optimized_s.wasm with capabilities: wasmcloud:httpserver,wasmcloud:blobstore,wasmcloud:logging
```

### Pushing

Once signed, you can push it to an OCI registry. Please note that you'll need to be signed into that
registry in order to push:

```shell
$ wasm-to-oci push ./build/optimized_s.wasm webassembly.azurecr.io/hello-world-wasmcloud-assemblyscript:v0.2.0
```


