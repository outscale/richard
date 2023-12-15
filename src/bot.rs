use crate::endpoints::Endpoints;
use crate::feeds::Feeds;
use crate::github::Github;
use crate::hello::Hello;
use crate::help::Help;
use crate::ollama::Ollama;
use crate::ping::Ping;
use crate::roll::Roll;
use crate::triggers::Triggers;
use crate::webpages::Webpages;
use async_trait::async_trait;
use log::{error, trace};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tokio::time::sleep;

#[derive(Clone)]
pub struct ModuleParam {
    pub name: String,
    pub description: String,
    pub optional: bool,
}

impl ModuleParam {
    pub fn new(name: &str, description: &str, optional: bool) -> ModuleParam {
        ModuleParam {
            name: name.to_string(),
            description: description.to_string(),
            optional,
        }
    }
}

#[async_trait]
pub trait Module {
    fn name(&self) -> &'static str;
    fn params(&self) -> Vec<ModuleParam>;
    async fn module_offering(&mut self, modules: &[SharedModule]);
    async fn has_needed_params(&self) -> bool;
    async fn run(&mut self, variation: usize); // alternative to `variation`?
    async fn variation_durations(&mut self) -> Vec<Duration>;
    async fn trigger(&mut self, message: &str, id: &str);
}

pub type SharedModule = Arc<RwLock<Box<dyn Module + Send + Sync>>>;
pub struct Bot {
    modules: Vec<SharedModule>,
}

impl Bot {
    pub fn new() -> Bot {
        let mut bot = Bot {
            modules: Vec::new(),
        };
        bot.modules
            .push(Arc::new(RwLock::new(Box::new(Ping::new().unwrap()))));
        bot.modules
            .push(Arc::new(RwLock::new(Box::new(Help::new().unwrap()))));
        bot.modules
            .push(Arc::new(RwLock::new(Box::new(Triggers::new().unwrap()))));
        bot.modules
            .push(Arc::new(RwLock::new(Box::new(Endpoints::new().unwrap()))));
        bot.modules
            .push(Arc::new(RwLock::new(Box::new(Github::new().unwrap()))));
        bot.modules
            .push(Arc::new(RwLock::new(Box::new(Hello::new().unwrap()))));
        bot.modules
            .push(Arc::new(RwLock::new(Box::new(Ollama::new().unwrap()))));
        bot.modules
            .push(Arc::new(RwLock::new(Box::new(Feeds::new().unwrap()))));
        bot.modules
            .push(Arc::new(RwLock::new(Box::new(Roll::new().unwrap()))));
        bot.modules
            .push(Arc::new(RwLock::new(Box::new(Webpages::new().unwrap()))));
        bot
    }

    pub async fn ready(&mut self) -> bool {
        let mut ret = true;
        for module in self.modules.iter() {
            let module_ro = module.read().await;
            let name = module_ro.name();
            let params = module_ro.params();
            drop(module_ro);
            if params.is_empty() {
                continue;
            }
            for param in params {
                if param.optional {
                    continue;
                }
                match env::var(&param.name) {
                    Ok(value) => {
                        if value.is_empty() {
                            error!(
                                "module {} need mandatory environment variable {}",
                                name, param.name
                            );
                            ret = false;
                        }
                    }
                    Err(_) => {
                        error!(
                            "module {} need mandatory environment variable {}",
                            name, param.name
                        );
                        ret = false;
                    }
                }
            }
        }
        ret
    }

    async fn send_modules(&self) {
        for module in self.modules.iter() {
            let mut module_rw = module.write().await;
            module_rw.module_offering(&self.modules).await;
        }
    }

    pub async fn help(&self) -> String {
        let mut output = String::new();
        for module in self.modules.iter() {
            let module_ro = module.read().await;
            let name = module_ro.name();
            let params = module_ro.params();
            drop(module_ro);
            output.push_str(format!("# '{name}' module parameters\n").as_str());
            if params.is_empty() {
                output.push_str("  [no parameter]\n\n");
                continue;
            }
            for param in params {
                output.push_str(
                    format!(
                        "- {}: {} (optional: {})\n",
                        param.name, param.description, param.optional
                    )
                    .as_str(),
                );
            }
            output.push('\n');
        }
        output
    }

    pub async fn run(&mut self) {
        self.send_modules().await;
        let mut tasks = JoinSet::new();
        for module in self.modules.iter() {
            let mut module_rw = module.write().await;
            let variations_cooldown_durations = module_rw.variation_durations().await;
            let module_name = module_rw.name();
            drop(module_rw);
            for (variation, duration) in variations_cooldown_durations.iter().enumerate() {
                let module = module.clone();
                let duration = *duration;
                tasks.spawn(tokio::spawn(async move {
                    let module = module.clone();
                    loop {
                        trace!("get module {} lock ...", module_name);
                        let mut module_rw = module.write().await;
                        trace!("module {} lock aquired", module_name);
                        trace!("module {} run variation {}", module_name, variation);
                        module_rw.run(variation).await;
                        drop(module_rw);
                        trace!(
                            "module {} run variation {} is now sleeping for {:#?}",
                            module_name,
                            variation,
                            duration
                        );
                        sleep(duration).await;
                    }
                }));
            }
        }
        tasks.join_next().await;
    }
}
