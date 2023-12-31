use crate::message_warn;
use crate::slint_generatedAppWindow::{AppWindow, Logic, Util};
use crate::util::translator::tr;
use crate::util::{self, number, time};
use image::Rgb;
use qrcode::QrCode;
use slint::{ComponentHandle, Image, Rgb8Pixel, SharedPixelBuffer};
use webbrowser;

pub fn init(ui: &AppWindow) {
    ui.global::<Util>().on_string_fixed2(move |n| {
        let n = n.to_string().parse::<f32>().unwrap_or(0.0f32);
        slint::format!("{:2}", (n * 100.0).round() / 100.0)
    });

    ui.global::<Util>()
        .on_float_fixed2(move |n| slint::format!("{:2}", (n * 100.0).round() / 100.0));

    let ui_handle = ui.as_weak();
    ui.global::<Util>().on_open_url(move |url| {
        let ui = ui_handle.unwrap();
        if let Err(e) = webbrowser::open(url.as_str()) {
            message_warn!(
                ui,
                format!("{}{}: {:?}", tr("打开链接失败！"), tr("原因"), e)
            );
        }
    });

    let ui_handle = ui.as_weak();
    ui.global::<Util>().on_generate_qrcode(move |msg| {
        let ui = ui_handle.unwrap();
        match QrCode::new(msg) {
            Ok(code) => {
                let qrc = code.render::<Rgb<u8>>().build();

                let buffer = SharedPixelBuffer::<Rgb8Pixel>::clone_from_slice(
                    qrc.as_raw(),
                    qrc.width(),
                    qrc.height(),
                );
                Image::from_rgb8(buffer)
            }
            Err(e) => {
                log::warn!("gen qrcode image error: {:?}", e);
                ui.global::<Util>().get_no_image()
            }
        }
    });

    ui.global::<Util>()
        .on_format_number_with_commas(move |number_str| {
            number::format_number_with_commas(number_str.as_str()).into()
        });

    ui.global::<Util>()
        .on_local_now(move |format| time::local_now(format.as_str()).into());

    ui.global::<Util>()
        .on_split_and_join_string(move |input, length, sep| {
            util::str::split_string_to_fixed_length_parts(input.as_str(), length as usize)
                .join(sep.as_str())
                .into()
        });
}
