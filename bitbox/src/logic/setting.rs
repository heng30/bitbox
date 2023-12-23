use crate::slint_generatedAppWindow::{AppWindow, Logic, Store};
use crate::util::translator::tr;
use crate::{config, util};
use crate::{message_success, message_warn};
use slint::{ComponentHandle, Weak};

pub fn init(ui: &AppWindow) {
    init_setting_dialog(ui.as_weak());

    let ui_handle = ui.as_weak();
    ui.global::<Logic>().on_clean_cache(move || {
        let ui = ui_handle.unwrap();
        match util::fs::remove_dir_files(&config::cache_dir()) {
            Err(e) => {
                message_warn!(ui, format!("{}. {}: {}", tr("清空失败"), tr("原因"), e));
            }
            _ => {
                // let mut setting_dialog = ui.global::<Store>().get_setting_dialog_config();
                // ui.global::<Store>()
                //     .set_setting_dialog_config(setting_dialog);

                message_success!(ui, tr("清空成功"));
            }
        }
    });

    let ui_handle = ui.as_weak();
    ui.global::<Logic>().on_setting_cancel(move || {
        init_setting_dialog(ui_handle.clone());
    });

    let ui_handle = ui.as_weak();
    ui.global::<Logic>().on_setting_ok(move |setting_config| {
        let ui = ui_handle.unwrap();
        let mut config = config::config();

        config.ui.font_size = setting_config
            .ui
            .font_size
            .to_string()
            .parse()
            .unwrap_or(18);
        config.ui.font_family = setting_config.ui.font_family.to_string();
        config.ui.win_width = setting_config
            .ui
            .win_width
            .to_string()
            .parse()
            .unwrap_or(1200);
        config.ui.win_height = setting_config
            .ui
            .win_height
            .to_string()
            .parse()
            .unwrap_or(800);

        config.ui.language = setting_config.ui.language.to_string();

        match config::save(config) {
            Err(e) => {
                message_warn!(ui, format!("{}, {}: {:?}", tr("保存失败"), tr("原因"), e));
            }
            _ => {
                init_setting_dialog(ui.as_weak());
                message_success!(ui, tr("保存成功"));
            }
        }
    });
}

fn init_setting_dialog(ui: Weak<AppWindow>) {
    let ui = ui.unwrap();
    let ui_config = config::ui();

    let mut setting_dialog = ui.global::<Store>().get_setting_dialog_config();
    setting_dialog.ui.font_size = slint::format!("{}", ui_config.font_size);
    setting_dialog.ui.font_family = ui_config.font_family.into();
    setting_dialog.ui.win_width = slint::format!("{}", ui_config.win_width);
    setting_dialog.ui.win_height = slint::format!("{}", ui_config.win_height);
    setting_dialog.ui.language = ui_config.language.into();

    ui.global::<Store>()
        .set_setting_dialog_config(setting_dialog);
}
