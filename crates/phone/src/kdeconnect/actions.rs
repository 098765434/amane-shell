use std::path::PathBuf;

use amane::run_in_order;
use amane::{Argument, Bus, Service, ServiceError};

use super::device::CLIPBOARD_PLUGIN;
use super::{CLIPBOARD, DAEMON, DAEMON_PATH, DEVICE, KDECONNECT, KdeConnect, config, device_path};

/*
 * each action runs on the worker, so a phone that's slow to answer never
 * stalls drawing; the device list is read again after, since kdeconnect's
 * own signal can come before the change is visible
 */
impl KdeConnect {
    /*
     * starts the daemon when it isn't running (d-bus starts it on the
     * first call), for a page offering to turn the phone on
     */
    pub fn start() {
        run_in_order(|| {
            let _ = Bus::session().try_call(
                KDECONNECT,
                DAEMON_PATH,
                DAEMON,
                "devices",
                &[Argument::from(false), Argument::from(false)],
            );

            super::resync();
        });
    }

    // looks for phones on the network again
    pub fn refresh() {
        run_in_order(|| {
            let _ = Bus::session().try_call(
                KDECONNECT,
                DAEMON_PATH,
                DAEMON,
                "forceOnNetworkChange",
                &[],
            );

            super::resync();
        });
    }

    // the phone shows a request to accept
    pub fn request_pair(device: &str) {
        device_call(device, "", DEVICE, "requestPairing", Vec::new());
    }

    // answers a request the phone sent
    pub fn accept_pairing(device: &str) {
        device_call(device, "", DEVICE, "acceptPairing", Vec::new());
    }

    // declines the phone's request, or stops waiting for it to accept ours
    pub fn cancel_pairing(device: &str) {
        device_call(device, "", DEVICE, "cancelPairing", Vec::new());
    }

    // forgets the phone; it has to pair again to connect
    pub fn unpair(device: &str) {
        device_call(device, "", DEVICE, "unpair", Vec::new());
    }

    // rings at full volume, even on silent, until stopped on the phone
    pub fn ring(device: &str) {
        device_call(
            device,
            "/findmyphone",
            "org.kde.kdeconnect.device.findmyphone",
            "ring",
            Vec::new(),
        );
    }

    // each file lands in the phone's downloads
    pub fn send_files(device: &str, files: Vec<PathBuf>) {
        let device = device.to_string();

        run_in_order(move || {
            begin(&device);
            if let Err(error) = KdeConnect::share_files_now(&device, &files) {
                fail(&device, error);
            }
        });
    }

    /*
     * the same, waiting for the daemon to take each file, for a caller on a
     * thread of its own that tells how it went; the files go on to the phone
     * after it answers
     */
    pub fn share_files_now(device: &str, files: &[PathBuf]) -> Result<(), ServiceError> {
        let bus = Bus::session();

        let path = format!("{}/share", device_path(device));

        for file in files {
            bus.try_call(
                KDECONNECT,
                &path,
                "org.kde.kdeconnect.device.share",
                "shareUrl",
                &[Argument::from(file_url(file))],
            )
            .map_err(ServiceError::from)?;
        }

        Ok(())
    }

    // text for the phone to show, or a link for it to open, waiting as above
    pub fn share_text_now(device: &str, text: &str) -> Result<(), ServiceError> {
        let path = format!("{}/share", device_path(device));

        Bus::session()
            .try_call(
                KDECONNECT,
                &path,
                "org.kde.kdeconnect.device.share",
                "shareText",
                &[Argument::from(text)],
            )
            .map(|_| ())
            .map_err(ServiceError::from)
    }

    // this computer's clipboard now, to the phone's
    pub fn send_clipboard(device: &str) {
        device_call(device, "/clipboard", CLIPBOARD, "sendClipboard", Vec::new());
    }

    /*
     * kdeconnect only reads this when the plugin starts, so the plugin is
     * turned off and on again after the file is written
     */
    pub fn set_clipboard_auto_sync(device: &str, on: bool) {
        let device = device.to_string();

        run_in_order(move || {
            begin(&device);
            if let Err(error) = config::set_clipboard_auto_sync(&device, on) {
                fail(&device, error.into());

                return;
            }

            restart_plugin(&device, CLIPBOARD_PLUGIN);

            super::resync();
        });
    }

    /*
     * the phone's remote: lock, suspend and screenshot this computer from
     * the phone's "Run command"; reloading sends the list to the phone
     */
    pub fn set_run_commands(device: &str, on: bool) {
        let device = device.to_string();

        run_in_order(move || {
            begin(&device);
            if let Err(error) = config::set_run_commands(&device, on) {
                fail(&device, error.into());

                return;
            }

            let _ = Bus::session().try_call(
                KDECONNECT,
                &device_path(&device),
                DEVICE,
                "reloadPlugins",
                &[],
            );

            super::resync();
        });
    }

    /// Sets the commands the phone's remote adds; once, before the first
    /// [`set_run_commands`](Self::set_run_commands). Without it the remote adds nothing.
    pub fn set_run_command_list(list: &'static [config::RunCommand]) {
        config::set_list(list);
    }

    /// The commands the remote adds, as the program set them.
    pub fn run_command_list() -> &'static [config::RunCommand] {
        config::list()
    }
}

fn restart_plugin(device: &str, plugin: &str) {
    let bus = Bus::session();

    let path = device_path(device);

    // d-bus keeps the order, so the plugin is back on after it was off
    for on in [false, true] {
        let _ = bus.try_call(
            KDECONNECT,
            &path,
            DEVICE,
            "setPluginEnabled",
            &[Argument::from(plugin), Argument::from(on)],
        );
    }
}

// one call on the device or one of its plugins (`sub` like "/battery"), then a fresh read
pub(crate) fn device_call(
    device: &str,
    sub: &'static str,
    interface: &'static str,
    method: &'static str,
    arguments: Vec<Argument>,
) {
    let device = device.to_string();

    run_in_order(move || {
        let path = format!("{}{sub}", device_path(&device));

        begin(&device);
        let result = Bus::session().try_call(KDECONNECT, &path, interface, method, &arguments);

        if let Err(error) = result {
            fail(&device, ServiceError::from(&error));
        }

        super::resync();
    });
}

/*
 * like device_call() but without reading the list again, for the trackpad
 * and keyboard, which send many small calls that change nothing shown
 */
pub(crate) fn input_call(
    device: &str,
    sub: &'static str,
    interface: &'static str,
    method: &'static str,
    arguments: Vec<Argument>,
) {
    let device = device.to_string();

    run_in_order(move || {
        let path = format!("{}{sub}", device_path(&device));

        let _ = Bus::session().try_call(KDECONNECT, &path, interface, method, &arguments);
    });
}

// a device's last failure, kept until its next action; how long it shows is the UI's choice
fn fail(device: &str, error: ServiceError) {
    KdeConnect::write()
        .failures
        .insert(device.to_string(), error);
}

// a new action on the device: its last failure goes
fn begin(device: &str) {
    let mut kdeconnect = KdeConnect::write();
    if kdeconnect.failures.remove(device).is_none() {
        kdeconnect.quiet();
    }
}

// a file:// url with the path's unsafe bytes escaped
fn file_url(file: &std::path::Path) -> String {
    let path = file.to_string_lossy();

    let mut url = String::from("file://");

    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                url.push(byte as char);
            }
            byte => url.push_str(&format!("%{byte:02X}")),
        }
    }

    url
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn file_urls_escape_spaces_and_unicode() {
        assert_eq!(
            file_url(Path::new("/home/me/My Photo é.jpg")),
            "file:///home/me/My%20Photo%20%C3%A9.jpg"
        );
    }
}
