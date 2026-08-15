use crate::error::Error;

/// 活动业务错误码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityErrorCode {
    ShopUnavailable,
    ShopResponseInvalid,
    ShopGoodsNotFound,
    ShopGoodsUnavailable,
    ShopBalanceUnavailable,
    InsufficientStarSand,
    InvalidShopGoodsId,
    InvalidExchangeCount,
    InvalidSolarTermId,
    SeasonDataEmpty,
    ConstellationActivityMissing,
    InvalidQingmeiUid,
    InvalidQingmeiCount,
    InvalidQingmeiIngredients,
    DuplicateQingmeiUid,
    InsufficientQingmei,
}

impl ActivityErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ShopUnavailable => "SHOP_UNAVAILABLE",
            Self::ShopResponseInvalid => "SHOP_RESPONSE_INVALID",
            Self::ShopGoodsNotFound => "SHOP_GOODS_NOT_FOUND",
            Self::ShopGoodsUnavailable => "SHOP_GOODS_UNAVAILABLE",
            Self::ShopBalanceUnavailable => "SHOP_BALANCE_UNAVAILABLE",
            Self::InsufficientStarSand => "INSUFFICIENT_STAR_SAND",
            Self::InvalidShopGoodsId => "INVALID_SHOP_GOODS_ID",
            Self::InvalidExchangeCount => "INVALID_EXCHANGE_COUNT",
            Self::InvalidSolarTermId => "INVALID_SOLAR_TERM_ID",
            Self::SeasonDataEmpty => "SEASON_DATA_EMPTY",
            Self::ConstellationActivityMissing => "CONSTELLATION_ACTIVITY_MISSING",
            Self::InvalidQingmeiUid => "INVALID_QINGMEI_UID",
            Self::InvalidQingmeiCount => "INVALID_QINGMEI_COUNT",
            Self::InvalidQingmeiIngredients => "INVALID_QINGMEI_INGREDIENTS",
            Self::DuplicateQingmeiUid => "DUPLICATE_QINGMEI_UID",
            Self::InsufficientQingmei => "INSUFFICIENT_QINGMEI",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActivityError {
    pub code: ActivityErrorCode,
    pub message: String,
}

impl std::fmt::Display for ActivityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for ActivityError {}

impl From<ActivityError> for Error {
    fn from(e: ActivityError) -> Self {
        Error::Business(e.to_string())
    }
}
