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
use log::info;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tokio::time::sleep;

pub struct ModuleParam {
    name: String,
    description: String,
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
            let has_needed_params = module_ro.has_needed_params().await;
            info!(
                "module {} has needed params: {}",
                module_ro.name(),
                has_needed_params
            );
            if !has_needed_params {
                ret = false;
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

    pub async fn run(&mut self) {
        self.send_modules().await;
        let mut tasks = JoinSet::new();
        for module in self.modules.iter() {
            let mut module_rw = module.write().await;
            let variations_cooldown_durations = module_rw.variation_durations().await;
            drop(module_rw);
            for (variation, duration) in variations_cooldown_durations.iter().enumerate() {
                let module = module.clone();
                let duration = *duration;
                tasks.spawn(tokio::spawn(async move {
                    let module = module.clone();
                    loop {
                        let mut module_rw = module.write().await;
                        module_rw.run(variation).await;
                        drop(module_rw);
                        sleep(duration).await;
                    }
                }));
            }
        }
        tasks.join_next().await;
    }
}
