use super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

fn calendar_fixture() -> serde_json::Value {
    let now = 1_792_252_900;
    serde_json::json!({
        "serverTime": now,
        "defaultEventId": 401,
        "events": [
            {
                "id": 401, "name": "重阳",
                "description": "农历 九月初九\n九日登高，云淡天长",
                "startTime": now - 100, "endTime": now + 86_400, "status": 2,
                "directClaimed": false, "quizClaimed": false, "quizWrongToday": false,
                "wrongOptions": [],
                "question": "重阳节又被称为什么节？",
                "options": ["A.敬老节", "B.儿童节", "C.母亲节", "D.教师节"],
                "errorTips": "它在农历九月初九。",
                "rules": ["节令期间可直接领取基础奖励。"],
                "rewards": [{"id": 1002, "count": 200, "name": "点券", "image": ""}],
                "questionRewards": [{"id": 1002, "count": 100, "name": "点券", "image": ""}],
                "canClaim": true, "canAnswer": true
            },
            {
                "id": 402, "name": "霜降",
                "description": "农历 九月十四\n气肃而凝，丹柿垂檐",
                "startTime": now + 86_400, "endTime": now + 172_800, "status": 1,
                "directClaimed": false, "quizClaimed": false, "quizWrongToday": false,
                "wrongOptions": [], "question": "", "options": [], "errorTips": "",
                "rules": [], "rewards": [], "questionRewards": [],
                "canClaim": false, "canAnswer": false
            }
        ],
        "seasons": [
            {
                "id": 4, "name": "第四季度", "year": "2026", "description": "",
                "eventIds": [401, 402], "litCount": 1, "totalCount": 3,
                "claimed": false, "canClaim": false,
                "rewards": [{"id": 1002, "count": 10, "name": "点券", "image": ""}]
            }
        ]
    })
}

fn seed_calendar(this: &mut crate::desktop::FarmDesktop) {
    this.page = Page::SolarCalendar;
    this.page_section = "all".into();
    this.selected_account = Some("7".into());
    this.page_raw = calendar_fixture();
    let calendar = this.page_raw.clone();
    this.apply_solar_calendar(calendar);
}

#[gpui_kit::test]
fn solar_calendar_renders_seasons_terms_and_phase_states(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| seed_calendar(this));
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("solar-refresh").visible());
        assert_eq!(window.find("solar-season-0").label(), Some("第四季度（1/3）"));
        assert_eq!(window.find(("solar-claim", 0_usize)).label(), Some("领取"));
        assert_eq!(window.find(("solar-quiz", 0_usize)).label(), Some("答题"));
        // 未开始的事件按距离显示，且领取按钮禁用（点击不触发操作）
        assert_eq!(window.find(("solar-claim", 1_usize)).label(), Some("1天后可领取"));
        window.click(("solar-claim", 1_usize), cx);
        assert_eq!(desktop.read(cx).solar_calendar.action_generation, 0);
        // 季度奖励未集齐不可领
        assert_eq!(window.find("solar-season-claim").label(), Some("集齐后可领"));
    })
    .expect("solar calendar renders seasons, terms and phase-gated buttons");
}

#[gpui_kit::test]
fn solar_calendar_quiz_dialog_selects_an_answer_option(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| seed_calendar(this));
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("solar-quiz", 0_usize), cx);
        window.render_frame(cx);
        assert!(window.has_active_dialog(cx));
        assert_eq!(window.find(("solar-option", 0_usize)).label(), Some("A. 敬老节"));
        assert_eq!(window.find(("solar-option", 3_usize)).label(), Some("D. 教师节"));
        window.click(("solar-option", 1_usize), cx);
        assert_eq!(desktop.read(cx).solar_calendar.quiz_answer, Some(1));
        window.press("escape", cx);
    })
    .expect("solar quiz dialog lists options and records the selected answer");
}
