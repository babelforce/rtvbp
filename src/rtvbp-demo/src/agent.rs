use openai_realtime::{AgentConfig, Voice};

#[derive(Debug, Clone, clap::Parser)]
pub struct AgentCliArgs {
    #[clap(long = "agent-speed", default_value = "1.2")]
    pub speed: f32,

    #[clap(long = "agent-voice", default_value = "alloy")]
    pub voice: Voice,

    #[clap(
        long = "agent-prompt",
        default_value = "You are a nice and friendly person wanting to have a nice conversation"
    )]
    pub prompt: String,

    #[clap(long = "agent-lang", default_value = "en-US")]
    pub lang: String,

    #[clap(long = "agent-create-response", default_value = "false")]
    pub create_response: bool,
}

impl Default for AgentCliArgs {
    fn default() -> Self {
        Self {
            speed: 1.2,
            voice: Voice::Alloy,
            prompt: "You are a nice and friendly person wanting to have a nice conversation".into(),
            lang: "en-US".into(),
            create_response: false,
        }
    }
}

impl Into<AgentConfig> for AgentCliArgs {
    fn into(self) -> AgentConfig {
        AgentConfig {
            speed: self.speed.into(),
            voice: self.voice.into(),
            instructions: format!(
                r###"
# ROLEPLAY

This is a roleplay. Play along with it.

**Instruction**

{}

**Rules**

- Your spoken language is: {}"###,
                self.prompt, self.lang,
            )
            .into(),
            ..Default::default()
        }
    }
}
