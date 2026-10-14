#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AccountColumnId {
    Index,
    Name,
    Platform,
    RunStatus,
    AuthStatus,
    LastOnline,
    Actions,
}

impl AccountColumnId {
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::Index => "index",
            Self::Name => "name",
            Self::Platform => "platform",
            Self::RunStatus => "run-status",
            Self::AuthStatus => "auth-status",
            Self::LastOnline => "last-online",
            Self::Actions => "actions",
        }
    }

    pub(crate) fn title(self, language: &str) -> &'static str {
        let english = language == "en-US";
        match self {
            Self::Index => {
                if english {
                    "Index"
                } else {
                    "序号"
                }
            }
            Self::Name => {
                if english {
                    "Display Name"
                } else {
                    "账号备注"
                }
            }
            Self::Platform => {
                if english {
                    "Platform"
                } else {
                    "平台"
                }
            }
            Self::RunStatus => {
                if english {
                    "Run Status"
                } else {
                    "运行状态"
                }
            }
            Self::AuthStatus => {
                if english {
                    "Authorization"
                } else {
                    "授权状态"
                }
            }
            Self::LastOnline => {
                if english {
                    "Last Online"
                } else {
                    "最近在线"
                }
            }
            Self::Actions => {
                if english {
                    "Operate"
                } else {
                    "操作"
                }
            }
        }
    }

    pub(crate) const fn width(self) -> f32 {
        match self {
            Self::Index => 44.,
            Self::Name => 220.,
            Self::Platform => 90.,
            Self::RunStatus => 112.,
            Self::AuthStatus => 142.,
            Self::LastOnline => 160.,
            Self::Actions => 282.,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum AccountColumnPin {
    Left,
    Right,
    #[default]
    Unfixed,
}

impl AccountColumnPin {
    pub(crate) const fn next(self) -> Self {
        match self {
            Self::Unfixed => Self::Left,
            Self::Left => Self::Right,
            Self::Right => Self::Unfixed,
        }
    }

    pub(crate) fn label(self, language: &str) -> &'static str {
        let english = language == "en-US";
        match self {
            Self::Unfixed => {
                if english {
                    "Pin left"
                } else {
                    "固定左侧"
                }
            }
            Self::Left => {
                if english {
                    "Pin right"
                } else {
                    "固定右侧"
                }
            }
            Self::Right => {
                if english {
                    "Unpin"
                } else {
                    "取消固定"
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AccountColumnSetting {
    pub(crate) id: AccountColumnId,
    pub(crate) visible: bool,
    pub(crate) pin: AccountColumnPin,
}

impl AccountColumnSetting {
    const fn new(id: AccountColumnId) -> Self {
        Self { id, visible: true, pin: AccountColumnPin::Unfixed }
    }
}

pub(crate) const fn default_account_columns() -> [AccountColumnSetting; 7] {
    [
        AccountColumnSetting::new(AccountColumnId::Index),
        AccountColumnSetting::new(AccountColumnId::Name),
        AccountColumnSetting::new(AccountColumnId::Platform),
        AccountColumnSetting::new(AccountColumnId::RunStatus),
        AccountColumnSetting::new(AccountColumnId::AuthStatus),
        AccountColumnSetting::new(AccountColumnId::LastOnline),
        AccountColumnSetting {
            id: AccountColumnId::Actions,
            visible: true,
            pin: AccountColumnPin::Right,
        },
    ]
}

pub(crate) fn move_account_column(
    columns: &mut [AccountColumnSetting],
    index: usize,
    direction: isize,
) {
    let Some(target) = index.checked_add_signed(direction) else { return };
    if target < columns.len() {
        columns.swap(index, target);
    }
}

#[cfg(test)]
mod tests {
    use super::{default_account_columns, move_account_column, AccountColumnId, AccountColumnPin};

    #[test]
    fn column_settings_match_vue_defaults_and_reordering_bounds() {
        let mut columns = default_account_columns();
        assert!(columns.iter().all(|column| column.visible));
        assert!(columns[..6].iter().all(|column| column.pin == AccountColumnPin::Unfixed));
        assert_eq!(columns[6].id, AccountColumnId::Actions);
        assert_eq!(columns[6].pin, AccountColumnPin::Right);

        move_account_column(&mut columns, 1, -1);
        assert_eq!(columns[0].id.title("zh-CN"), "账号备注");
        move_account_column(&mut columns, 0, -1);
        assert_eq!(columns[0].id.title("zh-CN"), "账号备注");
        move_account_column(&mut columns, 0, 1);
        assert_eq!(columns[1].id.title("zh-CN"), "账号备注");
    }

    #[test]
    fn column_pin_cycle_matches_vue_left_right_unfixed_order() {
        let pin = AccountColumnPin::Unfixed;
        assert_eq!(pin.next(), AccountColumnPin::Left);
        assert_eq!(pin.next().next(), AccountColumnPin::Right);
        assert_eq!(pin.next().next().next(), AccountColumnPin::Unfixed);
    }
}
