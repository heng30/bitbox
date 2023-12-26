use crate::slint_generatedAppWindow::{AddressBookItem, AppWindow, Logic, Store};
use slint::{ComponentHandle, Model, VecModel};

pub fn init(ui: &AppWindow) {
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

                    // TODO: remove data from the database
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
}
