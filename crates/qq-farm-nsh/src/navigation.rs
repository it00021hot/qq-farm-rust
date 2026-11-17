#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Section {
    Dashboard,
    Farm,
    Friends,
    Ranch,
    Collections,
    Activities,
    Analytics,
    Mall,
    Settings,
    Config,
    Accounts,
}

pub(crate) const SECTIONS: [Section; 11] = [
    Section::Dashboard,
    Section::Farm,
    Section::Friends,
    Section::Ranch,
    Section::Collections,
    Section::Activities,
    Section::Analytics,
    Section::Mall,
    Section::Settings,
    Section::Config,
    Section::Accounts,
];

impl Section {
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Dashboard => "dashboard",
            Self::Farm => "farm",
            Self::Friends => "friends",
            Self::Ranch => "ranch",
            Self::Collections => "collections",
            Self::Activities => "activities",
            Self::Analytics => "analytics",
            Self::Mall => "mall",
            Self::Settings => "settings",
            Self::Config => "config",
            Self::Accounts => "accounts",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        SECTIONS.into_iter().find(|section| section.id() == id)
    }

    pub(crate) const fn label_key(self) -> &'static str {
        match self {
            Self::Dashboard => "nav.dashboard",
            Self::Farm => "nav.farm",
            Self::Friends => "nav.friends",
            Self::Ranch => "nav.ranch",
            Self::Collections => "nav.collections",
            Self::Activities => "nav.activities",
            Self::Analytics => "nav.analytics",
            Self::Mall => "nav.mall",
            Self::Settings => "nav.settings",
            Self::Config => "nav.config",
            Self::Accounts => "nav.accounts",
        }
    }

    pub(crate) const fn detail_key(self) -> Option<&'static str> {
        match self {
            Self::Dashboard
            | Self::Farm
            | Self::Friends
            | Self::Analytics
            | Self::Mall
            | Self::Activities
            | Self::Settings
            | Self::Config
            | Self::Ranch
            | Self::Collections
            | Self::Accounts => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Section;

    #[test]
    fn unknown_and_empty_ids_are_absent() {
        assert_eq!(Section::from_id(""), None);
        assert_eq!(Section::from_id("weather"), None);
    }

    #[test]
    fn each_section_id_round_trips() {
        for section in super::SECTIONS {
            assert_eq!(Section::from_id(section.id()), Some(section));
        }
    }
}
