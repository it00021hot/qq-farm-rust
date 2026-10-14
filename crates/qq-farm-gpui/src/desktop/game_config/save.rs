use super::super::*;
use super::form;
use gpui_kit::component::WindowExt;

impl FarmDesktop {
    pub(in crate::desktop) fn save_game_config_form(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let language = self.preferences.language.clone();
        let value = |entity: &Entity<InputState>| entity.read(cx).value().trim().to_string();
        let textarea_value =
            |entity: &Entity<TextareaState>| entity.read(cx).value().trim().to_string();
        let read = |key| {
            form::read_extra_field(
                &self.game_config.extra_fields,
                &self.game_config.extra_textareas,
                key,
                cx,
            )
        };
        let number = |raw: String, label: &str| -> Result<i64, String> {
            if raw.trim().is_empty() {
                return Ok(0);
            }
            raw.trim().parse::<i64>().map_err(|_| {
                let label = super::labels::form_field(&language, label);
                if language == "en-US" {
                    format!("{label} must be a valid integer.")
                } else {
                    format!("{label}必须是有效整数。")
                }
            })
        };
        let kind = self.game_config.kind.as_str();
        let id = match number(value(&self.game_config.id), "编号") {
            Ok(id) if id > 0 => Some(id),
            Ok(_) if kind == "fruit" && self.game_config.record.is_null() => None,
            Ok(_) => {
                self.game_config.form_error = Some(
                    crate::i18n::text(&language, "编号必须大于 0。", "ID must be greater than 0.")
                        .into(),
                );
                cx.notify();
                return;
            }
            Err(error) => {
                self.game_config.form_error = Some(error);
                cx.notify();
                return;
            }
        };
        let name = value(&self.game_config.name);
        if name.is_empty() {
            self.game_config.form_error =
                Some(crate::i18n::text(&language, "名称不能为空。", "Name is required.").into());
            cx.notify();
            return;
        }
        let image_path = value(&self.game_config.image_path);
        let image = match form::read_image(&image_path, &language) {
            Ok(image) => image,
            Err(error) => {
                self.game_config.form_error = Some(error);
                cx.notify();
                return;
            }
        };
        let price = number(read("price"), "价格");
        let price_id = number(read("priceId"), "货币");
        let level = number(value(&self.game_config.level), "等级");
        let rarity = number(read("rarity"), "稀有度");
        let max_count = number(read("maxCount"), "堆叠上限");
        let can_use = number(read("canUse"), "可使用");
        let (price, price_id, level, rarity, max_count, can_use) =
            match (price, price_id, level, rarity, max_count, can_use) {
                (Ok(price), Ok(price_id), Ok(level), Ok(rarity), Ok(max_count), Ok(can_use)) => {
                    (price, price_id, level, rarity, max_count, can_use)
                }
                (Err(error), _, _, _, _, _)
                | (_, Err(error), _, _, _, _)
                | (_, _, Err(error), _, _, _)
                | (_, _, _, Err(error), _, _)
                | (_, _, _, _, Err(error), _)
                | (_, _, _, _, _, Err(error)) => {
                    self.game_config.form_error = Some(error);
                    cx.notify();
                    return;
                }
            };
        if [price, price_id, level, rarity, max_count, can_use].iter().any(|value| *value < 0) {
            self.game_config.form_error = Some(
                crate::i18n::text(
                    &language,
                    "数字字段不能小于 0。",
                    "Numeric fields cannot be negative.",
                )
                .into(),
            );
            cx.notify();
            return;
        }

        let result = match self.game_config.kind.as_str() {
            "seed" => {
                let id = id.expect("seed identifiers are required by the form");
                let seasons = number(read("seasons"), "季节");
                let exp = number(read("exp"), "经验");
                let size = number(read("size"), "占地");
                let fruit_count = number(read("fruitCount"), "收获数");
                match (seasons, exp, size, fruit_count) {
                    (Ok(seasons), Ok(exp), Ok(size), Ok(fruit_count))
                        if [seasons, exp, size, fruit_count].iter().any(|value| *value < 0) =>
                    {
                        Err(qq_farm_app::AppError::BadRequest(
                            crate::i18n::text(
                                &language,
                                "数字字段不能小于 0。",
                                "Numeric fields cannot be negative.",
                            )
                            .into(),
                        ))
                    }
                    (Ok(seasons), Ok(exp), Ok(size), Ok(fruit_count)) => {
                        qq_farm_app::config::upsert_seed_config(
                            qq_farm_app::config::SeedConfigWrite {
                                seed_id: Some(id),
                                name: Some(name),
                                grow_phases: Some(read("growPhases")),
                                land_level_need: Some(level),
                                seasons: Some(seasons),
                                fruit_count: Some(fruit_count),
                                price: Some(price),
                                price_id: Some(price_id),
                                exp: Some(exp),
                                size: Some(size),
                            },
                            image.as_ref(),
                        )
                    }
                    (Err(error), _, _, _)
                    | (_, Err(error), _, _)
                    | (_, _, Err(error), _)
                    | (_, _, _, Err(error)) => Err(qq_farm_app::AppError::BadRequest(error)),
                }
            }
            "fruit" => {
                let plant_id = self.game_config.plant_id;
                let fruit_count = number(read("fruitCount"), "收获数");
                match (plant_id, fruit_count) {
                    (_, Err(error)) => Err(qq_farm_app::AppError::BadRequest(error)),
                    (_, Ok(fruit_count)) if fruit_count < 0 => {
                        Err(qq_farm_app::AppError::BadRequest(
                            crate::i18n::text(
                                &language,
                                "数字字段不能小于 0。",
                                "Numeric fields cannot be negative.",
                            )
                            .into(),
                        ))
                    }
                    (Some(plant_id), Ok(fruit_count)) if plant_id > 0 => {
                        qq_farm_app::config::upsert_fruit_config(
                            qq_farm_app::config::FruitConfigWrite {
                                id,
                                plant_id: Some(plant_id),
                                name: Some(name),
                                price: Some(price),
                                price_id: Some(price_id),
                                desc: Some(textarea_value(&self.game_config.desc)),
                                effect_desc: Some(read("effectDesc")),
                                rarity: Some(rarity),
                                max_count: Some(max_count),
                                level: Some(level),
                                fruit_count: Some(fruit_count),
                                asset_name: Some(value(&self.game_config.asset)),
                            },
                            image.as_ref(),
                        )
                    }
                    _ => Err(qq_farm_app::AppError::BadRequest(
                        crate::i18n::text(
                            &language,
                            "请选择一个有效的关联植物。",
                            "Choose a valid linked plant.",
                        )
                        .into(),
                    )),
                }
            }
            _ => {
                let id = id.expect("item identifiers are required by the form");
                let item_type =
                    self.game_config.item_type_form_selector.read(cx).selected_value().copied();
                let interaction_type = read("interactionType");
                match item_type {
                    Some(item_type) if item_type > 0 => qq_farm_app::config::upsert_item_config(
                        qq_farm_app::config::ItemConfigWrite {
                            id: Some(id),
                            item_type: Some(item_type),
                            name: Some(name),
                            price: Some(price),
                            price_id: Some(price_id),
                            interaction_type: Some(interaction_type),
                            can_use: Some(can_use),
                            desc: Some(textarea_value(&self.game_config.desc)),
                            effect_desc: Some(read("effectDesc")),
                            rarity: Some(rarity),
                            max_count: Some(max_count),
                            level: Some(level),
                            asset_name: Some(value(&self.game_config.asset)),
                        },
                        image.as_ref(),
                    ),
                    Some(_) => Err(qq_farm_app::AppError::BadRequest(
                        crate::i18n::text(
                            &language,
                            "道具类型必须大于 0。",
                            "Item type must be greater than 0.",
                        )
                        .into(),
                    )),
                    None => Err(qq_farm_app::AppError::BadRequest(
                        crate::i18n::text(&language, "请选择道具类型。", "Choose an item type.")
                            .into(),
                    )),
                }
            }
        };
        match result {
            Ok(()) => {
                self.show_game_config_success("更新成功", "Update Success", cx);
                self.game_config.form_open = false;
                window.close_sheet(cx);
                self.refresh(cx);
            }
            Err(error) => {
                self.game_config.form_error = Some(error.to_string());
                cx.notify();
            }
        }
    }
}
