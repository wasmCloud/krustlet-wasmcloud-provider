// This library was adapted from the original wascc logging
// implementation contributed by Brian Ketelsen to wascc.
// Original license below:

// Copyright 2015-2019 Capital One Services, LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use wasmcloud_actor_core::{CapabilityConfiguration, HealthCheckResponse};
use wasmcloud_actor_logging::{WriteLogArgs, OP_LOG};
use wasmcloud_provider_core::capabilities::{CapabilityProvider, Dispatcher, NullDispatcher};
use wasmcloud_provider_core::core::{OP_BIND_ACTOR, OP_HEALTH_REQUEST, OP_REMOVE_ACTOR};
use wasmcloud_provider_core::{deserialize, serialize};

use log::Log;

use std::collections::HashMap;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::sync::{Arc, RwLock};

use simplelog::{Config, LevelFilter, WriteLogger};

#[cfg(not(feature = "static_plugin"))]
capability_provider!(LoggingProvider, LoggingProvider::new);

pub const LOG_PATH_KEY: &str = "LOG_PATH";

/// Origin of messages coming from wasmcloud host
const SYSTEM_ACTOR: &str = "system";

#[allow(dead_code)]
const CAPABILITY_ID: &str = "wasmcloud:logging";

const ERROR: &str = "error";
const WARN: &str = "warn";
const INFO: &str = "info";
const DEBUG: &str = "debug";
const TRACE: &str = "trace";

/// LoggingProvider provides an implementation of the wasmcloud:logging capability
/// that keeps separate log output for each actor.
#[derive(Clone)]
pub struct LoggingProvider {
    dispatcher: Arc<RwLock<Box<dyn Dispatcher>>>,
    output_map: Arc<RwLock<HashMap<String, Box<WriteLogger<File>>>>>,
}

impl Default for LoggingProvider {
    fn default() -> Self {
        LoggingProvider {
            dispatcher: Arc::new(RwLock::new(Box::new(NullDispatcher::new()))),
            output_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl LoggingProvider {
    pub fn new() -> Self {
        Self::default()
    }

    fn configure(
        &self,
        config: CapabilityConfiguration,
    ) -> Result<Vec<u8>, Box<dyn Error + Sync + Send>> {
        let path = config
            .values
            .get(LOG_PATH_KEY)
            .ok_or("log file path was unspecified")?;

        let file = OpenOptions::new().write(true).open(path)?;
        let logger = WriteLogger::new(LevelFilter::Trace, Config::default(), file);
        let mut output_map = self.output_map.write().unwrap();
        output_map.insert(config.module, logger);
        Ok(vec![])
    }
}

impl CapabilityProvider for LoggingProvider {
    // Invoked by the runtime host to give this provider plugin the ability to communicate
    // with actors
    fn configure_dispatch(
        &self,
        dispatcher: Box<dyn Dispatcher>,
    ) -> Result<(), Box<dyn Error + Sync + Send>> {
        let mut lock = self.dispatcher.write().unwrap();
        *lock = dispatcher;

        Ok(())
    }

    // Invoked by host runtime to allow an actor to make use of the capability
    // All providers MUST handle the "configure" message, even if no work will be done
    fn handle_call(
        &self,
        actor: &str,
        op: &str,
        msg: &[u8],
    ) -> Result<Vec<u8>, Box<dyn Error + Sync + Send>> {
        match (op, actor) {
            (OP_BIND_ACTOR, SYSTEM_ACTOR) => {
                let cfg_vals = deserialize::<CapabilityConfiguration>(msg)?;
                self.configure(cfg_vals)
            }
            (OP_REMOVE_ACTOR, SYSTEM_ACTOR) => Ok(vec![]),
            (OP_HEALTH_REQUEST, SYSTEM_ACTOR) => Ok(serialize(HealthCheckResponse {
                healthy: true,
                message: "".to_string(),
            })?),
            (OP_LOG, _) => {
                let log_msg = deserialize::<WriteLogArgs>(msg)?;

                let level = match &*log_msg.level {
                    ERROR => log::Level::Error,
                    WARN => log::Level::Warn,
                    INFO => log::Level::Info,
                    DEBUG => log::Level::Debug,
                    TRACE => log::Level::Trace,
                    _ => return Err(format!("Unknown log level {}", log_msg.level).into()),
                };

                let output_map = self.output_map.read().unwrap();
                let logger = output_map
                    .get(actor)
                    .ok_or(format!("Unable to find logger for actor {}", actor))?;
                logger.log(
                    &log::Record::builder()
                        .args(format_args!("[{}] {}", actor, log_msg.text))
                        .level(level)
                        .target(&log_msg.target)
                        .build(),
                );
                Ok(vec![])
            }
            _ => Err(format!("Unknown operation: {}", op).into()),
        }
    }

    // No cleanup needed on stop
    fn stop(&self) {}
}
