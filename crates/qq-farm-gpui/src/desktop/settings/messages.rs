pub(in crate::desktop) fn settings_text(language: &str, chinese: &str) -> String {
    if language != "en-US" {
        return chinese.to_string();
    }
    let english = match chinese {
        "系统设置" => "System settings",
        "跟随系统" => "System",
        "浅色" => "Light",
        "深色" => "Dark",
        "编辑设置" => "Edit setting",
        "编辑" => "Edit",
        "设置值" => "Setting value",
        "取消" => "Cancel",
        "应用" => "Apply",
        "已开启" => "On",
        "已关闭" => "Off",
        "自动控制已保存" => "Automation settings saved",
        "设置已保存，但化肥检查失败。" => "Settings saved, but the fertilizer check failed.",
        "请选择账号后编辑自动化策略。" => "Select an account to edit automation settings.",
        "保存自动控制" => "Save Automation",
        "基础自动化" => "Basic automation",
        "自动种植收获" => "Auto Plant & Harvest",
        "推送触发巡田" => "Push-triggered Farm Check",
        "自动升级土地" => "Auto Upgrade Land",
        "自动做任务" => "Auto Tasks",
        "自动卖果实" => "Auto Sell Fruit",
        "自动好友互动" => "Auto Friend Actions",
        "巡田时跳过一键务农" => "Skip one-click farm care on check",
        "施肥与购买" => "Fertilizer and purchases",
        "自动填充化肥" => "Auto Fill Fertilizer",
        "自动购买有机化肥" => "Auto Buy Organic Fertilizer",
        "自动购买无机化肥" => "Auto Buy Inorganic Fertilizer",
        "多季补肥" => "Multi-season Refertilize",
        "施肥范围" => "Fertilizer Scope",
        "紫金土地" => "Purple-Gold Land",
        "金土地" => "Gold Land",
        "黑土地" => "Black Land",
        "红土地" => "Red Land",
        "普通土地" => "Normal Land",
        "施肥策略" => "Fertilizer Strategy",
        "普通 + 有机" => "Regular + organic",
        "智能" => "Smart",
        "普通" => "Regular",
        "有机" => "Organic",
        "不施肥" => "Do not fertilize",
        "快成熟判定秒数" => "Fast-mature Threshold (sec)",
        "购买数量" => "Buy Count",
        "触发阈值 (小时)" => "Trigger Threshold (h)",
        "自动偷菜" => "Auto Steal",
        "自动帮忙" => "Auto Help",
        "自动捣乱" => "Auto Sabotage",
        "经验满不帮忙" => "No Help at Exp Cap",
        "好友申请自动通过" => "Auto accept friend requests",
        "最低等级" => "Min level",
        "0 = 不限" => "0 = no limit",
        "不低于自己等级" => "Not below own level",
        "收偷比过滤" => "Steal/harvest ratio filter",
        "收获份数" => "Harvest count",
        "偷菜份数" => "Steal count",
        "神秘商人" => "Mystery merchant",
        "神秘商人自动购买" => "Mystery Shop Auto Buy",
        "神秘商人到货通知" => "Arrival Notify",
        "购买成功通知" => "Purchase Notify",
        "允许用金币购买" => "Allow Gold",
        "允许用点券购买" => "Allow Coupon",
        "允许用金豆豆购买" => "Allow Gold Bean",
        "允许用钻石购买" => "Allow Diamond",
        "种植策略" => "Planting strategy",
        "指定种子" => "Preferred seed",
        "等级优先" => "Prioritize level",
        "最高经验" => "Highest experience",
        "化肥经验" => "Fertilizer experience",
        "最高收益" => "Highest profit",
        "化肥收益" => "Fertilizer profit",
        "背包优先" => "Prioritize inventory seeds",
        "自动选择" => "Automatic selection",
        "背包种子优先顺序" => "Bag Seed Priority Order",
        "第二优先策略" => "Secondary Strategy",
        "重置顺序" => "Reset Order",
        "多格种子预留空地" => "Reserve Lands for Multi-Land Seeds",
        "正在读取背包种子…" => "Loading inventory seeds…",
        "优先种植顺序（拖动或使用 ↑ ↓ 调整）" => {
            "Planting priority (drag rows or use ↑ ↓ to reorder)"
        }
        "当前没有可排序的背包种子。" => "There are no inventory seeds to reorder.",
        "种子" => "Seed",
        "未命名种子" => "Unnamed seed",
        "离线提醒" => "Offline reminders",
        "提醒服务" => "Notification provider",
        "离线提醒服务" => "Offline notification provider",
        "不发送提醒" => "Do not send notifications",
        "钉钉" => "DingTalk",
        "Webhook 地址" => "Webhook URL",
        "https://oapi.dingtalk.com/robot/send?access_token=…（与 Access Token 二选一）" => {
            "https://oapi.dingtalk.com/robot/send?access_token=... (either this or the access token)"
        }
        "Access Token" => "Access Token",
        "与 Webhook 地址二选一" => "Either the webhook URL or this",
        "加签密钥" => "Signing secret",
        "可选；安全设置选择加签时填写 SEC 开头密钥" => {
            "Optional; SEC-prefixed secret when the bot uses signed mode"
        }
        "Webhook 地址与 Access Token 二选一；机器人安全设置选择「加签」时填写加签密钥。" => {
            "Webhook URL and access token are interchangeable; fill the signing secret when the bot uses signed mode."
        }
        "微信 Bot（暂不可用）" => "WeChat Bot (unavailable)",
        "Bot 文档" => "Bot documentation",
        "机器人类型" => "Bot Type",
        "官网" => "Docs",
        "绑定状态" => "Bind status",
        "未绑定" => "Not bound",
        "等待扫码" => "Waiting for scan",
        "绑定 QQ 通知" => "Bind QQ Notifications",
        "打开机器人" => "Open Bot",
        "解绑" => "Unbind",
        "测试通知" => "Test Notification",
        "保存下线提醒" => "Save Offline Alert",
        "服务器地址" => "Server URL",
        "平台" => "Platform",
        "操作系统" => "Operating system",
        "内存 (MB)" => "Memory (MB)",
        "重置为默认" => "Reset to default",
        "填写 AppID 和 AppSecret 后点绑定，再用手机 QQ 打开机器人发任意消息完成绑定。绑定后自动推送：下线、上线、应用宝授权二维码。" => "Enter the AppID and AppSecret, click bind, then open the bot in mobile QQ and send any message. After binding, offline, online, and YYB QR alerts are pushed automatically.",
        "选定提醒服务后，保存本地通知配置；测试操作会向已绑定目标发送一条通知。" => "Choose a provider and save its local notification settings. Testing sends one notification to the bound destination.",
        "绑定 QQ 机器人后，账号下线、上线、应用宝授权失效时会自动推送，无需配置标题和内容。" => "After binding, offline, online, and YYB QR alerts are sent automatically. No title or message setup is needed.",
        "QQ Bot 凭据" => "QQ Bot credentials",
        "填写 AppID 和 AppSecret 后点绑定，再用手机 QQ 打开机器人发任意消息完成绑定。" => "Enter AppID and AppSecret, click bind, then open the bot in mobile QQ and send any message.",
        "已绑定" => "Bound",
        "尚未绑定" => "Not bound",
        "请使用 QQ 扫描二维码完成绑定。" => "Scan this QR code with QQ to finish binding.",
        "二维码暂不可用，请在 QQ Bot 页面手动确认绑定。" => "The QR code is unavailable. Confirm the binding on the QQ Bot page.",
        "如已填写邀请链接，可扫码打开机器人资料卡，再发任意消息。" => "If an invite URL is set, scan to open the bot card, then send any message.",
        "等待绑定中：请用手机 QQ 打开该机器人，发送任意消息（无需扫本页二维码）。" => "Waiting: open this bot in mobile QQ and send any message (no in-app QR required).",
        "等待扫码…" => "Waiting for scan…",
        "绑定 QQ Bot" => "Bind QQ Bot",
        "打开 Bot" => "Open Bot",
        "解除绑定" => "Unbind",
        "钉钉 Webhook" => "DingTalk Webhook",
        "填写 Webhook 地址或 Access Token；两者至少填写一项。签名密钥可选。" => "Enter a Webhook URL or Access Token. At least one is required. The signing secret is optional.",
        "微信机器人暂未实现。" => "WeChat Bot is not available yet.",
        "发送测试通知" => "Send test notification",
        "保存提醒" => "Save reminder settings",
        "巡田间隔（秒）" => "Intervals (sec)",
        "农场巡查最小" => "Farm Check Min",
        "农场巡查最大" => "Farm Check Max",
        "帮助巡查最小" => "Help Check Min",
        "帮助巡查最大" => "Help Check Max",
        "偷菜巡查最小" => "Steal Check Min",
        "偷菜巡查最大" => "Steal Check Max",
        "好友静默时段" => "Friend Quiet Hours",
        "启用静默时段" => "Enable Quiet Hours",
        "开始" => "Start",
        "结束" => "End",
        "静默时继续农场巡查" => "Keep farm patrol in quiet hours",
        "种植与偷菜延迟设置" => "Plant & Steal Delay",
        "种植顺序随机" => "Random Plant Order",
        "种植延迟 (秒)" => "Plant Delay (sec)",
        "偷菜延迟 (秒)" => "Steal Delay (sec)",
        "编辑时间" => "Edit time",
        "无法更新此设置项。" => "This setting could not be updated.",
        "该数值不能小于 0。" => "This value cannot be less than 0.",
        "请输入有效整数。" => "Enter a valid integer.",
        "请输入有效数字。" => "Enter a valid number.",
        "请先填写 AppID 和 AppSecret。" => "Enter AppID and AppSecret first.",
        "正在创建 QQ Bot 绑定二维码…" => "Creating a QQ Bot binding QR code…",
        "等待 QQ 扫码并确认绑定…" => "Waiting for a QQ scan and binding confirmation…",
        "QQ Bot 已绑定。" => "QQ Bot is bound.",
        "二维码已过期，请重新发起绑定。" => "The QR code expired. Start the binding process again.",
        "QQ Bot 已解除绑定。" => "QQ Bot was unbound.",
        "无法打开链接" => "Could not open the link",
        "绑定任务失败" => "Binding failed",
        "绑定状态查询失败，将自动重试" => "Could not check binding status; retrying automatically",
        _ => chinese,
    };
    english.to_string()
}

#[cfg(test)]
mod tests {
    use super::settings_text;

    #[test]
    fn settings_labels_for_fertilizer_thresholds_and_missing_seed_are_localized() {
        assert_eq!(settings_text("en-US", "触发阈值 (小时)"), "Trigger Threshold (h)");
        assert_eq!(settings_text("en-US", "购买数量"), "Buy Count");
        assert_eq!(settings_text("en-US", "未命名种子"), "Unnamed seed");
    }

    #[test]
    fn ding_talk_fields_and_help_are_localized_like_vue() {
        assert_eq!(settings_text("zh-CN", "Webhook 地址"), "Webhook 地址");
        assert_eq!(settings_text("en-US", "Webhook 地址"), "Webhook URL");
        assert_eq!(settings_text("en-US", "Access Token"), "Access Token");
        assert_eq!(settings_text("en-US", "加签密钥"), "Signing secret");
        assert_eq!(
            settings_text("en-US", "与 Webhook 地址二选一"),
            "Either the webhook URL or this"
        );
        assert_eq!(
            settings_text("en-US", "可选；安全设置选择加签时填写 SEC 开头密钥"),
            "Optional; SEC-prefixed secret when the bot uses signed mode"
        );
        assert_eq!(
            settings_text(
                "en-US",
                "Webhook 地址与 Access Token 二选一；机器人安全设置选择「加签」时填写加签密钥。"
            ),
            "Webhook URL and access token are interchangeable; fill the signing secret when the bot uses signed mode."
        );
    }
}
