pub(crate) fn account_status_text(language: &str, value: &str) -> String {
    if language != "en-US" {
        return value.to_string();
    }
    if let Some((nickname, _)) = value.split_once(" · 请在电脑微信中确认") {
        return format!("{nickname} · Confirm in WeChat for desktop");
    }
    let fixed = match value {
        "正在获取微信登录二维码…" => "Loading WeChat QR code…",
        "等待微信扫码" => "Waiting for a WeChat scan",
        "登录状态检查失败" => "Could not check the login status",
        "已扫码，请在手机上确认" => "Scanned. Confirm on your phone.",
        "已确认，正在建立登录会话…" => "Confirmed. Creating the login session…",
        "登录失败，请重试" => "Login failed. Try again.",
        "登录二维码已失效" => "The login QR code has expired",
        "正在检测本机微信…" => "Checking for WeChat on this device…",
        "本机微信已就绪，请点击授权" => {
            "WeChat is ready on this device. Click to authorize."
        }
        "等待电脑微信确认…" => "Waiting for confirmation in WeChat for desktop…",
        "正在保存账号…" => "Saving the account…",
        "保存账号失败" => "Could not save the account",
        "二维码加载中…" => "Loading QR code…",
        _ => return localized_account_message(language, value),
    };
    fixed.to_string()
}

pub(crate) fn localized_account_message(language: &str, value: &str) -> String {
    if language != "en-US" {
        return value.to_string();
    }
    let exact = match value {
        "请完成微信授权后保存账号" => {
            Some("Complete WeChat authorization before saving the account")
        }
        "请输入登录 code 或粘贴完整登录链接" => {
            Some("Enter a login code or paste a full login URL")
        }
        "保存账号失败" => Some("Could not save the account"),
        "本机微信授权失败，请扫码登录" => {
            Some("WeChat authorization failed. Use QR code login.")
        }
        "二维码已失效，请重新获取" => Some("The QR code expired. Get a new one."),
        "登录状态检查失败" => Some("Could not check the login status"),
        "二维码获取失败" | "二维码加载失败" => Some("Could not load the QR code"),
        _ => None,
    };
    if let Some(message) = exact {
        return message.to_string();
    }
    for (prefix, english) in [
        ("账号保存任务失败：", "The account save task failed: "),
        ("账号已保存，但自动启动失败：", "Account saved, but automatic start failed: "),
        ("部分账号未删除：", "Some accounts could not be deleted: "),
        ("二维码图片写入失败：", "Could not write the QR code image: "),
        ("二维码任务失败：", "The QR code task failed: "),
        ("登录任务失败：", "The login task failed: "),
        ("登录状态检查任务失败：", "The login status check failed: "),
        ("未检测到本机微信：", "WeChat for desktop was not found: "),
    ] {
        if let Some(suffix) = value.strip_prefix(prefix) {
            let mut message = format!("{english}{suffix}");
            if prefix == "未检测到本机微信：" {
                message = message.replace("。请扫码登录", ". Use QR code login.");
                if !message.contains("Use QR code login") {
                    message.push_str(". Use QR code login.");
                }
            }
            return message;
        }
    }
    value.to_string()
}
