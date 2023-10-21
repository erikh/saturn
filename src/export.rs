use anyhow::{anyhow, Result};

pub enum ExportFormat {
    JSON,
    YAML,
    CBOR,
}

impl std::str::FromStr for ExportFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(Self::JSON),
            "yaml" => Ok(Self::YAML),
            "cbor" => Ok(Self::CBOR),
            _ => Err(anyhow!(
                "Invalid export format type; must be one of [json, yaml, cbor]"
            )),
        }
    }
}

impl std::fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::JSON => "json",
            Self::YAML => "yaml",
            Self::CBOR => "cbor",
        })
    }
}

pub fn export(
    w: &mut impl std::io::Write,
    records: Vec<crate::record::Record>,
    format: ExportFormat,
) -> Result<()> {
    match format {
        ExportFormat::JSON => serde_json::to_writer(w, &records)?,
        ExportFormat::YAML => serde_yaml::to_writer(w, &records)?,
        ExportFormat::CBOR => ciborium::into_writer(&records, w)?,
    }

    Ok(())
}
