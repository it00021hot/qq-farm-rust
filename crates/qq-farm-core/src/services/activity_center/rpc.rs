use prost::Message;

use crate::constants::ACTIVITY_SERVICE;
use crate::error::Result;
use crate::proto::generated::gamepb::activitypb::{ActivityOperateReply, QueryActivityRequest};

use super::ActivityCenterService;

impl ActivityCenterService {
    // ----- 活动通用（Query / Operate） -----

    /// 通用活动查询
    pub async fn query_activity(
        &self,
        activity_id: i64,
        operate_type: i64,
    ) -> Result<ActivityOperateReply> {
        let req = QueryActivityRequest { activity_id, operate_type };
        let body =
            self.gateway.request(ACTIVITY_SERVICE, "Operate", &req.encode_to_vec(), 10_000).await?;
        Ok(ActivityOperateReply::decode(&body[..])?)
    }

    /// 通用活动操作
    pub async fn operate_activity(
        &self,
        activity_id: i64,
        operate_type: i64,
    ) -> Result<ActivityOperateReply> {
        // Operate 和 QueryActivity 都走 "Operate" 方法（proto 不区分）
        self.query_activity(activity_id, operate_type).await
    }
}
