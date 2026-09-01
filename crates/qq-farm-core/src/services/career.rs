//! 生涯服务 —— 生涯收获 / 生涯偷菜统计。
//!
//! 对应原 `core/src/services/career.ts`：`CareerService.CareerInfoGet` 可查任意
//! 角色（自己或好友）；查询失败不阻断土地读取（返回 None）。

use std::sync::Arc;

use prost::Message as _;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::careerpb::{CareerInfoGetReply, CareerInfoGetRequest};

/// 生涯统计摘要（对齐 bot `CareerInfo`）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CareerInfo {
    pub gid: i64,
    pub harvest: i64,
    pub steal: i64,
    pub level: i64,
    pub name: String,
}

/// 查询生涯统计
pub async fn get_career_info(gateway: &Arc<Gateway>, gid: i64) -> Result<CareerInfo> {
    if gid <= 0 {
        return Err(Error::Business("缺少有效的角色 GID".to_string()));
    }
    let body = CareerInfoGetRequest { gid }.encode_to_vec();
    let resp =
        gateway.request("gamepb.careerpb.CareerService", "CareerInfoGet", &body).await?;
    let reply = CareerInfoGetReply::decode(&*resp).map_err(Error::from)?;
    Ok(CareerInfo {
        gid: if reply.gid != 0 { reply.gid } else { gid },
        harvest: reply.total_harvest_count,
        steal: reply.total_steal_count,
        level: reply.level,
        name: reply.name,
    })
}

/// 查询生涯统计；失败仅打 warn 返回 None（不阻断土地读取）
pub async fn get_career_info_or_null(gateway: &Arc<Gateway>, gid: i64) -> Option<CareerInfo> {
    match get_career_info(gateway, gid).await {
        Ok(info) => Some(info),
        Err(e) => {
            tracing::warn!(error = %e, "生涯查询失败");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn career_info_camel_case() {
        let info = CareerInfo {
            gid: 123,
            harvest: 4567,
            steal: 89,
            level: 30,
            name: "测试".to_string(),
        };
        let v = serde_json::to_value(&info).expect("json");
        assert_eq!(v["harvest"], 4567);
        assert_eq!(v["steal"], 89);
        assert_eq!(v["name"], "测试");
    }
}
