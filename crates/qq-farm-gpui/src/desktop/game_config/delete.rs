use super::super::*;

impl FarmDesktop {
    pub(super) fn confirm_selected_game_config_deletion(&mut self, cx: &mut Context<Self>) {
        if !self.game_config.pending_batch_delete {
            return;
        }
        self.game_config.pending_batch_delete = false;
        self.delete_selected_game_config(cx);
    }

    pub(super) fn delete_selected_game_config(&mut self, cx: &mut Context<Self>) {
        let selected = self
            .game_config
            .selected_ids
            .iter()
            .filter(|(section, _)| section == &self.page_section)
            .cloned()
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return;
        }

        let mut deleted_count = 0;
        let mut failed = Vec::new();
        for (section, id) in selected {
            let kind = if section == "seeds" {
                qq_farm_app::config::GameConfigKind::Plant
            } else {
                qq_farm_app::config::GameConfigKind::Item
            };
            match qq_farm_app::config::delete_game_config(kind, id) {
                Ok(_) => deleted_count += 1,
                Err(error) => failed.push(format!("{id}: {error}")),
            }
        }

        self.game_config.selected_ids.clear();
        if deleted_count > 0 {
            self.show_game_config_success("删除成功", "Delete Success", cx);
        }
        if failed.is_empty() {
            self.error = None;
        } else if self.preferences.language == "en-US" {
            self.error =
                Some(format!("Some configurations could not be deleted: {}", failed.join("; ")));
        } else {
            self.error = Some(format!("部分配置删除失败：{}", failed.join("；")));
        }
        if deleted_count > 0 {
            self.refresh(cx);
        } else {
            cx.notify();
        }
    }

    pub(super) fn delete_game_config(&mut self, cx: &mut Context<Self>) {
        let Some((kind, id)) = self.game_config.pending_delete.take() else { return };
        let kind = if kind == "seed" {
            qq_farm_app::config::GameConfigKind::Plant
        } else {
            qq_farm_app::config::GameConfigKind::Item
        };
        match qq_farm_app::config::delete_game_config(kind, id) {
            Ok(_) => {
                self.show_game_config_success("删除成功", "Delete Success", cx);
                self.refresh(cx);
            }
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
    }
}
