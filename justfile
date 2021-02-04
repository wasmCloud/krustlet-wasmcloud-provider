export RUST_LOG := "wascc_host=debug,wascc_provider=debug,main=debug"
export PFX_PASSWORD := "testing"
export CONFIG_DIR := env_var_or_default('CONFIG_DIR', '$HOME/.krustlet/config')

build +FLAGS='':
    cargo build {{FLAGS}}

test:
    cargo fmt --all -- --check
    cargo clippy --workspace
    cargo test --workspace --lib
    cargo test --doc --all

test-e2e:
    cargo test --test integration_tests

test-e2e-standalone:
    cargo run --bin oneclick

run +FLAGS='': bootstrap
    KUBECONFIG=$(eval echo $CONFIG_DIR)/kubeconfig-wascc cargo run --bin krustlet-wascc {{FLAGS}} -- --node-name krustlet-wascc --port 3000 --bootstrap-file $(eval echo $CONFIG_DIR)/bootstrap.conf --cert-file $(eval echo $CONFIG_DIR)/krustlet-wascc.crt --private-key-file $(eval echo $CONFIG_DIR)/krustlet-wascc.key

bootstrap:
    @# This is to get around an issue with the default function returning a string that gets escaped
    @mkdir -p $(eval echo $CONFIG_DIR)
    @test -f  $(eval echo $CONFIG_DIR)/bootstrap.conf || CONFIG_DIR=$(eval echo $CONFIG_DIR) ./scripts/bootstrap.sh
    @chmod 600 $(eval echo $CONFIG_DIR)/*
