use crate::db;
use crate::message::{async_message_success, async_message_warn};
use crate::slint_generatedAppWindow::{AddressBookItem, AppWindow, Logic, Store};
use crate::util::translator::tr;
use serde_json::{json, Value};
use slint::{ComponentHandle, Model, VecModel, Weak};
use tokio::task::spawn;
use uuid::Uuid;

pub fn init(ui: &AppWindow) {
    let ui_handle = ui.as_weak();
    ui.global::<Logic>().on_set_receive_address(move |address| {
        ui_handle.unwrap().set_receive_address(address);
    });

    let ui_handle = ui.as_weak();
    ui.global::<Logic>()
        .on_address_book_delete_item(move |uuid| {
            let ui = ui_handle.unwrap();

            for (index, item) in ui
                .global::<Store>()
                .get_address_book_datas()
                .iter()
                .enumerate()
            {
                if item.uuid == uuid {
                    ui.global::<Store>()
                        .get_address_book_datas()
                        .as_any()
                        .downcast_ref::<VecModel<AddressBookItem>>()
                        .expect("We know we set a VecModel earlier")
                        .remove(index);

                    let (ui, uuid) = (ui.as_weak(), uuid.to_string());
                    spawn(async move {
                        match db::address_book::delete(&uuid).await {
                            Ok(_) => async_message_success(ui.clone(), tr("删除成功")),
                            Err(e) => async_message_warn(
                                ui.clone(),
                                format!("{}. {}: {}", tr("删除失败"), tr("原因"), e),
                            ),
                        }
                    });
                    return;
                }
            }
        });

    let ui_handle = ui.as_weak();
    ui.global::<Logic>()
        .on_address_book_item_address(move |uuid| {
            let ui = ui_handle.unwrap();

            for item in ui.global::<Store>().get_address_book_datas().iter() {
                if item.uuid == uuid {
                    return item.address;
                }
            }

            slint::SharedString::default()
        });

    let ui_handle = ui.as_weak();
    ui.global::<Logic>()
        .on_address_book_add_item(move |name, address, network| {
            let ui = ui_handle.unwrap();
            let uuid = Uuid::new_v4().to_string();
            let network = network.to_string();

            let item = AddressBookItem {
                uuid: uuid.clone().into(),
                name,
                address,
            };

            let json_item = json!({
                "uuid": uuid.clone(),
                "name": item.name.to_string(),
                "address": item.address.to_string()
            });
            let json = serde_json::to_string(&json_item).unwrap();

            ui.global::<Store>()
                .get_address_book_datas()
                .as_any()
                .downcast_ref::<VecModel<AddressBookItem>>()
                .expect("We know we set a VecModel earlier")
                .push(item);

            let ui = ui.as_weak();
            spawn(async move {
                match db::address_book::insert(&uuid, &network, &json).await {
                    Ok(_) => async_message_success(ui.clone(), tr("添加成功")),
                    Err(e) => async_message_warn(
                        ui.clone(),
                        format!("{}. {}: {}", tr("添加失败"), tr("原因"), e),
                    ),
                }
            });
        });
}

pub fn load_items(ui: Weak<AppWindow>, network: String) {
    spawn(async move {
        match db::address_book::select_all_network(&network).await {
            Ok(items) => {
                let mut address_items = vec![];
                for item in items.iter() {
                    match serde_json::from_str::<Value>(&item.data) {
                        Err(e) => log::warn!("Error: {e:?}"),
                        Ok(value) => {
                            address_items.push(AddressBookItem {
                                uuid: value["uuid"].as_str().unwrap().into(),
                                name: value["name"].as_str().unwrap().into(),
                                address: value["address"].as_str().unwrap().into(),
                            });
                        }
                    }
                }

                let _ = slint::invoke_from_event_loop(move || {
                    ui.clone()
                        .unwrap()
                        .global::<Store>()
                        .get_address_book_datas()
                        .as_any()
                        .downcast_ref::<VecModel<AddressBookItem>>()
                        .expect("We know we set a VecModel earlier")
                        .set_vec(address_items);
                });
            }
            Err(e) => async_message_warn(
                ui.clone(),
                format!("{}. {}: {}", tr("加载失败"), tr("原因"), e),
            ),
        }
    });
}
