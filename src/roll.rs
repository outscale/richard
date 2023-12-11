use std::env::VarError;

use log::trace;
use rand::Rng;

use crate::webex::WebexAgent;

#[derive(Clone)]
pub struct Roll {
    webex: WebexAgent,
}

impl Roll {
    pub fn new() -> Result<Self, VarError> {
        Ok(Roll {
            webex: WebexAgent::new()?,
        })
    }

    pub async fn run_trigger(&mut self, message: &str, parent_message: &str) {
        if !message.contains("roll") {
            return;
        }
        let response = Roll::gen(&message.into()).unwrap_or(Roll::help().into());
        self.webex.respond(parent_message, &response).await;
    }

    fn gen(request: &String) -> Option<String> {
        trace!("asking to roll {}", request);
        let first_item_after_roll = request.split("roll").nth(1)?;
        let dices = first_item_after_roll.split(' ').nth(1)?;
        trace!("dices: {}", dices);

        let mut iter = dices.split('d');
        let count_str = iter.next()?;
        trace!("dice count: {}", count_str);
        let faces_str = iter.next()?;
        trace!("faces: {}", faces_str);
        let count = count_str.parse::<usize>().ok()?;
        let faces = faces_str.parse::<usize>().ok()?;

        if count == 0 || count > 1_000 || faces == 0 || faces > 1000 {
            return None;
        }

        let mut rng = rand::thread_rng();
        let mut total = 0;
        let mut output = format!("roll {}d{}: ", count, faces);
        if count > 1 && count < 100 {
            output.push('(');
        }
        for _ in 0..count {
            let roll = rng.gen_range(1..faces + 1);
            if count > 1 && count < 100 {
                output.push_str(format!("{}+", roll).as_str());
            }
            total += roll;
        }
        if count > 1 && count < 100 {
            output.pop();
            output.push_str(") = ");
        }
        output.push_str(format!("{}", total).as_str());
        trace!("roll result message: {}", output);
        Some(output)
    }

    fn help() -> &'static str {
        "roll <dices> : roll one or more dices where '<dice>' is formated like 1d20."
    }
}
