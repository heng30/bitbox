use crate::slint_generatedAppWindow::{AppWindow, Logic, Store};
use crate::util::translator::tr;
use slint::ComponentHandle;

pub fn init(ui: &AppWindow) {
    let ui_handle = ui.as_weak();
    ui.global::<Logic>()
        .on_handle_password_dialog(move |handle_type, handle_uuid, password| {
            let ui = ui_handle.unwrap();

            match handle_type.as_str() {
                "delete-account" => {
                    ui.global::<Logic>()
                        .invoke_delete_account(handle_uuid, password);
                }
                "show-mnemonic" => {
                    ui.global::<Logic>()
                        .invoke_show_mnemonic(handle_uuid, password);
                }
                "logout" => {
                    handle_logout(&ui, password.as_str());
                }
                _ => (),
            }
        });
}

fn handle_logout(ui: &AppWindow, password: &str) {
    let mut config = ui.global::<Store>().get_password_dialog_config();

    if is_password_verify(password) {
        ui.global::<Logic>()
            .invoke_show_message("warning".into(), tr("密码错误").into());
        config.show = false;
    } else {
        config.show = true;
    }

    ui.global::<Store>().set_password_dialog_config(config);
}

// TODO
pub fn is_password_verify(password: &str) -> bool {
    false
}
