use crate::activity::activity_add_item;
use crate::message::async_message_warn;
use crate::password_dialog::is_password_verify;
use crate::slint_generatedAppWindow::{AppWindow, Logic, Store, TxDetail};
use crate::util::translator::tr;
use crate::wallet::{
    account::{address, sendinfo, tx},
    transaction::blockstream,
};
use crate::{config, db, message_info, message_success, message_warn};
use serde_json::Value;
use slint::ComponentHandle;
use tokio::task::spawn;

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

            let btc_price: f64 = ui
                .global::<Store>()
                .get_btc_info()
                .price
                .parse()
                .unwrap_or(0_f64);

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
                            let send_info = sendinfo::Info {
                                recipient_address: receive_address.to_string(),
                                fee_rate: feerate.parse().unwrap(),
                                max_fee_rate: account_conf.max_feerate as u64,
                                max_fee_amount: account_conf.max_fee_amount as u64,
                                ..Default::default()
                            };

                            let send_info = match send_info.amount_from_btc(
                                send_amount.as_str(),
                                &format!("{}", account_conf.max_send_amount),
                            ) {
                                Ok(v) => v,
                                Err(e) => {
                                    async_message_warn(
                                        ui.clone(),
                                        format!("{}. {e:?}", tr("出错")),
                                    );
                                    return;
                                }
                            };

                            let mut tx_detail = TxDetail {
                                show: true,
                                network: address_info.network.clone().into(),
                                receive_address: receive_address.clone().into(),
                                send_amount_usd: slint::format!(
                                    "{:.2}",
                                    btc_price * send_amount.parse::<f64>().unwrap_or(0_f64)
                                ),
                                send_amount_btc: send_amount.into(),
                                send_address: if address_info.network == "main" {
                                    address_info.address.0.clone().into()
                                } else {
                                    address_info.address.1.clone().into()
                                },
                                ..Default::default()
                            };

                            match tx::build(&password, address_info, send_info).await {
                                Err(e) => async_message_warn(
                                    ui.clone(),
                                    format!("{}. {}: {e:?}", tr("生成交易失败"), tr("原因")),
                                ),
                                Ok(detail) => {
                                    tx_detail.fee_amount_usd = slint::format!(
                                        "{:.2}",
                                        detail.fee_amount as f64 * btc_price / 1e8
                                    );

                                    tx_detail.fee_amount_btc =
                                        slint::format!("{}", detail.fee_amount as f64 / 1e8);

                                    tx_detail.detail_raw = tx::parse_tx(&detail.tx_hex)
                                        .unwrap_or(String::default())
                                        .into();

                                    tx_detail.detail_hex = detail.tx_hex.into();

                                    let _ = slint::invoke_from_event_loop(move || {
                                        let ui = ui.clone().unwrap();
                                        ui.global::<Store>().set_tx_detail_dialog(tx_detail);
                                    });
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

    let ui_handle = ui.as_weak();
    ui.global::<Logic>()
        .on_broadcast_tx(move |network, tx_hex, send_amount_btc, fee| {
            let ui = ui_handle.clone();
            let network = network.to_string();
            let tx_hex = tx_hex.to_string();
            let amount = send_amount_btc.to_string();
            let fee = fee.to_string();

            message_info!(ui.clone().unwrap(), tr("正在发送交易..."));

            spawn(async move {
                match blockstream::broadcast_transaction(&network, tx_hex).await {
                    Err(e) => async_message_warn(
                        ui.clone(),
                        format!("{}. {}: {e:?}", tr("发送交易失败"), tr("原因")),
                    ),
                    Ok(txid) => {
                        if !tx::is_valid_txid(&txid) {
                            async_message_warn(
                                ui.clone(),
                                format!("{}. txid: {txid}", tr("非法交易")),
                            );
                            return;
                        }

                        let _ = slint::invoke_from_event_loop(move || {
                            let ui = ui.clone().unwrap();
                            activity_add_item(
                                &ui,
                                &network,
                                &txid,
                                "send",
                                &amount,
                                &fee,
                                "unconfirmed",
                            );
                            message_success!(ui, tr("发送交易成功"));
                        });
                    }
                }
            });
        });
}
