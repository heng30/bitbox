use crate::config;
use crate::db;
use crate::message::{async_message_success, async_message_warn};
use crate::password_dialog::is_password_verify;
use crate::slint_generatedAppWindow::{AddressBookItem, AppWindow, Logic, Store};
use crate::util::translator::tr;
use crate::wallet::{
    account::{address, sendinfo, tx},
    transaction::blockstream,
};
use crate::{message_success, message_warn};
use serde_json::Value;
use slint::{ComponentHandle, Weak};
use tokio::task::spawn;
use uuid::Uuid;

pub fn init(ui: &AppWindow) {
    let ui_handle = ui.as_weak();
    ui.global::<Logic>().on_send_tx(
        move |uuid, password, receive_address, send_amount, feerate| {
            let ui = ui_handle.unwrap();
            let uuid = uuid.to_string();
            let password = password.to_string();
            let receive_address = receive_address.to_string();
            let send_amount = send_amount.to_string();
            let feerate = feerate.to_string();

            if receive_address.is_empty() || send_amount.is_empty() || feerate.is_empty() {
                message_warn!(ui, tr("非法输入"));
                return;
            }

            let ui = ui.as_weak();
            spawn(async move {
                if !is_password_verify(uuid.clone(), password.clone()).await {
                    async_message_warn(ui.clone(), tr("密码错误"));
                    return;
                }

                match db::account::select(&uuid).await {
                    Ok(account) => match serde_json::from_str::<Value>(&account.data) {
                        Err(e) => async_message_warn(
                            ui.clone(),
                            format!("{}. {}: {e:?}", tr("生成交易失败"), tr("原因")),
                        ),
                        Ok(value) => {
                            let address_info = address::Info {
                                uuid: value["uuid"].as_str().unwrap().to_string(),
                                name: value["name"].as_str().unwrap().to_string(),
                                mnemonic: value["mnemonic"].as_str().unwrap().to_string(),
                                network: value["network"].as_str().unwrap().to_string(),
                                address: (
                                    value["main-address"].as_str().unwrap().to_string(),
                                    value["test-address"].as_str().unwrap().to_string(),
                                ),
                            };

                            let account_conf = config::account();
                            let mut send_info = sendinfo::Info::default();
                            send_info.recipient_address = receive_address.to_string();
                            send_info.fee_rate = feerate.parse().unwrap();
                            send_info.max_fee_rate = account_conf.max_feerate as u64;
                            send_info.max_fee_amount = account_conf.max_fee_amount as u64;
                            let send_info = send_info
                                .amount_from_btc(
                                    send_amount.as_str(),
                                    &format!("{}", account_conf.max_send_amount),
                                )
                                .unwrap();

                            match tx::build(&password, address_info, send_info).await {
                                Err(e) => async_message_warn(
                                    ui.clone(),
                                    format!("{}. {}: {e:?}", tr("生成交易失败"), tr("原因")),
                                ),
                                Ok(tx_detail) => {
                                    log::debug!("{tx_detail:?}");
                                    // TODO
                                }
                            }
                        }
                    },
                    Err(e) => async_message_warn(
                        ui.clone(),
                        format!("{}. {}: {e:?}", tr("生成交易失败"), tr("原因")),
                    ),
                }
            });
        },
    );
}
