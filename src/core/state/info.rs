use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use result::OptionResultExt;
use serde::{
    de::{value::MapAccessDeserializer, Visitor},
    Deserialize, Serialize,
};

use crate::{core::context::TextContext, text::templating::TemplatableString};

#[derive(Deserialize, Serialize, Debug)]
pub struct InfoContents {
    name: TemplatableString,
    #[serde(rename = "as")]
    as_name: Option<TemplatableString>,
}

#[derive(Debug)]
pub struct InfoApplication {
    info: InfoContents,
}

pub type InfoApplications = Vec<InfoApplication>;
pub type InfoPages = BTreeMap<String, String>;

pub struct InfoApplicationVisitor;

impl<'de> Visitor<'de> for InfoApplicationVisitor {
    type Value = InfoContents;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("string or map")
    }

    fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(InfoContents {
            name: v.to_owned().into(),
            as_name: None,
        })
    }

    fn visit_map<A>(self, map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        InfoContents::deserialize(MapAccessDeserializer::new(map))
    }
}

impl<'de> Deserialize<'de> for InfoApplication {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(InfoApplication {
            info: deserializer.deserialize_any(InfoApplicationVisitor)?,
        })
    }
}

impl Serialize for InfoApplication {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match &self.info.as_name {
            None => self.info.name.serialize(serializer),
            Some(_) => self.info.serialize(serializer),
        }
    }
}

impl InfoApplication {
    pub fn to_unlocked(&self, text_context: &TextContext) -> Result<UnlockedInfoPage> {
        let name = self.info.name.fill(text_context)?;
        let result = UnlockedInfoPage {
            name: name.clone(),
            as_name: self
                .info
                .as_name
                .as_ref()
                .map(|s| s.fill(text_context))
                .invert()?
                .unwrap_or(name),
        };
        Ok(result)
    }

    fn validate(&self, pages: &InfoPages) -> Result<()> {
        if let Some(page) = self.info.name.content() {
            if !pages.contains_key(page) {
                return Err(anyhow!("Invalid info page '{page}'"));
            }
        }
        Ok(())
    }

    pub fn validate_all(apps: &InfoApplications, pages: &InfoPages) -> Result<()> {
        for (index, app) in apps.iter().enumerate() {
            app.validate(pages)
                .with_context(|| format!("Failed to validate info application #{}", index + 1))?;
        }
        Ok(())
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct UnlockedInfoPage {
    pub name: String,
    #[serde(rename = "as")]
    pub as_name: String,
}

pub type UnlockedInfoPages = Vec<UnlockedInfoPage>;
