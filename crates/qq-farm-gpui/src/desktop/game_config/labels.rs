pub(in crate::desktop) fn form_field<'a>(language: &str, chinese: &'a str) -> &'a str {
    use crate::i18n::text;

    match chinese {
        "编号" => text(language, "编号", "ID"),
        "种子ID" => text(language, "种子ID", "Seed ID"),
        "果实ID" => text(language, "果实ID", "Fruit ID"),
        "物品ID" => text(language, "物品ID", "Item ID"),
        "名称" => text(language, "名称", "Name"),
        "阶段模板" => text(language, "阶段模板", "Phase template"),
        "生长阶段" => text(language, "生长阶段", "Growth phases"),
        "等级" => text(language, "等级", "Level"),
        "季节" => text(language, "季节", "Seasons"),
        "收获数" => text(language, "收获数", "Harvest"),
        "价格" => text(language, "价格", "Price"),
        "货币" => text(language, "货币", "Currency"),
        "经验" => text(language, "经验", "Experience"),
        "占地" => text(language, "占地", "Size"),
        "上传图片" => text(language, "上传图片", "Image"),
        "植物" => text(language, "植物", "Plant"),
        "关联植物编号" => text(language, "关联植物编号", "Linked plant ID"),
        "关联植物编号或名称" => {
            text(language, "关联植物编号或名称", "Linked plant ID or name")
        }
        "稀有度" => text(language, "稀有度", "Rarity"),
        "堆叠上限" => text(language, "堆叠上限", "Max stack"),
        "资源名" => text(language, "资源名", "Asset name"),
        "描述" => text(language, "描述", "Description"),
        "效果描述" => text(language, "效果描述", "Effect"),
        "可使用" => text(language, "可使用", "Usable"),
        "交互类型" => text(language, "交互类型", "Interaction"),
        "类型" => text(language, "类型", "Type"),
        _ => chinese,
    }
}

#[cfg(test)]
mod tests {
    use super::form_field;

    #[test]
    fn game_config_form_fields_follow_the_selected_language() {
        assert_eq!(form_field("zh-CN", "种子ID"), "种子ID");
        assert_eq!(form_field("en-US", "种子ID"), "Seed ID");
        assert_eq!(form_field("en-US", "关联植物编号或名称"), "Linked plant ID or name");
    }
}
