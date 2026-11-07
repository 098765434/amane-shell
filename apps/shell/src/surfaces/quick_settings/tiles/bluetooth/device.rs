//! A device's own page: its details, renaming it, and its menu.

use super::*;

pub(super) fn device_page(device: BluetoothDevice) -> DetailPage {
    let bluetooth = Bluetooth::read();
    let operation = bluetooth.operation(device.path());
    let failure = super::super::failure::recent(bluetooth.failure(device.path()))
        .map(|failure| super::super::bluetooth_text::description(failure).to_string());
    drop(bluetooth);

    let status = match operation {
        Some(operation) => operation_label(operation).to_string(),
        None if device.connected() => String::from("Connected"),
        None => String::from("Not Connected"),
    };

    DetailPage::new("amane.qs.bluetooth.device", device.name())
        .subtitle(status)
        .on_back(close_device)
        .body(move |height| device_body(&device, operation, failure, height))
}

pub(super) fn device_body(
    device: &BluetoothDevice,
    operation: Option<Operation>,
    failure: Option<String>,
    height: f32,
) -> Vec<Box<dyn Widget>> {
    let id = |part: &str| format!("amane.qs.bluetooth.device.page.{part}");
    let mut rows: Vec<ListItem> = Vec::new();

    let label = if device.connected() {
        "Disconnect"
    } else {
        "Connect"
    };
    let mut connect = ListItem::new(id("connect"), label).leading(device_glyph(device.icon()));
    if let Some(operation) = operation {
        connect = connect.trailing(caption(operation_label(operation)));
    }
    let pressed = device.clone();
    rows.push(connect.on_press(move || act(&pressed)));

    if let Some(percent) = device.battery() {
        rows.push(
            ListItem::new(id("battery"), "Battery").trailing(list::trailing_text(
                format!("{percent}%"),
                battery::color(percent, false, material::label(Level::Secondary)),
            )),
        );
    }

    let trusted = device.trusted();
    let path = device.path().to_string();
    let toggled = path.clone();
    rows.push(
        ListItem::new(id("trusted"), "Connect Automatically")
            .trailing(
                Switch::new(id("trusted.switch"), trusted)
                    .on_toggle(move |on| Bluetooth::set_trusted(&toggled, on))
                    .build(),
            )
            .on_press(move || Bluetooth::set_trusted(&path, !trusted)),
    );

    let forgotten = device.clone();
    rows.push(
        ListItem::new(id("forget"), "Forget This Device…")
            .color(sys::color::status::critical::text())
            .on_press(move || ask_forget(&forgotten)),
    );

    let mut body: Vec<Box<dyn Widget>> = Vec::new();
    if let Some(failure) = failure {
        body.push(boxed(
            Text::new(failure)
                .style(sys::r#type::caption())
                .color(sys::color::status::critical::text()),
        ));
    }
    body.push(boxed(List::new(rows).build()));
    body.push(rename_row(device));
    body.push(boxed(caption(format!(
        "{} · {}",
        kind_name(device.icon()),
        device.address()
    ))));

    let column = Column::new(body).gap(sys::space::gap::l()).width(Parent);
    vec![boxed(
        ScrollArea::new("amane.qs.bluetooth.device.page", column)
            .height(Size::Fixed(height.max(1.0))),
    )]
}

/// The name this computer shows for the device, changed on Return.
pub(super) fn rename_row(device: &BluetoothDevice) -> Box<dyn Widget> {
    let path = device.path().to_string();
    let input = TextInput::new(NAME_ID)
        .placeholder(device.name())
        .size(sys::r#type::label().size)
        .color(material::label(Level::Primary))
        .width(Size::Fixed(page_width()))
        .on_submit(move |name| {
            if !name.trim().is_empty() {
                Bluetooth::rename(&path, &name);
            }
        });

    boxed(
        Column::new(vec![boxed(caption("Name")), boxed(input)])
            .gap(sys::space::gap::s())
            .width(Parent),
    )
}

/// What kind of device a BlueZ icon name says it is.
pub(super) fn kind_name(icon: &str) -> &'static str {
    match icon {
        i if i.starts_with("audio-head") => "Headphones",
        "audio-card" | "audio-speakers" => "Speaker",
        "input-keyboard" => "Keyboard",
        "input-mouse" => "Mouse",
        "input-gaming" => "Game Controller",
        "input-tablet" => "Tablet",
        "phone" => "Phone",
        "computer" => "Computer",
        "camera-photo" | "camera-video" => "Camera",
        "printer" => "Printer",
        _ => "Device",
    }
}

pub(super) fn open_device_menu(device: &BluetoothDevice) {
    let connected = device.connected();
    let for_act = device.clone();
    let for_details = device.clone();
    let for_forget = device.clone();

    let menu = ContextMenu::new()
        .item(
            MenuItem::label(if connected { "Disconnect" } else { "Connect" })
                .on_select(move || act(&for_act)),
        )
        .item(MenuItem::label("Details…").on_select(move || open_device(&for_details)))
        .separator()
        .item(
            MenuItem::label("Forget This Device…")
                .destructive()
                .on_select(move || ask_forget(&for_forget)),
        );

    context_menu::show_under(DEVICE_MENU_KEY, menu, QS_NS, cursor());
}

/// Asks before forgetting `device`, which then has to pair again.
pub(super) fn ask_forget(device: &BluetoothDevice) {
    let path = device.path().to_string();
    let ask = Dialog::new(
        "amane.alert.bluetooth.forget",
        format!("Forget “{}”?", device.name()),
    )
    .message("It will have to pair again before it can connect.")
    .button("Cancel", || {})
    .destructive("Forget Device", move || {
        Bluetooth::forget(&path);
        if DEVICE.with_borrow(|shown| shown.as_deref() == Some(path.as_str())) {
            close_device();
        }
    });
    alert::show(ask);
}

pub(super) fn forget_code() {
    CODE.with_borrow_mut(String::clear);
    TextInput::set_text(CODE_ID, "");
}
