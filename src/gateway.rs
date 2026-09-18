use crate::prelude::*;

pub(crate) struct Gateway {
    pub(crate) name: &'static str,
    pub(crate) base: &'static str,
    pub(crate) needs_key: bool,
}

pub(crate) const GATEWAYS: &[Gateway] = &[
    Gateway {
        name: "AILE Free",
        base: AILE,
        needs_key: false,
    },
    Gateway {
        name: "AILE",
        base: AILE,
        needs_key: true,
    },
    Gateway {
        name: "OpenAI",
        base: "https://api.openai.com/v1",
        needs_key: true,
    },
    Gateway {
        name: "OpenRouter",
        base: "https://openrouter.ai/api/v1",
        needs_key: true,
    },
    Gateway {
        name: "Ollama",
        base: "http://localhost:11434/v1",
        needs_key: false,
    },
    Gateway {
        name: "Custom",
        base: "",
        needs_key: false,
    },
];

pub(crate) fn gateway(name: &str) -> &'static Gateway {
    GATEWAYS
        .iter()
        .find(|g| g.name == name)
        .unwrap_or(&GATEWAYS[0])
}
