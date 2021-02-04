use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::{Node, Pod, Taint};
use kube::api::{Api, DeleteParams, ListParams, LogParams, PostParams};
use kube_runtime::watcher::{watcher, Event};
use serde_json::json;

#[tokio::test]
async fn test_wasmcloud_provider() -> Result<(), Box<dyn std::error::Error>> {
    let client = kube::Client::try_default().await?;

    let nodes: Api<Node> = Api::all(client);

    let node = nodes.get("krustlet-wasmcloud").await?;

    verify_wasmcloud_node(node).await;

    let client: kube::Client = nodes.into();

    let _cleaner = WasmCloudTestResourceCleaner {};

    let pods: Api<Pod> = Api::namespaced(client.clone(), "default");

    create_wasmcloud_pod(client.clone(), &pods).await?;

    let mut tries: u8 = 0;
    loop {
        // Send a request to the pod to trigger some logging
        if reqwest::get("http://127.0.0.1:30000").await.is_ok() {
            break;
        }
        tries += 1;
        if tries == 10 {
            panic!("wasmCloud pod failed 10 readiness checks.");
        }
        tokio::time::delay_for(std::time::Duration::from_millis(100)).await;
    }

    let logs = pods
        .logs("greet-wasmcloud", &LogParams::default())
        .await
        .expect("unable to get logs");
    assert!(logs.contains("warn something"));
    assert!(logs.contains("info something"));
    assert!(logs.contains("raw msg I'm a Body!"));
    assert!(logs.contains("error body"));

    Ok(())
}

async fn verify_wasmcloud_node(node: Node) {
    let node_status = node.status.expect("node reported no status");
    assert_eq!(
        node_status
            .node_info
            .expect("node status reported no info")
            .architecture,
        "wasm-wasi",
        "expected node to support the wasm-wasi architecture"
    );

    let node_meta = node.metadata;
    assert_eq!(
        node_meta
            .labels
            .expect("node had no labels")
            .get("kubernetes.io/arch")
            .expect("node did not have kubernetes.io/arch label"),
        "wasm32-wasmcloud"
    );

    let taints = node
        .spec
        .expect("node had no spec")
        .taints
        .expect("node had no taints");
    let taint = taints
        .iter()
        .find(|t| (t.key == "kubernetes.io/arch") & (t.effect == "NoExecute"))
        .expect("did not find kubernetes.io/arch taint");
    // There is no "operator" field in the type for the crate for some reason,
    // so we can't compare it here
    assert_eq!(
        taint,
        &Taint {
            effect: "NoExecute".to_owned(),
            key: "kubernetes.io/arch".to_owned(),
            value: Some("wasm32-wasmcloud".to_owned()),
            ..Default::default()
        }
    );

    let taint = taints
        .iter()
        .find(|t| (t.key == "kubernetes.io/arch") & (t.effect == "NoSchedule"))
        .expect("did not find kubernetes.io/arch taint");
    // There is no "operator" field in the type for the crate for some reason,
    // so we can't compare it here
    assert_eq!(
        taint,
        &Taint {
            effect: "NoSchedule".to_owned(),
            key: "kubernetes.io/arch".to_owned(),
            value: Some("wasm32-wasmcloud".to_owned()),
            ..Default::default()
        }
    );
}

async fn create_wasmcloud_pod(client: kube::Client, pods: &Api<Pod>) -> anyhow::Result<()> {
    let p = serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": "greet-wasmcloud"
        },
        "spec": {
            "containers": [
                {
                    "name": "greet-wasmcloud",
                    "image": "webassembly.azurecr.io/greet-wascc:v0.4",
                    "ports": [
                        {
                            "containerPort": 8080,
                            "hostPort": 30000
                        }
                    ],
                },
            ],
            "tolerations": [
                {
                    "effect": "NoExecute",
                    "key": "kubernetes.io/arch",
                    "operator": "Equal",
                    "value": "wasm32-wasmcloud"
                },
                {
                    "effect": "NoSchedule",
                    "key": "kubernetes.io/arch",
                    "operator": "Equal",
                    "value": "wasm32-wasmcloud"
                },
            ]
        }
    }))?;

    let pod = pods.create(&PostParams::default(), &p).await?;

    assert_eq!(pod.status.unwrap().phase.unwrap(), "Pending");

    wait_for_pod_ready(client, "greet-wasmcloud", "default").await?;

    Ok(())
}

struct WasmCloudTestResourceCleaner {}

impl Drop for WasmCloudTestResourceCleaner {
    fn drop(&mut self) {
        let t = std::thread::spawn(move || {
            let mut rt =
                tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime for cleanup");
            rt.block_on(clean_up_wasmcloud_test_resources());
        });

        t.join()
            .expect("Failed to clean up wasmcloud test resources");
    }
}

async fn clean_up_wasmcloud_test_resources() {
    let client = kube::Client::try_default()
        .await
        .expect("Failed to create client");

    let pods: Api<Pod> = Api::namespaced(client.clone(), "default");
    pods.delete("greet-wasmcloud", &DeleteParams::default())
        .await
        .expect("Failed to delete pod");
}

pub async fn wait_for_pod_ready(
    client: kube::Client,
    pod_name: &str,
    namespace: &str,
) -> anyhow::Result<()> {
    let api: Api<Pod> = Api::namespaced(client, namespace);
    let inf = watcher(
        api,
        ListParams::default()
            .fields(&format!("metadata.name={}", pod_name))
            .timeout(30),
    );

    let mut watcher = inf.boxed();
    let mut went_ready = false;
    while let Some(event) = watcher.try_next().await? {
        if let Event::Applied(o) = event {
            let containers = o
                .clone()
                .status
                .unwrap()
                .container_statuses
                .unwrap_or_else(Vec::new);
            let phase = o.status.unwrap().phase.unwrap();
            if (phase == "Running")
                & (!containers.is_empty())
                & containers.iter().all(|status| status.ready)
            {
                went_ready = true;
                break;
            }
        }
    }

    assert!(went_ready, "pod never went ready");

    Ok(())
}
