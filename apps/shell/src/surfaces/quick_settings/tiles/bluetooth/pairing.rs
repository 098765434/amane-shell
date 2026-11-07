//! Pairing a device: the sheet that shows or asks for its code.

use super::*;

/// How tall the pairing sheet is: a line saying what's asked over a row
/// of controls.
pub(super) fn sheet_height() -> f32 {
    text_height(sys::r#type::caption()) + sys::space::gap::s() + sys::size::control::height()
}

/// What pairing `device` is asking: a code to confirm, type or show, or
/// a yes or no.
pub(super) fn pairing_sheet(request: &PairingRequest, device: &str) -> Box<dyn Widget> {
    let (question, controls) = match request.kind() {
        PairingKind::Confirm(passkey) => (
            format!("Is this code showing on “{device}”?"),
            confirm_row(request, &digits(*passkey), "Pair"),
        ),
        PairingKind::EnterPin => (
            format!("Enter the PIN for “{device}” (often 0000)"),
            code_row(request, "PIN", PIN_MAX),
        ),
        PairingKind::EnterPasskey => (
            format!("Enter the code shown on “{device}”"),
            code_row(request, "Code", PASSKEY_MAX),
        ),
        PairingKind::DisplayPin(pin) => (
            format!("Type this on “{device}”, then Return"),
            shown_row(request, pin, 0),
        ),
        PairingKind::DisplayPasskey { passkey, entered } => (
            format!("Type this on “{device}”, then Return"),
            shown_row(request, &digits(*passkey), usize::from(*entered)),
        ),
        PairingKind::Authorize => (
            format!("“{device}” wants to pair"),
            confirm_row(request, "", "Pair"),
        ),
        PairingKind::AuthorizeService(_) => (
            format!("Allow “{device}” to connect?"),
            confirm_row(request, "", "Allow"),
        ),
    };

    boxed(
        Column::new(vec![boxed(caption(question)), controls])
            .gap(sys::space::gap::s())
            .width(Parent),
    )
}

/// A passkey as BlueZ means it: six digits, leading zeros kept.
pub(super) fn digits(passkey: u32) -> String {
    format!("{passkey:06}")
}

/// The code (when there is one) and Cancel / `yes`.
pub(super) fn confirm_row(request: &PairingRequest, code: &str, yes: &str) -> Box<dyn Widget> {
    let id = request.id();
    let refused = request.clone();
    let cancel = PushButton::new("amane.qs.bluetooth.pair.cancel", "Cancel")
        .style(Style::Tonal)
        .on_press(move || refuse(&refused))
        .build();
    let accept = PushButton::new("amane.qs.bluetooth.pair.accept", yes)
        .style(Style::Filled)
        .on_press(move || Bluetooth::answer(id, PairingAnswer::Accept))
        .build();
    let buttons_w = fixed_width(&cancel) + fixed_width(&accept) + sys::space::gap::s() * 2.0;
    let code_w = (page_width() - buttons_w).max(1.0);

    boxed(
        Row::new(vec![
            boxed(code_text(code, 0).width(Size::Fixed(code_w))),
            boxed(cancel),
            boxed(accept),
        ])
        .gap(sys::space::gap::s())
        .width(Parent)
        .align(Center),
    )
}

/// A field for the code and Pair, with Cancel.
pub(super) fn code_row(request: &PairingRequest, placeholder: &str, max: usize) -> Box<dyn Widget> {
    let refused = request.clone();
    let cancel = PushButton::new("amane.qs.bluetooth.pair.cancel", "Cancel")
        .style(Style::Tonal)
        .on_press(move || refuse(&refused))
        .build();
    let sent = request.clone();
    let pair = PushButton::new("amane.qs.bluetooth.pair.send", "Pair")
        .style(Style::Filled)
        .on_press(move || send_code(&sent))
        .build();
    let buttons_w = fixed_width(&cancel) + fixed_width(&pair) + sys::space::gap::s() * 2.0;
    let input_w = (page_width() - buttons_w).max(1.0);

    let submitted = request.clone();
    let input = TextInput::new(CODE_ID)
        .focused()
        .placeholder(placeholder)
        .size(sys::r#type::label().size)
        .color(material::label(Level::Primary))
        .width(Size::Fixed(input_w))
        .on_change(move |text| {
            let text: String = text.chars().take(max).collect();
            CODE.set(text);
        })
        .on_submit(move |_| send_code(&submitted));

    boxed(
        Row::new(vec![boxed(input), boxed(cancel), boxed(pair)])
            .gap(sys::space::gap::s())
            .width(Parent)
            .align(Center),
    )
}

/// A code to type on the device, the digits typed so far in accent, and
/// Cancel.
pub(super) fn shown_row(request: &PairingRequest, code: &str, entered: usize) -> Box<dyn Widget> {
    let refused = request.clone();
    let cancel = PushButton::new("amane.qs.bluetooth.pair.cancel", "Cancel")
        .style(Style::Tonal)
        .on_press(move || refuse(&refused))
        .build();
    let code_w = (page_width() - fixed_width(&cancel) - sys::space::gap::s()).max(1.0);

    boxed(
        Row::new(vec![
            boxed(code_text(code, entered).width(Size::Fixed(code_w))),
            boxed(cancel),
        ])
        .gap(sys::space::gap::s())
        .width(Parent)
        .align(Center),
    )
}

/// A code in large numerals, the first `entered` characters in accent:
/// plain text, so it's drawn as letters with the rest of the page.
pub(super) fn code_text(code: &str, entered: usize) -> Row {
    let split = code
        .char_indices()
        .nth(entered)
        .map_or(code.len(), |(at, _)| at);
    let (typed, rest) = code.split_at(split);
    let style = sys::r#type::title();
    let mut parts: Vec<Box<dyn Widget>> = Vec::new();
    if !typed.is_empty() {
        parts.push(boxed(
            Text::new(typed)
                .style(style)
                .color(sys::color::accent::bg()),
        ));
    }
    if !rest.is_empty() {
        parts.push(boxed(
            Text::new(rest)
                .style(style)
                .color(material::label(Level::Primary)),
        ));
    }
    Row::new(parts).align(Center)
}

/// Answers the pairing asked about with the code typed, if it's one that
/// can be sent: digits only for a passkey.
pub(super) fn send_code(request: &PairingRequest) {
    let typed = CODE.with_borrow(|code| code.trim().to_string());
    let answer = match request.kind() {
        PairingKind::EnterPin if !typed.is_empty() => PairingAnswer::Pin(typed),
        PairingKind::EnterPasskey => match typed.parse::<u32>() {
            Ok(passkey) if typed.len() <= PASSKEY_MAX => PairingAnswer::Passkey(passkey),
            _ => return,
        },
        _ => return,
    };
    Bluetooth::answer(request.id(), answer);
    forget_code();
}

/// Says no to the request, and stops the pairing it belongs to.
pub(super) fn refuse(request: &PairingRequest) {
    if request.asks() {
        Bluetooth::answer(request.id(), PairingAnswer::Reject);
    }
    Bluetooth::cancel_pairing(request.device());
    forget_code();
}
