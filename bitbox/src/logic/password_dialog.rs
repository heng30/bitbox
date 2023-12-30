use crate::slint_generatedAppWindow::{AppWindow, Logic, Store};
use crate::util::translator::tr;
use crate::{db, util};
use serde_json::Value;
use slint::{ComponentHandle, Weak};
use tokio::task::spawn;

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
                "recover-account" => {
                    ui.global::<Logic>()
                        .invoke_recover_account(handle_uuid, password);
                }
                "send-tx" => {
                    let receive_address = ui.get_receive_address();
                    let send_amount = ui.get_send_amount();
                    let feerate = ui.get_feerate();

                    ui.global::<Logic>()
                    .invoke_send_tx(handle_uuid, password, receive_address, send_amount, feerate);
                }
                "logout" => {
                    handle_logout(ui.as_weak(), handle_uuid.to_string(), password.to_string());
                }
                _ => (),
            }
        });
}

fn handle_logout(ui: Weak<AppWindow>, uuid: String, password: String) {
    spawn(async move {
        let ret = is_password_verify(uuid, password).await;

        let _ = slint::invoke_from_event_loop(move || {
            let ui = ui.unwrap();

            let mut config = ui.global::<Store>().get_password_dialog_config();

            if !ret {
                ui.global::<Logic>()
                    .invoke_show_message(tr("密码错误").into(), "warning".into());
            }

            config.show = !ret;
            ui.global::<Store>().set_password_dialog_config(config);
        });
    });
}

pub async fn is_password_verify(uuid: String, password: String) -> bool {
    match db::account::select(&uuid).await {
        Ok(account) => match serde_json::from_str::<Value>(&account.data) {
            Err(e) => {
                log::warn!("Error: {e:?}");
                false
            }
            Ok(value) => util::crypto::hash(&password) == value["password"],
        },
        Err(e) => {
            log::warn!("Error: {e:?}");
            false
        }
    }
}
