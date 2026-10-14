use super::super::super::*;

impl FarmDesktop {
    pub(in crate::desktop::activities) fn load_pet_diary_records(
        &mut self,
        kind: &'static str,
        cx: &mut Context<Self>,
    ) {
        if self.loading {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let revision = self.request_revision;
        self.activities.pet_diary_log_kind = kind.into();
        self.loading = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::activity::pet_diary_records(&app, &account_id, kind)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.request_revision != revision
                    || this.page != Page::Activities
                    || this.page_section != "petDiary"
                {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(Ok(value)) => {
                        this.activities.pet_diary_records.insert(kind.to_string(), value.clone());
                        if let Some(raw) = this.page_raw.as_object_mut() {
                            raw.insert("petDiaryRecords".into(), value);
                        }
                        this.rows = pages::page_data(Page::Activities, this.page_raw.clone()).rows;
                    }
                    Ok(Err(error)) => this.error = Some(error),
                    Err(error) => {
                        this.error = Some(crate::i18n::background_task_error(
                            &this.preferences.language,
                            error,
                        ))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
