use crate::down_detectors::DownDetectors;
use crate::feeds::Feeds;
use crate::github_orgs::GithubOrgs;
use crate::github_repos::GithubRepos;
use crate::hello::Hello;
use crate::help::Help;
use crate::outscale_api_versions::OutscaleApiVersions;
use crate::ping::Ping;
use crate::roll::Roll;
use crate::triggers::Triggers;
use crate::webex::Webex;
use crate::webpages::Webpages;
use async_trait::async_trait;
use log::{error, info, trace};
use std::collections::HashMap;
use std::env;
use std::env::VarError;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::channel;
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
    fn variation_durations(&self) -> Vec<Duration>;
    fn capabilities(&self) -> ModuleCapabilities;

    async fn module_offering(&self, modules: &[ModuleData]);
    async fn run(&self, variation: usize) -> Option<Vec<Message>>;
    async fn trigger(&self, message: &str) -> Option<Vec<MessageResponse>>;
    async fn send_message(&self, messages: &[Message]);
    async fn read_message(&self) -> Option<Vec<MessageCtx>>;
    async fn resp_message(&self, parent: MessageCtx, message: Message);
}

pub type SharedModule = Arc<Box<dyn Module + Send + Sync>>;

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
    pub name: &'static str,
    pub variation_durations: Vec<Duration>,
    pub params: Vec<ModuleParam>,
    pub capabilities: ModuleCapabilities,
}

impl ModuleData {
    fn new<M: Module + Send + Sync + 'static>(module: M) -> ModuleData {
        let name = module.name();
        let variation_durations = module.variation_durations();
        let params = module.params();
        let capabilities = module.capabilities();
        let module: SharedModule = Arc::new(Box::new(module));
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
    pub fn new() -> Self {
        Bot::default()
            .register("webex", Webex::new())
            .register("ping", Ping::new())
            .register("help", Help::new())
            .register("down_detectors", DownDetectors::new())
            .register("github_orgs", GithubOrgs::new())
            .register("github_repos", GithubRepos::new())
            .register("triggers", Triggers::new())
            .register("hello", Hello::new())
            .register("feeds", Feeds::new())
            .register("roll", Roll::new())
            .register("webpages", Webpages::new())
            .register("outscale_api_versions", OutscaleApiVersions::new())
    }

    fn register<M: Module + Send + Sync + 'static>(
        mut self,
        module_name: &str,
        module: Result<M, VarError>,
    ) -> Self {
        if !Bot::is_module_enabled(module_name) {
            info!("module {} is not enabled", module_name);
            return self;
        }
        info!("module {} is enabled", module_name);
        let module = match module {
            Ok(module) => module,
            Err(err) => {
                panic!("cannot init module {}: {}", module_name, err);
            }
        };
        self.modules.push(ModuleData::new(module));
        self
    }

    async fn send_modules(&self) {
        for module in self.modules.iter() {
            module.module.module_offering(&self.modules).await;
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

    pub async fn run(mut self) {
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
                tasks.spawn(async move {
                    let module = module.clone();
                    loop {
                        if let Some(messages) = module.module.run(variation).await {
                            if let Err(err) = mailbox_tx.send(messages).await {
                                error!("{}", err);
                            }
                        }
                        sleep(duration).await;
                    }
                });
            }
        }
        let modules = self.modules.clone();
        tasks.spawn(async move {
            let modules = modules;
            loop {
                while let Some(messages) = mailbox_rx.recv().await {
                    for module in modules.iter() {
                        if module.capabilities.send_message {
                            module.module.send_message(&messages).await;
                        }
                    }
                }
            }
        });
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
