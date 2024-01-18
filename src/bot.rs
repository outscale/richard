use crate::down_detectors::DownDetectors;
use crate::feeds::Feeds;
use crate::github_orgs::GithubOrgs;
use crate::github_repos::GithubRepos;
use crate::hello::Hello;
use crate::help::Help;
use crate::ollama::Ollama;
use crate::outscale_api_versions::OutscaleApiVersions;
use crate::ping::Ping;
use crate::roll::Roll;
use crate::triggers::Triggers;
use crate::webex::Webex;
use crate::webpages::Webpages;
use async_trait::async_trait;
use log::{debug, error, info, trace};
use std::collections::HashMap;
use std::env;
use std::env::VarError;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::channel;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tokio::time::sleep;

#[derive(Clone)]
pub struct ModuleParam {
    pub name: String,
    pub description: String,
    pub mandatory: bool,
}

impl ModuleParam {
    pub fn new(name: &str, description: &str, mandatory: bool) -> ModuleParam {
        ModuleParam {
            name: name.to_string(),
            description: description.to_string(),
            mandatory,
        }
    }
}

pub type MessageResponse = String;
pub type Message = String;

#[derive(Clone)]
pub struct MessageCtx {
    pub content: Message,
    pub id: String,
}

#[async_trait]
pub trait Module {
    fn name(&self) -> &'static str;
    fn params(&self) -> Vec<ModuleParam>;
    async fn module_offering(&mut self, modules: &[ModuleData]);
    fn capabilities(&self) -> ModuleCapabilities;
    async fn run(&mut self, variation: usize) -> Option<Vec<Message>>;
    async fn variation_durations(&mut self) -> Vec<Duration>;
    async fn trigger(&mut self, message: &str) -> Option<Vec<MessageResponse>>;
    async fn send_message(&mut self, messages: &[Message]);
    async fn read_message(&mut self) -> Option<Vec<MessageCtx>>;
    async fn resp_message(&mut self, parent: MessageCtx, message: Message);
}

pub type SharedModule = Arc<RwLock<Box<dyn Module + Send + Sync>>>;

#[derive(Clone, Default)]
pub struct ModuleCapabilities {
    pub triggers: Option<Vec<String>>,
    pub catch_non_triggered: bool,
    pub catch_all: bool,
    pub send_message: bool,
    pub read_message: bool,
    pub resp_message: bool,
}

#[derive(Clone)]
pub struct ModuleData {
    pub module: SharedModule,
    pub name: String,
    pub variation_durations: Vec<Duration>,
    pub params: Vec<ModuleParam>,
    pub capabilities: ModuleCapabilities,
}

impl ModuleData {
    async fn new<M: Module + Send + Sync + 'static>(mut module: M) -> ModuleData {
        let name = String::from(module.name());
        let variation_durations = module.variation_durations().await;
        let params = module.params();
        let capabilities = module.capabilities();
        let module: SharedModule = Arc::new(RwLock::new(Box::new(module)));
        ModuleData {
            module,
            name,
            variation_durations,
            params,
            capabilities,
        }
    }
}

#[derive(Default)]
pub struct Bot {
    modules: Vec<ModuleData>,
}

impl Bot {
    pub async fn new() -> Bot {
        let mut bot = Bot::default();
        bot.register("webex", Webex::new()).await;
        bot.register("ping", Ping::new()).await;
        bot.register("help", Help::new()).await;
        bot.register("down_detectors", DownDetectors::new()).await;
        bot.register("github_orgs", GithubOrgs::new()).await;
        bot.register("github_repos", GithubRepos::new()).await;
        bot.register("triggers", Triggers::new()).await;
        bot.register("hello", Hello::new()).await;
        bot.register("ollama", Ollama::new()).await;
        bot.register("feeds", Feeds::new()).await;
        bot.register("roll", Roll::new()).await;
        bot.register("webpages", Webpages::new()).await;
        bot.register("outscale_api_versions", OutscaleApiVersions::new())
            .await;
        bot
    }

    async fn register<M: Module + Send + Sync + 'static>(
        &mut self,
        module_name: &str,
        module: Result<M, VarError>,
    ) {
        if !Bot::is_module_enabled(module_name) {
            info!("module {} is not enabled", module_name);
            return;
        }
        info!("module {} is enabled", module_name);
        let module = match module {
            Ok(module) => module,
            Err(err) => {
                error!("cannot init module {}: {}", module_name, err);
                return;
            }
        };
        self.modules.push(ModuleData::new(module).await);
    }

    async fn send_modules(&self) {
        for module in self.modules.iter() {
            let mut module_rw = module.module.write().await;
            module_rw.module_offering(&self.modules).await;
        }
    }

    pub async fn help(&self) -> String {
        let mut output = String::new();
        for module in self.modules.iter() {
            output.push_str(format!("# '{}' module parameters\n", module.name).as_str());
            output.push_str(
                format!(
                    "- BOT_MODULE_{}_ENABLED: enable module {} (optional: false)\n",
                    module.name.to_uppercase(),
                    module.name
                )
                .as_str(),
            );
            let param_map = module.params.iter().fold(
                HashMap::<String, ModuleParam>::new(),
                |mut map, param| {
                    map.insert(param.name.clone(), param.clone());
                    map
                },
            );

            for (param_name, param) in param_map.iter() {
                output.push_str(
                    format!(
                        "- {}: {} (mandatory: {})\n",
                        param_name, param.description, param.mandatory
                    )
                    .as_str(),
                );
            }
            output.push('\n');
        }
        output
    }

    pub async fn run(&mut self) {
        if self.modules.is_empty() {
            error!("no module enabled");
            return;
        }
        let (mailbox_tx, mut mailbox_rx) = channel(100);
        self.send_modules().await;
        let mut tasks = JoinSet::new();
        for module in self.modules.iter_mut() {
            for (variation, duration) in module.variation_durations.iter().enumerate() {
                let module = module.clone();
                let duration = *duration;
                let mailbox_tx = mailbox_tx.clone();
                tasks.spawn(tokio::spawn(async move {
                    let module = module.clone();
                    loop {
                        debug!("{}: wait for module write lock", module.name);
                        let mut module_rw = module.module.write().await;
                        debug!("{}: run({})", module.name, variation);
                        if let Some(messages) = module_rw.run(variation).await {
                            if let Err(err) = mailbox_tx.send(messages).await {
                                error!("{}", err);
                            }
                        }
                        drop(module_rw);
                        sleep(duration).await;
                    }
                }));
            }
        }
        let modules = self.modules.clone();
        tasks.spawn(tokio::spawn(async move {
            let modules = modules;
            loop {
                match mailbox_rx.try_recv() {
                    Ok(messages) => {
                        for module in modules.iter() {
                            if module.capabilities.send_message {
                                let mut module_rw = module.module.write().await;
                                module_rw.send_message(&messages).await;
                            }
                        }
                    }
                    Err(_) => {
                        sleep(Duration::from_secs(10)).await;
                    }
                };
            }
        }));
        tasks.join_next().await;
    }

    fn is_module_enabled(module_name: &str) -> bool {
        let env_var_name = format!("BOT_MODULE_{}_ENABLED", module_name.to_uppercase());
        match env::var(&env_var_name) {
            Ok(env_var_value) => {
                trace!(
                    "module {}: env {} is set to '{}'",
                    env_var_name,
                    module_name,
                    env_var_value
                );
                matches!(env_var_value.as_str(), "1" | "true")
            }
            Err(_) => {
                trace!(
                    "{} env variable not defined. Module {} is disabled",
                    env_var_name,
                    module_name
                );
                false
            }
        }
    }
}
