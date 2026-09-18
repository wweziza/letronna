use crate::prelude::*;

pub(crate) struct Gateway {
    pub(crate) name: &'static str,
    pub(crate) base: &'static str,
    pub(crate) needs_key: bool,
    pub(crate) blurb: &'static str,
}

pub(crate) const GATEWAYS: &[Gateway] = &[
    Gateway {
        name: "AILE Free",
        base: AILE,
        needs_key: false,
        blurb: "Guest access to the AILE catalog. No key, a few replies per session.",
    },
    Gateway {
        name: "AILE",
        base: AILE,
        needs_key: true,
        blurb: "Your AILE account with an API key. Full catalog and quota.",
    },
    Gateway {
        name: "OpenAI",
        base: "https://api.openai.com/v1",
        needs_key: true,
        blurb: "OpenAI directly. Needs an OpenAI API key.",
    },
    Gateway {
        name: "OpenRouter",
        base: "https://openrouter.ai/api/v1",
        needs_key: true,
        blurb: "Hundreds of models behind one key.",
    },
    Gateway {
        name: "Ollama",
        base: "http://localhost:11434/v1",
        needs_key: false,
        blurb: "Local models on this machine. Works offline.",
    },
    Gateway {
        name: "Custom",
        base: "",
        needs_key: false,
        blurb: "Any OpenAI-compatible server. Paste the base URL.",
    },
];

pub(crate) fn gateway(name: &str) -> &'static Gateway {
    GATEWAYS
        .iter()
        .find(|g| g.name == name)
        .unwrap_or(&GATEWAYS[0])
}
