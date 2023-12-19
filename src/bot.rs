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
use crate::webpages::Webpages;
use async_trait::async_trait;
use log::{error, trace};
use std::collections::HashMap;
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
    async fn module_offering(&mut self, modules: &[ModuleData]);
    async fn has_needed_params(&self) -> bool;
    async fn run(&mut self, variation: usize); // alternative to `variation`?
    async fn variation_durations(&mut self) -> Vec<Duration>;
    async fn trigger(&mut self, message: &str, id: &str);
}

pub type SharedModule = Arc<RwLock<Box<dyn Module + Send + Sync>>>;

#[derive(Clone)]
pub struct ModuleData {
    pub module: SharedModule,
    pub name: String,
    pub variation_durations: Vec<Duration>,
    pub params: Vec<ModuleParam>,
}

impl ModuleData {
    async fn new<M: Module + Send + Sync + 'static>(mut module: M) -> ModuleData {
        let name = String::from(module.name());
        let variation_durations = module.variation_durations().await;
        let params = module.params();
        let module: SharedModule = Arc::new(RwLock::new(Box::new(module)));
        ModuleData {
            module,
            name,
            variation_durations,
            params,
        }
    }
}

pub struct Bot {
    modules: Vec<ModuleData>,
}

impl Bot {
    pub async fn new() -> Bot {
        let mut bot = Bot {
            modules: Vec::new(),
        };
        if Bot::is_module_enabled("ping") {
            bot.modules
                .push(ModuleData::new(Ping::new().unwrap()).await);
        }
        if Bot::is_module_enabled("help") {
            bot.modules
                .push(ModuleData::new(Help::new().unwrap()).await);
        }
        if Bot::is_module_enabled("triggers") {
            bot.modules
                .push(ModuleData::new(Triggers::new().unwrap()).await);
        }
        if Bot::is_module_enabled("down_detectors") {
            bot.modules
                .push(ModuleData::new(DownDetectors::new().unwrap()).await);
        }
        if Bot::is_module_enabled("github_orgs") {
            bot.modules
                .push(ModuleData::new(GithubOrgs::new().unwrap()).await);
        }
        if Bot::is_module_enabled("github_repos") {
            bot.modules
                .push(ModuleData::new(GithubRepos::new().unwrap()).await);
        }
        if Bot::is_module_enabled("hello") {
            bot.modules
                .push(ModuleData::new(Hello::new().unwrap()).await);
        }
        if Bot::is_module_enabled("ollama") {
            bot.modules
                .push(ModuleData::new(Ollama::new().unwrap()).await);
        }
        if Bot::is_module_enabled("feeds") {
            bot.modules
                .push(ModuleData::new(Feeds::new().unwrap()).await);
        }
        if Bot::is_module_enabled("roll") {
            bot.modules
                .push(ModuleData::new(Roll::new().unwrap()).await);
        }
        if Bot::is_module_enabled("webpages") {
            bot.modules
                .push(ModuleData::new(Webpages::new().unwrap()).await);
        }
        if Bot::is_module_enabled("outscale_api_versions") {
            bot.modules
                .push(ModuleData::new(OutscaleApiVersions::new().unwrap()).await);
        }
        bot
    }

    pub async fn ready(&mut self) -> bool {
        let mut ret = true;
        for module in self.modules.iter() {
            if module.params.is_empty() {
                continue;
            }
            for param in module.params.iter() {
                if param.optional {
                    continue;
                }
                match env::var(&param.name) {
                    Ok(value) => {
                        if value.is_empty() {
                            error!(
                                "module {} need mandatory environment variable {}",
                                module.name, param.name
                            );
                            ret = false;
                        }
                    }
                    Err(_) => {
                        error!(
                            "module {} need mandatory environment variable {}",
                            module.name, param.name
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
                        "- {}: {} (optional: {})\n",
                        param_name, param.description, param.optional
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
        self.send_modules().await;
        let mut tasks = JoinSet::new();
        for module in self.modules.iter_mut() {
            for (variation, duration) in module.variation_durations.iter().enumerate() {
                let module = module.clone();
                let duration = *duration;
                tasks.spawn(tokio::spawn(async move {
                    let module = module.clone();
                    loop {
                        trace!("get module {} lock ...", module.name);
                        let mut module_rw = module.module.write().await;
                        trace!("module {} lock aquired", module.name);
                        trace!("module {} run variation {}", module.name, variation);
                        module_rw.run(variation).await;
                        drop(module_rw);
                        trace!(
                            "module {} run variation {} is now sleeping for {:#?}",
                            module.name,
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
