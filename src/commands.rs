// commands.rs
use regex::Regex;

#[derive(Debug, Clone)]
pub struct CommandProcessor {
    bot_mention_regex: Regex,
}

impl CommandProcessor {
    pub fn new() -> Self {
        // Matches @bot followed by a command
        let bot_mention_regex = Regex::new(r"@bot\s+(\w+)").unwrap();

        Self { bot_mention_regex }
    }

    pub fn parse_command(&self, comment_body: &str) -> Option<String> {
        if let Some(captures) = self.bot_mention_regex.captures(comment_body) {
            if let Some(command) = captures.get(1) {
                return Some(command.as_str().to_lowercase());
            }
        }
        None
    }
}
