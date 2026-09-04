use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Manufacturing policy selected by callers.  It controls release-package
/// expectations rather than attempting to model a fabricator's full DFM rule
/// book.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ManufacturingProfile {
    #[default]
    Generic,
    Jlcpcb,
}

impl ManufacturingProfile {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "generic" => Some(Self::Generic),
            "jlcpcb" | "jlc-pcb" | "jlc" => Some(Self::Jlcpcb),
            _ => None,
        }
    }

    pub fn rules(self) -> ProfileRules {
        match self {
            Self::Generic => ProfileRules {
                name: "generic".to_owned(),
                require_board_outline: true,
                require_drill: true,
                require_bom_value: true,
                require_bom_footprint: false,
                require_cpl: false,
                require_cpl_coordinates: false,
                minimum_copper_layers: 2,
            },
            Self::Jlcpcb => ProfileRules {
                name: "jlcpcb".to_owned(),
                require_board_outline: true,
                require_drill: true,
                require_bom_value: true,
                require_bom_footprint: true,
                require_cpl: true,
                require_cpl_coordinates: true,
                minimum_copper_layers: 2,
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct ProfileRules {
    pub name: String,
    pub require_board_outline: bool,
    pub require_drill: bool,
    pub require_bom_value: bool,
    pub require_bom_footprint: bool,
    pub require_cpl: bool,
    pub require_cpl_coordinates: bool,
    pub minimum_copper_layers: u8,
}
