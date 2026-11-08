/// Selects the recovered executable-era behavior independently of save-file format.
/// `Win95` has the broader recovered simulation contract. `Mac68k` selects
/// recovered Mac subcontracts, including portions of its monthly dispatcher,
/// but whole-session scheduling and several simulation families remain
/// incomplete.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EngineFidelity {
    Win95,
    Mac68k,
}

impl EngineFidelity {
    pub(crate) fn from_env() -> Self {
        match std::env::var("SC2000_ENGINE_FIDELITY")
            .ok()
            .map(|value| value.trim().to_ascii_lowercase())
            .as_deref()
        {
            Some("mac") | Some("mac68k") | Some("mac-68k") | Some("mac_68k") => Self::Mac68k,
            _ => Self::Win95,
        }
    }

    pub(crate) fn uses_win95(self) -> bool {
        matches!(self, Self::Win95)
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Win95 => "win95",
            Self::Mac68k => "mac68k",
        }
    }
}
