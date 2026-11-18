use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub models: Vec<Model>,
    pub selected: Option<String>,
    pub auto_approve: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub url: String,
    pub model: String,
    #[serde(default)]
    pub tools: bool,
}

impl Model {
    pub fn endpoint(&self) -> String {
        let url = self.url.trim_end_matches('/');
        if url.ends_with("/chat/completions") {
            url.into()
        } else {
            format!("{url}/chat/completions")
        }
    }

    pub fn credential(&self) -> String {
        format!("elyterminal.ai/{}/{}", self.id, self.url)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.id.is_empty()
                && self.id.len() <= 64
                && self
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "ai: invalid model id"
        );
        ensure!(
            !self.name.trim().is_empty() && self.name.len() <= 128,
            "ai: model name must contain 1–128 bytes"
        );
        ensure!(
            !self.model.trim().is_empty() && self.model.len() <= 256,
            "ai: model identifier must contain 1–256 bytes"
        );
        let host = self
            .url
            .strip_prefix("https://")
            .or_else(|| self.url.strip_prefix("http://"));
        ensure!(
            host.is_some_and(|host| !host.is_empty() && !host.starts_with('/')),
            "ai: service address must start with http:// or https://"
        );
        ensure!(
            self.url.len() <= 2048
                && !self
                    .url
                    .chars()
                    .any(|c| c.is_control() || c.is_whitespace())
                && !self.url.contains(['@', '#', '?']),
            "ai: service address cannot contain credentials, whitespace, a query or a fragment"
        );
        Ok(())
    }
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.models.len() <= 32,
            "ai: at most 32 model configurations"
        );
        let mut ids = std::collections::HashSet::new();
        for model in &self.models {
            model.validate()?;
            ensure!(ids.insert(&model.id), "ai: duplicate model id");
        }
        ensure!(
            self.selected.as_ref().is_none_or(|id| ids.contains(id)),
            "ai: selected model does not exist"
        );
        Ok(())
    }

    pub fn selected(&self) -> Option<&Model> {
        self.selected
            .as_ref()
            .and_then(|id| self.models.iter().find(|model| &model.id == id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_and_public_settings_reject_hidden_credentials() {
        let defaults: Settings = serde_json::from_str("{}").unwrap();
        assert!(!defaults.auto_approve);
        let automatic = Settings {
            auto_approve: true,
            ..defaults
        };
        assert!(
            serde_json::from_str::<Settings>(&serde_json::to_string(&automatic).unwrap())
                .unwrap()
                .auto_approve
        );
        let mut model = Model {
            id: "local".into(),
            name: "Local".into(),
            url: "http://127.0.0.1:11434/v1/".into(),
            model: "test".into(),
            tools: false,
        };
        assert!(model.validate().is_ok());
        assert_eq!(
            model.endpoint(),
            "http://127.0.0.1:11434/v1/chat/completions"
        );
        model.url.push_str("chat/completions");
        assert_eq!(model.endpoint(), model.url);
        for url in [
            "file:///secret",
            "https://key@host/v1",
            "https://host/v1?key=secret",
            "https://host/\nheader",
        ] {
            model.url = url.into();
            assert!(model.validate().is_err(), "{url:?}");
        }
        assert!(serde_json::from_str::<Settings>(r#"{"api_key":"secret"}"#).is_err());
    }
}
