//! A custom kubelet backend that can run [waSCC](https://wascc.dev/) based workloads
//!
//! The crate provides the [`WasccProvider`] type which can be used
//! as a provider with [`kubelet`].
//!
//! # Example
//! ```rust,no_run
//! use kubelet::{Kubelet, config::Config};
//! use kubelet::module_store::FileModuleStore;
//! use wascc_provider::WasccProvider;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Get a configuration for the Kubelet
//!     let kubelet_config = Config::default();
//!     let client = oci_distribution::Client::default();
//!     let store = FileModuleStore::new(client, &std::path::PathBuf::from(""));
//!
//!     // Instantiate the provider type
//!     let provider = WasccProvider::new(store, &kubelet_config).await.unwrap();
//!
//!     // Load a kubernetes configuration
//!     let kubeconfig = kube::config::load_kube_config().await.unwrap();
//!     
//!     // Instantiate the Kubelet
//!     let kubelet = Kubelet::new(provider, kubeconfig, kubelet_config);
//!     // Start the Kubelet and block on it
//!     kubelet.start().await.unwrap();
//! }
//! ```

#![warn(missing_docs)]

use async_trait::async_trait;
use kube::client::Client;
use kubelet::module_store::ModuleStore;
use kubelet::provider::ProviderError;
use kubelet::status::{ContainerStatus, Status};
use kubelet::{Pod, Provider};
use kubelet::handle::{PodHandle, RuntimeHandle, Stop, key_from_pod, pod_key};
use log::{error,debug, info, warn};
use wascc_host::{host, Actor, NativeCapability};
use tokio::sync::RwLock;
use tokio::fs::File;
use tokio::sync::watch::{self, Receiver};
use tempfile::NamedTempFile;

use wascc_logging::{LOG_PATH_KEY};

use std::collections::HashMap;
use std::path::{PathBuf, Path};
use std::sync::Arc;

const ACTOR_PUBLIC_KEY: &str = "deislabs.io/wascc-action-key";
const TARGET_WASM32_WASCC: &str = "wasm32-wascc";

/// The name of the HTTP capability.
const HTTP_CAPABILITY: &str = "wascc:http_server";
const LOG_CAPABILITY: &str = "wascc:logging";

const LOG_DIR_NAME: &str = "wascc-logs";

#[cfg(target_os = "linux")]
const HTTP_LIB: &str = "./lib/libwascc_httpsrv.so";

#[cfg(target_os = "linux")]
const LOG_LIB: &str = "./lib/libwascc_logging.so";

#[cfg(target_os = "macos")]
const HTTP_LIB: &str = "./lib/libwascc_httpsrv.dylib";

#[cfg(target_os = "macos")]
const LOG_LIB: &str = "./lib/libwascc_logging.dylib";

/// Kubernetes' view of environment variables is an unordered map of string to string.
type EnvVars = std::collections::HashMap<String, String>;

/// A [kubelet::handle::Stop] implementation for a wascc actor
pub struct ActorStopper {
    pub key: String,
}

#[async_trait::async_trait]
impl Stop for ActorStopper {
    async fn stop(&mut self) -> anyhow::Result<()> {
        debug!("stopping wascc instance {}", self.key);
        host::remove_actor(&self.key).map_err(|e| anyhow::anyhow!("unable to remove actor: {:?}", e))
    }

    async fn wait(&mut self) -> anyhow::Result<()> {
        // TODO: Figure out if there is a way to wait for an actor to be removed
        Ok(())
    }
}

/// WasccProvider provides a Kubelet runtime implementation that executes WASM binaries.
///
/// Currently, this runtime uses WASCC as a host, loading the primary container as an actor.
/// TODO: In the future, we will look at loading capabilities using the "sidecar" metaphor
/// from Kubernetes.
#[derive(Clone)]
pub struct WasccProvider<S> {
    handles: Arc<RwLock<HashMap<String, PodHandle<File, ActorStopper>>>>,
    store: S,
    log_path: PathBuf,
    kubeconfig: kube::config::Configuration,
}

impl<S: ModuleStore + Send + Sync> WasccProvider<S> {
    /// Returns a new wasCC provider configured to use the proper data directory
    /// (including creating it if necessary)
    pub async fn new(store: S, config: &kubelet::config::Config, kubeconfig: kube::config::Configuration) -> anyhow::Result<Self> {
        let log_path = config.data_dir.to_path_buf().join(LOG_DIR_NAME);
        tokio::fs::create_dir_all(&log_path).await?;

        tokio::task::spawn_blocking(|| {
            warn!("Loading HTTP Capability");
            let data = NativeCapability::from_file(HTTP_LIB).map_err(|e| {
                anyhow::anyhow!("Failed to read HTTP capability {}: {}", HTTP_LIB, e)
            })?;
            host::add_native_capability(data)
                .map_err(|e| {
                    anyhow::anyhow!("Failed to load HTTP capability: {}", e)
            })?;

            warn!("Loading LOG Capability");
            let logdata = NativeCapability::from_file(LOG_LIB).map_err(|e| {
                anyhow::anyhow!("Failed to read LOG capability {}: {}", LOG_LIB, e)
            })?;
            host::add_native_capability(logdata)
                .map_err(|e| anyhow::anyhow!("Failed to load LOG capability: {}", e))
        })
        .await??;
        Ok(Self {
            handles: Default::default(),
            store,
            log_path,
            kubeconfig,
        })
    }
}

#[async_trait]
impl<S: ModuleStore + Send + Sync> Provider for WasccProvider<S> {
    const ARCH: &'static str = TARGET_WASM32_WASCC;
    fn can_schedule(&self, pod: &Pod) -> bool {
        // If there is a node selector and it has arch set to wasm32-wascc, we can
        // schedule it.
        pod.node_selector()
            .and_then(|i| {
                i.get("beta.kubernetes.io/arch")
                    .map(|v| v.eq(&TARGET_WASM32_WASCC))
            })
            .unwrap_or(false)
    }

    async fn add(&self, pod: Pod) -> anyhow::Result<()> {
        // To run an Add event, we load the WASM, update the pod status to Running,
        // and then execute the WASM, passing in the relevant data.
        // When the pod finishes, we update the status to Succeeded unless it
        // produces an error, in which case we mark it Failed.
        debug!("Pod added {:?}", pod.name());
        // This would lock us into one wascc actor per pod. I don't know if
        // that is a good thing. Other containers would then be limited
        // to acting as components... which largely follows the sidecar
        // pattern.
        //
        // Another possibility is to embed the key in the image reference
        // (image/foo.wasm@ed25519:PUBKEY). That might work best, but it is
        // not terribly useable.
        //
        // A really icky one would be to just require the pubkey in the env
        // vars and suck it out of there. But that violates the intention
        // of env vars, which is to communicate _into_ the runtime, not to
        // configure the runtime.

        // TODO: Implement this for real.
        //
        // What it should do:
        // - for each volume
        //   - set up the volume map
        // - for each init container:
        //   - set up the runtime
        //   - mount any volumes (popen)
        //   - run it to completion
        //   - bail with an error if it fails
        // - for each container and ephemeral_container
        //   - set up the runtime
        //   - mount any volumes (popen)
        //   - run it to completion
        //   - bail if it errors

        info!("Starting containers for pod {:?}", pod.name());
        let mut modules = self.store.fetch_pod_modules(&pod).await?;
        let mut container_handles = HashMap::new();
        let client = kube::Client::from(self.kubeconfig.clone());
        for container in pod.containers() {
            let env = Self::env_vars(&container, &pod, &client).await;

            debug!("Starting container {} on thread", container.name);

            let module_data = modules
                .remove(&container.name)
                .expect("FATAL ERROR: module map not properly populated");
            let lp = self.log_path.clone();
            let (status_sender, status_recv) = watch::channel(ContainerStatus::Waiting {
                timestamp: chrono::Utc::now(),
                message: "No status has been received from the process".into(),
            });
            let http_result =
                tokio::task::spawn_blocking(move || wascc_run_http(module_data, env, &lp, status_recv))
                    .await?;
            match http_result {
                Ok(handle) => {
                    container_handles.insert(container.name.clone(), handle);
                    status_sender.broadcast(ContainerStatus::Running {
                        timestamp: chrono::Utc::now(),
                    }).expect("status should be able to send");
                }
                Err(e) => {
                    status_sender.broadcast(ContainerStatus::Terminated {
                        timestamp: chrono::Utc::now(),
                        failed: true,
                        message: format!("Error while starting container: {:?}", e),
                    }).expect("status should be able to send");
                    return Err(anyhow::anyhow!("Failed to run pod: {}", e));
                }
            }
        }
        info!(
            "All containers started for pod {:?}. Updating status",
            pod.name()
        );
        // Wrap this in a block so the write lock goes out of scope when we are done
        {
            let mut handles = self.handles.write().await;
            handles.insert(
                key_from_pod(&pod),
                PodHandle::new(container_handles, pod, client)?,
            );
        }

        Ok(())
    }

    async fn modify(&self, pod: Pod) -> anyhow::Result<()> {
        // Modify will be tricky. Not only do we need to handle legitimate modifications, but we
        // need to sift out modifications that simply alter the status. For the time being, we
        // just ignore them, which is the wrong thing to do... except that it demos better than
        // other wrong things.
        info!("Pod modified");
        info!(
            "Modified pod spec: {:#?}",
            pod.as_kube_pod().status.as_ref().unwrap()
        );
        Ok(())
    }

    async fn delete(&self, pod: Pod) -> anyhow::Result<()> {
        let mut handles = self.handles.write().await;
        if let Some(mut h) = handles.remove(&key_from_pod(&pod)) {
            h.stop().await.unwrap_or_else(|e| {
                error!(
                    "unable to stop pod {} in namespace {}: {:?}",
                    pod.name(),
                    pod.namespace(),
                    e
                );
                // Insert the pod back in to our store if we failed to delete it
                handles.insert(key_from_pod(&pod), h);
            })
        } else {
            info!(
                "unable to find pod {} in namespace {}, it was likely already deleted",
                pod.name(),
                pod.namespace()
            );
        }
        Ok(())
    }

    async fn logs(
        &self,
        namespace: String,
        pod_name: String,
        container_name: String,
    ) -> anyhow::Result<Vec<u8>> {
        let mut handles = self.handles.write().await;
        let handle = handles
            .get_mut(&pod_key(&namespace, &pod_name))
            .ok_or_else(|| ProviderError::PodNotFound {
                pod_name: pod_name.clone(),
            })?;
        let mut output = Vec::new();
        handle.output(&container_name, &mut output).await?;
        Ok(output)
    }
}

/// Run a WasCC module inside of the host, configuring it to handle HTTP requests.
///
/// This bootstraps an HTTP host, using the value of the env's `PORT` key to expose a port.
fn wascc_run_http(data: Vec<u8>, env: EnvVars, log_path: &Path, status_recv: Receiver<ContainerStatus>) -> anyhow::Result<RuntimeHandle<File, ActorStopper>> {
    let mut caps: Vec<Capability> = Vec::new();

    caps.push(Capability {
        name: HTTP_CAPABILITY,
        env: env,
    });
    wascc_run(
        data,
        &mut caps,
        log_path,
        status_recv,
    )
}

/// Capability describes a waSCC capability.
///
/// Capabilities are made available to actors through a two-part processthread:
/// - They must be registered
/// - For each actor, the capability must be configured
struct Capability {
    name: &'static str,
    env: EnvVars,
}

/// Run the given WASM data as a waSCC actor with the given public key.
///
/// The provided capabilities will be configured for this actor, but the capabilities
/// must first be loaded into the host by some other process, such as register_native_capabilities().
fn wascc_run(data: Vec<u8>, capabilities: &mut Vec<Capability>, log_path: &Path, status_recv: Receiver<ContainerStatus>) -> anyhow::Result<RuntimeHandle<File, ActorStopper>> {
    info!("wascc run");

    let log_output = NamedTempFile::new_in(log_path)?;
    let mut logenv: HashMap<String, String> = HashMap::new();
    logenv.insert(LOG_PATH_KEY.to_string(), log_output.path().to_str().unwrap().to_owned());
    capabilities.push(Capability {
        name: LOG_CAPABILITY,
        env: logenv,
    });

    let load = Actor::from_bytes(data).map_err(|e| anyhow::anyhow!("Error loading WASM: {}", e))?;
    let pk = load.public_key();

    host::add_actor(load).map_err(|e| anyhow::anyhow!("Error adding actor: {}", e))?;
    capabilities.iter().try_for_each(|cap| {
        info!("configuring capability {}", cap.name);
        host::configure(&pk, cap.name, cap.env.clone())
            .map_err(|e| anyhow::anyhow!("Error configuring capabilities for module: {}", e))
    })?;
    info!("Instance executing");
    Ok(RuntimeHandle::new(tokio::fs::File::from_std(log_output.reopen()?), ActorStopper{key: pk}, status_recv))
}

#[cfg(test)]
mod test {
    use super::*;
    use k8s_openapi::api::core::v1::Pod as KubePod;
    use k8s_openapi::api::core::v1::PodSpec;
    use oci_distribution::Reference;

    pub struct TestStore {
        modules: HashMap<Reference, Vec<u8>>,
    }

    impl TestStore {
        fn new(modules: HashMap<Reference, Vec<u8>>) -> Self {
            Self { modules }
        }
    }

    #[async_trait]
    impl ModuleStore for TestStore {
        async fn get(&self, image_ref: &Reference) -> anyhow::Result<Vec<u8>> {
            self.modules
                .get(image_ref)
                .cloned()
                .ok_or(anyhow::anyhow!("Failed to find module for reference"))
        }
    }

    #[cfg(target_os = "linux")]
    const ECHO_LIB: &str = "./testdata/libecho_provider.so";
    #[cfg(target_os = "macos")]
    const ECHO_LIB: &str = "./testdata/libecho_provider.dylib";

    #[test]
    fn test_wascc_run() {

        use std::path::PathBuf;
        // Open file
        let data = std::fs::read("./testdata/echo.wasm").expect("read the wasm file");

        let log_path = PathBuf::from(r"~/.krustlet");
        
        // Send into wascc_run
        wascc_run_http(
            data,
            EnvVars::new(),
            "MB4OLDIC3TCZ4Q4TGGOVAZC43VXFE2JQVRAXQMQFXUCREOOFEKOKZTY2",
           &log_path,
        )
        .expect("successfully executed a WASM");

        // Give the webserver a chance to start up.
        std::thread::sleep(std::time::Duration::from_secs(3));
        wascc_stop("MB4OLDIC3TCZ4Q4TGGOVAZC43VXFE2JQVRAXQMQFXUCREOOFEKOKZTY2")
            .expect("Removed the actor");
    }

    #[test]
    fn test_wascc_echo() {
        let data = NativeCapability::from_file(ECHO_LIB).expect("loaded echo library");
        host::add_native_capability(data).expect("added echo capability");

        let key = "MDAYLDTOZEHQFPB3CL5PAFY5UTNCW32P54XGWYX3FOM2UBRYNCP3I3BF";

        let log_path = PathBuf::from(r"~/.krustlet");
        let wasm = std::fs::read("./testdata/echo_actor_s.wasm").expect("load echo WASM");
        // TODO: use wascc_run to execute echo_actor
        wascc_run(
            wasm,
            key,
            &mut vec![Capability {
                name: "wok:echoProvider",
                env: EnvVars::new(),
            }],
            &log_path,
        )
        .expect("completed echo run")
    }

    #[tokio::test]
    async fn test_can_schedule() {
        let store = TestStore::new(Default::default());

        let wr = WasccProvider::new(store, &Default::default())
            .await
            .unwrap();
        let mock = Default::default();
        assert!(!wr.can_schedule(&mock));

        let mut selector = std::collections::BTreeMap::new();
        selector.insert(
            "beta.kubernetes.io/arch".to_string(),
            "wasm32-wascc".to_string(),
        );
        let mut mock: KubePod = mock.into();
        mock.spec = Some(PodSpec {
            node_selector: Some(selector.clone()),
            ..Default::default()
        });
        let mock = Pod::new(mock);
        assert!(wr.can_schedule(&mock));
        selector.insert("beta.kubernetes.io/arch".to_string(), "amd64".to_string());
        let mut mock: KubePod = mock.into();
        mock.spec = Some(PodSpec {
            node_selector: Some(selector),
            ..Default::default()
        });
        let mock = Pod::new(mock);
        assert!(!wr.can_schedule(&mock));
    }
}