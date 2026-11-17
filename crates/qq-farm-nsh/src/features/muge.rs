use nsh_core::prelude::*;
use serde_json::{json, Value};

use super::activities::ActivityPage;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, notice, ButtonVariant};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct MugeBoard {
    active: bool,
    celebration: bool,
    friends: bool,
    handbook: bool,
    can_share: bool,
    can_settle: bool,
    can_reset: bool,
    tasks: Vec<MugeTask>,
    questions: Vec<MugeQuestion>,
}

#[derive(Clone, PartialEq, Eq)]
struct MugeTask {
    id: String,
    description: String,
    progress: String,
    can_claim: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct MugeQuestion {
    id: i64,
    title: String,
    selected: i64,
    options: Vec<MugeOption>,
}

#[derive(Clone, PartialEq, Eq)]
struct MugeOption {
    value: i64,
    text: String,
}

pub(crate) fn loaded(
    board: Resource<MugeBoard, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(when(
            move || board.loading() && board.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || board.error().is_some(),
            move || notice(move || SharedString::from(board.error().unwrap_or_default())),
        ))
        .child(when(move || board.get().is_some(), move || body(board, pending, failure, page)))
}

fn body(
    board: Resource<MugeBoard, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(when(
            move || board.get().is_some_and(|item| !item.active),
            move || notice(move || text_key("activities.mugeClosed")),
        ))
        .child(claim_button(
            "claimCelebration",
            "activities.mugeCelebration",
            move || board.get().is_some_and(|item| item.celebration),
            pending,
            failure,
            page,
            board,
        ))
        .child(claim_button(
            "claimFriends",
            "activities.mugeFriends",
            move || board.get().is_some_and(|item| item.friends),
            pending,
            failure,
            page,
            board,
        ))
        .child(claim_button(
            "claimHandbook",
            "activities.mugeHandbook",
            move || board.get().is_some_and(|item| item.handbook),
            pending,
            failure,
            page,
            board,
        ))
        .child(each(
            move || board.get().map(|item| item.tasks).unwrap_or_default(),
            |task: &MugeTask| task.id.clone(),
            move |task, _| task_row(task.get_untracked(), pending, failure, page, board),
        ))
        .child(when(
            move || board.get().is_some_and(|item| item.can_reset),
            move || {
                button(move || text_key("activities.mugeRetest"), ButtonVariant::Quiet)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        run(
                            "saveAnswers",
                            json!({ "answers": {}, "clear": true }),
                            pending,
                            failure,
                            page,
                            board,
                        )
                    })
            },
        ))
        .child(each(
            move || board.get().map(|item| item.questions).unwrap_or_default(),
            |question: &MugeQuestion| question.id.to_string(),
            move |question, _| {
                question_row(question.get_untracked(), pending, failure, page, board)
            },
        ))
        .child(when(
            move || board.get().is_some_and(|item| item.can_settle),
            move || {
                button(move || text_key("activities.mugeSettle"), ButtonVariant::Primary)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || run("settle", json!({}), pending, failure, page, board))
            },
        ))
        .child(when(
            move || board.get().is_some_and(|item| item.can_share),
            move || {
                button(move || text_key("activities.mugeShare"), ButtonVariant::Secondary)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || run("claimShare", json!({}), pending, failure, page, board))
            },
        ))
}

fn claim_button(
    action: &'static str,
    label: &'static str,
    ready: impl Fn() -> bool + 'static,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    board: Resource<MugeBoard, String>,
) -> impl IntoElement {
    when(ready, move || {
        button(move || text_key(label), ButtonVariant::Secondary)
            .disabled(move || pending.get().is_some())
            .on_press(move || run(action, json!({}), pending, failure, page, board))
    })
}

fn task_row(
    task: MugeTask,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    board: Resource<MugeBoard, String>,
) -> impl IntoElement {
    let description = task.description.clone();
    let progress = task.progress.clone();
    let can_claim = task.can_claim;
    view()
        .gap_1()
        .child(text(description).text_sm())
        .child(text(progress).text_sm().text_color(move || nsh_base::theme().muted_foreground))
        .child(when(
            move || can_claim,
            move || {
                button(move || text_key("activities.mugeTask"), ButtonVariant::Quiet)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || run("claimTask", json!({}), pending, failure, page, board))
            },
        ))
}

fn question_row(
    question: MugeQuestion,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    board: Resource<MugeBoard, String>,
) -> impl IntoElement {
    let title = question.title.clone();
    let question_id = question.id;
    let selected = question.selected;
    view().gap_1().child(text(title).text_sm()).child(view().flex_row().flex_wrap().gap_2().child(
        each(
            move || question.options.clone(),
            |option: &MugeOption| option.value.to_string(),
            move |option, _| {
                let option = option.get_untracked();
                let value = option.value;
                let text = option.text.clone();
                button(move || SharedString::from(text.clone()), ButtonVariant::Quiet)
                    .selected(move || selected == value)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        let mut answers = serde_json::Map::new();
                        answers.insert(question_id.to_string(), json!(value));
                        run(
                            "saveAnswers",
                            json!({ "answers": answers }),
                            pending,
                            failure,
                            page,
                            board,
                        )
                    })
            },
        ),
    ))
}

fn run(
    action: &'static str,
    input: Value,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    board: Resource<MugeBoard, String>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let Some(id) = page.account_id.get_untracked() else {
        return;
    };
    pending.set(Some(action.to_owned()));
    failure.set(None);
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::muge::operate(&context, &id, action, &input).await
        })
        .await;
        if let Err(message) = result {
            failure.set(Some(message));
        }
        pending.set(None);
        board.refetch();
    });
}

pub(crate) fn board_from(value: &Value) -> MugeBoard {
    let answers = value.pointer("/personality/answers");
    let questions: Vec<MugeQuestion> = value
        .get("questions")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(|row| question_from(row, answers)).collect())
        .unwrap_or_default();
    let can_settle =
        !questions.is_empty() && questions.iter().all(|question| question.selected > 0);
    let can_reset = questions.iter().any(|question| question.selected > 0);
    MugeBoard {
        active: value.get("active").and_then(Value::as_bool).unwrap_or(false),
        celebration: value
            .pointer("/celebration/canClaim")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        friends: value.pointer("/friends/canClaim").and_then(Value::as_bool).unwrap_or(false),
        handbook: value.pointer("/handbook/canClaim").and_then(Value::as_bool).unwrap_or(false),
        can_share: value.pointer("/personality/canShare").and_then(Value::as_bool).unwrap_or(false),
        can_settle,
        can_reset,
        tasks: value
            .pointer("/handbook/tasks")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(task_from).collect())
            .unwrap_or_default(),
        questions,
    }
}

fn task_from(value: &Value) -> Option<MugeTask> {
    let id = value.get("id").and_then(Value::as_str).filter(|id| !id.is_empty())?;
    let progress = value.get("progress").and_then(Value::as_i64).unwrap_or(0);
    let target = value.get("target").and_then(Value::as_i64).unwrap_or(0);
    Some(MugeTask {
        id: id.to_owned(),
        description: value.get("description").and_then(Value::as_str).unwrap_or(id).to_owned(),
        progress: format!("{progress}/{target}"),
        can_claim: value.get("canClaim").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn question_from(value: &Value, answers: Option<&Value>) -> Option<MugeQuestion> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let title = value.get("title").and_then(Value::as_str).unwrap_or("");
    let options = value
        .get("options")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|option| {
                    Some(MugeOption {
                        value: option.get("value").and_then(Value::as_i64)?,
                        text: option.get("text").and_then(Value::as_str).unwrap_or("").to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(MugeQuestion {
        id,
        title: title.to_owned(),
        selected: answers
            .and_then(|row| row.get(id.to_string()))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        options,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::board_from;

    #[test]
    fn board_keeps_ready_claims_and_the_chosen_answer() {
        let board = board_from(&json!({
            "active": true,
            "celebration": { "canClaim": true },
            "friends": { "canClaim": false },
            "handbook": {
                "canClaim": false,
                "tasks": [{ "id": "3", "description": "收获", "progress": 1, "target": 2, "canClaim": true }]
            },
            "personality": { "canShare": false, "answers": { "1": 2 } },
            "questions": [
                { "id": 1, "title": "何时上线", "options": [{ "value": 2, "text": "偶尔" }] },
                { "id": 2, "title": "先做什么", "options": [{ "value": 1, "text": "收菜" }] }
            ]
        }));
        assert!(board.active);
        assert!(board.celebration);
        assert!(board.tasks[0].can_claim);
        assert_eq!(board.tasks[0].progress, "1/2");
        assert_eq!(board.questions[0].selected, 2);
        assert_eq!(board.questions[1].selected, 0);
        assert!(!board.can_settle);
        assert!(board.can_reset);
    }
}
