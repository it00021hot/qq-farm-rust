//! 隔离的界面测试/预览快照，不进入线上业务构建。
pub(crate) fn weather_fixture() -> serde_json::Value {
    serde_json::json!({"weather": {
        "snapshot": {
            "active": true, "activity": {"name": "雨落成诗"},
            "ownWeather": {"active": false},
            "shop": {"available": false, "reason": "金币不足", "balance": "500", "owned": false,
                "item": {"id": "123", "name": "天气采集瓶", "count": "1"},
                "cost": {"id": "1", "name": "金币", "count": "1000"}},
            "tasks": [{"id":"1", "title":"采集好友雨水", "current":2, "dailyLimit":5,
                "reward":{"id":"124", "name":"雷电徽章", "count":"10"}}],
            "research": {"badgeBalance":"25", "currentStage":1, "nodes":[
                {"id":"1", "completed":true, "availableByStatus":false, "affordable":true,
                    "cost":{"id":"124", "name":"雷电徽章", "count":"10"},
                    "reward":{"id":"101", "name":"种子", "count":"2"}},
                {"id":"2", "completed":false, "availableByStatus":true, "affordable":false,
                    "cost":{"id":"124", "name":"雷电徽章", "count":"30"},
                    "reward":{"id":"102", "name":"雷雨召唤瓶", "count":"1"}},
                {"id":"3", "locked":true, "completed":false, "availableByStatus":false,
                    "cost":{"id":"124", "name":"雷电徽章", "count":"40"},
                    "reward":{"id":"103", "name":"化肥", "count":"1"}}
            ]},
            "inventory":[{"id":"123", "name":"天气采集瓶", "count":"3"}],
            "actions": {
                "summonThunderstorm":{"enabled":false, "reason":"背包中没有可用的雷雨召唤瓶"},
                "exchangeCollector":{"enabled":true}, "collectWeather":{"enabled":true, "dailyLimit":5},
                "scanFriendWeather":{"enabled":true}, "frogMischief":{"enabled":false},
                "cloudMischief":{"enabled":true}, "advanceResearch":{"enabled":false, "reason":"雷电徽章不足"}
            },
            "rules":{"title":"活动说明", "paragraphs":["好友农场有雷雨时，可使用采集瓶采雨。", "天气任务奖励自动发放。"]}
        },
        "friends":[{"gid":"12", "name":"Alice", "level":18}, {"gid":"34", "name":"Bob", "level":22}]
    }})
}
